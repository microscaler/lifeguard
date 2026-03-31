# Lifeguard architecture

This document is the **detailed** architecture reference: numbered call flows (primary vs replicas, Redis, LifeReflector), multi-service deployment, connection pooling, and cache coherence sequences. For repository status see [STATUS.md](./STATUS.md); for quick start and doc index, see the [README](./README.md).

---

## Target architecture

**Numbered edges** show typical **order of operations** on the data plane. **Solid lines** are the ORM/pool path (writes always hit **primary**; reads go to **primary** or **replica** tier depending on `ReadPreference`, WAL lag, and pool routing). **Dotted lines** are **optional** cache-aside (your app or framework checks Redis before Postgres) and **background** coherence (LifeReflector runs out-of-band after commits—not on your request’s critical path).

```mermaid
flowchart TB
    subgraph ORM["Application & ORM"]
        App[Application / LifeModel / LifeRecord]
        SQ[SeaQuery → SQL]
    end

    subgraph PoolLayer["LifeguardPool + LifeExecutor"]
        Pool[LifeguardPool dispatch]
        Ex[LifeExecutor + may_postgres]
    end

    subgraph PG["PostgreSQL cluster"]
        Pri[(Primary<br/>all writes)]
        Rep[(Replica tier<br/>reads when healthy)]
    end

    subgraph Cache["Redis"]
        Redis[(Cache<br/>active set)]
    end

    subgraph Refl["LifeReflector — background"]
        LR[Leader: LISTEN / NOTIFY]
    end

    App -->|1 build query| SQ
    SQ -->|2 submit| Pool
    Pool -->|3 acquire slot| Ex

    Ex -->|4a writes + strong reads| Pri
    Ex -->|4b reads when replica OK| Rep

    App -.->|5 optional GET| Redis
    Redis -.->|6 miss → ORM read path| App

    Pri -->|7 NOTIFY after commit| LR
    LR -.->|8 if key warm: EXISTS| Redis
    LR -->|9 refresh row| Pri
    LR -.->|10 SETEX| Redis

    style Pri fill:#c0c0c0,stroke:#333,stroke-width:2px
    style Rep fill:#d8d8d8,stroke:#333,stroke-width:2px
    style Redis fill:#ffcccb,stroke:#333,stroke-width:2px
    style LR fill:#add8e6,stroke:#333,stroke-width:2px
    style Pool fill:#90ee90,stroke:#333,stroke-width:2px
    style Ex fill:#90ee90,stroke:#333,stroke-width:2px
```

