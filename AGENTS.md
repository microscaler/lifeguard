# Lifeguard — agent rules

> **Desktop dev environment** — before doing anything in this repo, read the
> Microscaler-wide topology brief. It explains that you are on a Mac but the
> code lives on `ms02` (NFS), where commands execute for this environment, how
> the Kind cluster and vLLM fit in, and the network constraints behind the SSH
> tunneling. Do not duplicate its contents here — link to it. If reality drifts,
> fix the canonical doc, not this copy.
>
> - GitHub: [`cylon-local-infra/docs/desktop-dev-environment.md`](https://github.com/microscaler/cylon-local-infra/blob/main/docs/desktop-dev-environment.md)
> - On ms02 NFS: `~/Workspace/microscaler/cylon-local-infra/docs/desktop-dev-environment.md`

---

Strict operational rules for AI assistants working on the Lifeguard ORM and migration tooling. **Knowledge** (how subsystems work, integration pitfalls, doc map) lives in **[`docs/llmwiki/`](./docs/llmwiki/)**, not in this file.

---

## Before you do anything

1. Read [`docs/llmwiki/README.md`](./docs/llmwiki/README.md) — wiki entry point (`SCHEMA.md`, `index.md`, `log.md`, `docs-catalog.md`).
2. Open [`docs/llmwiki/index.md`](./docs/llmwiki/index.md) — full catalog of **reference / entities / topics** (subsystem map). Pick the page for the area you are changing before opening random `docs/planning` files.
3. Tail [`docs/llmwiki/log.md`](./docs/llmwiki/log.md) for recent context.

**Explicit rule:** Do not treat this file as a wiki. For index attributes, BRRTRouter + Lifeguard footguns, and where reference docs live, use the wiki pages linked above.

---

## Core rules

### 1. Prefer `uuid::Uuid` for PostgreSQL `UUID` columns

Map UUID columns to `uuid::Uuid` (or `Option<uuid::Uuid>`) on `LifeModel` structs — not `String`. See [`docs/UUID_AND_POSTGRES_TYPES.md`](./docs/UUID_AND_POSTGRES_TYPES.md) and [`docs/llmwiki/topics/brrtrouter-integration-pitfalls.md`](./docs/llmwiki/topics/brrtrouter-integration-pitfalls.md).

### 2. `#[index]` / `#[indexed]` columns must exist on the struct

The derive parses index strings but does not prove columns exist; broken SQL fails at migration apply. See [`docs/llmwiki/topics/index-and-derive-constraints.md`](./docs/llmwiki/topics/index-and-derive-constraints.md).

### 3. Authoritative paths

| Area | Path |
|------|------|
| Derive macros | `lifeguard-derive/` |
| Migrations / compare-schema / infer | `lifeguard-migrate/`, [`lifeguard-migrate/README.md`](./lifeguard-migrate/README.md) |
| Integration tests | `docs/TEST_INFRASTRUCTURE.md`, `tests-integration/` |
| Planning / PRDs | `docs/planning/README.md` |

### 4. Downstream repos

Hauliage and BRRTRouter have their own agent rules and wikis. When debugging app-level routing or seeds, open **[`../hauliage/docs/llmwiki/`](../hauliage/docs/llmwiki/)** and **[`../BRRTRouter/llmwiki/`](../BRRTRouter/llmwiki/)** from a `microscaler/` sibling checkout.

### 5. Raw SQL is a last resort

Prefer **`SelectQuery`** and idiomatic ORM APIs (`LifeModel` / `LifeRecord`, relations, scopes, validators). **Do not** add raw SQL helpers or string-built queries for convenience. **New** raw-SQL paths require **explicit human approval** and a **comprehensive ADR** that shows the use case cannot be met by extending Lifeguard with new idiomatic ORM functionality. Full policy: [`docs/llmwiki/topics/raw-sql-vs-selectquery-policy.md`](./docs/llmwiki/topics/raw-sql-vs-selectquery-policy.md).

### 6. JSF-inspired discipline + Microsoft Pragmatic Rust (library stance)

Lifeguard adopts the **same reference bundle** as BRRTRouter and `microscaler-observability`: Joint Strike Fighter AV rules **distilled for Rust** (bounded complexity, explicit errors, strong types, tests on dispatch paths) plus Microsoft’s **Pragmatic Rust Guidelines** for API and documentation hygiene.

- **Synthesis (read first):** [`docs/llmwiki/topics/coding-standards-jsf-inspired.md`](./docs/llmwiki/topics/coding-standards-jsf-inspired.md), [`docs/llmwiki/topics/pragmatic-rust-guidelines.md`](./docs/llmwiki/topics/pragmatic-rust-guidelines.md).
- **Raw sources (verbatim):** [`docs/references/jsf-writeup.md`](./docs/references/jsf-writeup.md), [`docs/references/jsf-audit-opinion.md`](./docs/references/jsf-audit-opinion.md), [`docs/references/jsf-compliance.md`](./docs/references/jsf-compliance.md), [`docs/references/rust-guidelines.md`](./docs/references/rust-guidelines.md).
- **Mechanical alignment:** [`clippy.toml`](./clippy.toml) numeric thresholds match the platform stack; [`DEVELOPMENT.md`](./DEVELOPMENT.md) for fmt/clippy workflow.

---

## Postmortems and formal references (read via wiki)

The wiki links to postmortems and ADRs in Lifeguard and Hauliage; start from [`docs/llmwiki/topics/brrtrouter-integration-pitfalls.md`](./docs/llmwiki/topics/brrtrouter-integration-pitfalls.md) instead of duplicating them here.

- Lifeguard: [`docs/postmortem-lifeguard-derive-naivedate-chronodate-2026-04.md`](./docs/postmortem-lifeguard-derive-naivedate-chronodate-2026-04.md)

---

*This file is for rules and navigation only. Knowledge belongs in [`docs/llmwiki/`](./docs/llmwiki/).*
