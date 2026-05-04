# BRRTRouter + Lifeguard integration pitfalls

- **Status**: `partially-verified`
- **Source docs**: [`AGENT.md`](../../../AGENT.md) case-study section (moved here), Hauliage postmortems (fleet, consignments), [`docs/UUID_AND_POSTGRES_TYPES.md`](../../UUID_AND_POSTGRES_TYPES.md)
- **Code anchors**: Lifeguard `from_row` / type mapping; BRRTRouter `register_from_spec` / dispatcher (sibling repo)
- **Last updated**: 2026-04-17

## What it is

Microservices that combine **Lifeguard** for persistence and **BRRTRouter** for HTTP/OpenAPI can exhibit **silent empty responses** (`200` + `[]`) when two independent failure modes line up: ORM row decode issues and strict response validation stripping non-schema payloads.

## Pitfall 1 — PostgreSQL `UUID` vs Rust `String`

If a PostgreSQL `UUID` column is mapped to `pub id: String` on a `LifeModel`, the row decode path can fail in ways that surface as errors or degraded paths in service code. Downstream, if handlers map errors into shapes that **BRRTRouter’s response validator** rejects (for example unknown `status` enums), the gateway may strip content and the client sees an empty array.

**Prevention:** Use `uuid::Uuid` (or `Option<uuid::Uuid>`) for UUID columns; stringify only at API boundaries if the contract requires strings.

Authority: [`docs/UUID_AND_POSTGRES_TYPES.md`](../../UUID_AND_POSTGRES_TYPES.md), Hauliage [`postmortem-fleet-api-response-mismatch-2026-04.md`](../../../../hauliage/docs/postmortems/postmortem-fleet-api-response-mismatch-2026-04.md), [`postmortem-consignments-list-jobs-empty-2026-04.md`](../../../../hauliage/docs/postmortems/postmortem-consignments-list-jobs-empty-2026-04.md).

## Pitfall 2 — `register_from_spec` before manual controller overrides

`registry::register_from_spec` establishes generated stub routes. Manual `dispatcher.add_route` overrides must run **after** registration so they replace the stub senders. If ordering inverts, stubs that return canned empty data can remain authoritative.

**Prevention:** Follow Hauliage ADR 0001 “Register & Overwrite” — see [`hauliage/docs/adr/0001-brrtrouter-controller-routing-strategy.md`](../../../../hauliage/docs/adr/0001-brrtrouter-controller-routing-strategy.md) and [`hauliage/docs/llmwiki/topics/scaffolding-lifecycle.md`](../../../../hauliage/docs/llmwiki/topics/scaffolding-lifecycle.md).

## Cross-references

- BRRTRouter wiki: [`BRRTRouter/llmwiki/`](../../../../BRRTRouter/llmwiki/) (four levels up from `topics/` to `microscaler/`, then into `BRRTRouter`)