**Legend (numbers):** **1–4** — synchronous request path through ORM → pool → **primary** (writes, read-your-writes, or forced primary) or **replicas** (scaled reads when the pool routes there). **5–6** — optional Redis **read-through**: not automatic in every API today; pattern is GET first, on miss run **1–4** then populate Redis. **7–10** — **asynchronous**: after a successful commit, **NOTIFY** wakes LifeReflector; it refreshes Redis only for keys already in the active set (see [The Killer Feature: LifeReflector](./VISION.md#the-killer-feature-lifereflector) in **[VISION.md](./VISION.md)**).

**Key Components:**
- **LifeguardPool**: Persistent connection pool; routes to primary vs replica **worker pools** using WAL lag and optional [`ReadPreference`](./src/pool/pooled.rs).
- **LifeExecutor**: Database execution abstraction over `may_postgres`.
- **LifeModel/LifeRecord**: Complete ORM layer (replaces SeaORM).
- **SeaQuery**: SQL building (borrowed, compatible with coroutines).
- **may_postgres**: Coroutine-native PostgreSQL client (foundation).
- **Primary vs replicas**: Writes **always** use the primary URL; reads may use the replica tier when configured and healthy.
- **LifeReflector**: Background cache coherence (not on the hot path of a single `SELECT`).
- **Redis**: Optional cache layer; coherence refresh is **7–10**, not **1–4**.

## Multi-service deployment

```mermaid
flowchart TB
    subgraph Frontend["Frontend / Clients"]
        Web[Web App]
        Mobile[Mobile App]
        API[API Clients]
    end

    subgraph BFF["BFF Layer<br/>Built with BRRTRouter"]
        BFF_Service[Backend for Frontend<br/>API Gateway / Router]
    end

    subgraph Backend["Backend Microservices<br/>Your Business Logic"]
        MS1[User Service]
        MS2[Product Service]
        MS3[Order Service]
        MSN[Service N<br/>Your Domain]
    end

    subgraph Lifeguard
        Pool[LifeguardPool]
        Executor[LifeExecutor]
        LifeModel[LifeModel / LifeRecord]
        SeaQuery[SeaQuery]
    end

    subgraph DataLayer["Data layer"]
        may_postgres[may_postgres]
        PG_P[(PostgreSQL Primary)]
        PG_R[(PostgreSQL Replicas)]
        Redis[(Redis Cache)]
    end

    subgraph LifeReflector
        Reflector[LifeReflector Leader]
    end

    Web --> BFF_Service
    Mobile --> BFF_Service
    API --> BFF_Service

    BFF_Service --> MS1
    BFF_Service --> MS2
    BFF_Service --> MS3
    BFF_Service --> MSN

    MS1 -->|1| Pool
    MS2 -->|1| Pool
    MS3 -->|1| Pool
    MSN -->|1| Pool

    Pool -->|2| Executor
    Executor --> LifeModel
    LifeModel --> SeaQuery
    SeaQuery -->|3 SQL| Executor
    Executor -->|4| may_postgres
    may_postgres -->|5a writes / RYW reads| PG_P
    may_postgres -->|5b scaled reads when routed| PG_R

    MS1 -.->|optional cache| Redis
    MS2 -.->|optional cache| Redis
    MS3 -.->|optional cache| Redis
    MSN -.->|optional cache| Redis

    PG_P -->|NOTIFY bg| Reflector
    Reflector -.->|refresh warm keys| Redis
    Reflector -->|re-read| PG_P

    style Frontend fill:#e1f5ff
    style Web fill:#e1f5ff
    style Mobile fill:#e1f5ff
    style API fill:#e1f5ff
    style BFF fill:#add8e6
    style BFF_Service fill:#add8e6
    style Backend fill:#d4edda
    style MS1 fill:#d4edda
    style MS2 fill:#d4edda
    style MS3 fill:#d4edda
    style MSN fill:#d4edda
    style Pool fill:#90ee90
    style Executor fill:#90ee90
    style LifeModel fill:#90ee90
    style may_postgres fill:#90ee90
    style PG_P fill:#c0c0c0
    style PG_R fill:#d8d8d8
    style Redis fill:#ffcccb
    style Reflector fill:#add8e6
```

**Call order on the request path:** **1** service calls into **`LifeguardPool`** → **2** **`LifeExecutor`** → **3–4** SQL via **`may_postgres`** → **5a** **primary** (all writes; reads when forced or RYW) or **5b** **replicas** (reads when the pool’s WAL/routing allows). **Dotted:** optional Redis in front of Postgres; **NOTIFY** + Reflector runs **asynchronously** after commit (not numbered on the hot path).

## Connection pool architecture

Each **slot** is a long-lived `may_postgres` connection; the pool maintains separate **primary** and **replica** worker tiers when replicas are configured—**writes** and primary-tier reads use primary slots; **replica** slots serve scaled reads when WAL lag allows (see [pool docs](./docs/POOLING_OPERATIONS.md)).

```mermaid
graph TD
    subgraph LifeguardPool["LifeguardPool<br/>The 300 Spartans"]
        S[Semaphore<br/>max_connections tokens<br/>100-500 limit]
        subgraph Slots["Connection Slots<br/>Persistent & Reused"]
            C1[Slot 1<br/>in_use: false<br/>ready]
            C2[Slot 2<br/>in_use: true<br/>executing query]
            C3[Slot 3<br/>in_use: false<br/>ready]
            CN[Slot N<br/>in_use: false<br/>ready]
        end
    end

    subgraph Traffic["Incoming Traffic<br/>The Persian Empire"]
        R1[Request 1]
        R2[Request 2]
        R3[Request 3]
        RN[Request N<br/>millions/sec]
    end

    Traffic -->|acquire| S
    S -->|find free| Slots
    Slots -->|mark in_use| C2
    C2 -->|execute query| PG[PostgreSQL<br/>The Pass]
    PG -->|result| C2
    C2 -->|release| S
    C2 -->|mark free| Slots
    C2 -->|ready for| Traffic

    style LifeguardPool fill:#fff4e1
    style S fill:#fff4e1
    style Slots fill:#e1ffe1
    style PG fill:#e1ffe1
    style Traffic fill:#ffe1e1

    Note[100 connections<br/>handle millions of requests<br/>through aggressive reuse]
    LifeguardPool --> Note
```

## LifeReflector cache coherence

```mermaid
sequenceDiagram
    participant LifeRecord
    participant Postgres
    participant LifeReflector
    participant Redis
    participant LifeModel

    LifeRecord->>Postgres: Write (INSERT/UPDATE/DELETE)
    Postgres->>Postgres: Commit Transaction
    Postgres->>LifeReflector: NOTIFY table_changes, '{"id": 42}'
    LifeReflector->>Redis: EXISTS lifeguard:model:table:42?
    alt Key Exists (Active Item)
        LifeReflector->>Postgres: SELECT * FROM table WHERE id = 42 (from Primary)
        Postgres-->>LifeReflector: Fresh Data
        LifeReflector->>Redis: SETEX lifeguard:model:table:42 <TTL> <Serialized Data>
    else Key Not Exists (Inactive)
        LifeReflector->>LifeReflector: Ignore (item not cached, TTL expired)
    end
    LifeModel->>Redis: Read (GET lifeguard:model:table:42)
    alt Cache Hit
        Redis-->>LifeModel: Cached Data (Fresh)
    else Cache Miss
        LifeModel->>Postgres: Read (SELECT * FROM table WHERE id = 42)
        Postgres-->>LifeModel: Data
        LifeModel->>Redis: SETEX lifeguard:model:table:42 <TTL> <Serialized Data>
    end
```

---

[← README](./README.md) · [Vision](./VISION.md) · [Status](./STATUS.md) · [Comparison](./COMPARISON.md)
