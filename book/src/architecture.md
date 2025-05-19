
---

## 📄 `lifeguard-book/src/architecture.md`

```markdown
# Architecture

```
┌────────────┐         ┌────────────────┐         ┌────────────────────────┐
│ may::go!   │─────▶──▶│  DbPoolManager │─────▶──▶   tokio + SeaORM client │
└────────────┘         └────────────────┘         └─────────────▲──────────┘
async await
```


## Key Concepts

- `DbPoolManager` spawns a single Tokio runtime in a thread
- All queries are queued via `crossbeam_channel`
- Each query is timed and recorded using OpenTelemetry metrics
- Queries are executed using SeaORM's async API from within `tokio::spawn`


