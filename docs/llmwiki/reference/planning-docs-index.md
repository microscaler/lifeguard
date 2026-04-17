# Planning docs index (`docs/planning/`)

- **Status**: `partially-verified`
- **Source docs**: [`docs/planning/README.md`](../../planning/README.md)
- **Code anchors**: n/a
- **Last updated**: 2026-04-17

## What it is

`docs/planning/` holds **design PRDs, audits, and long-form analysis** for Lifeguard. It is large; use this page as a **routing table** — the wiki does not duplicate the PDF-length documents.

## Active clusters (start here)

| Theme | Entry |
|-------|--------|
| Pooling / replicas | [`PRD_CONNECTION_POOLING.md`](../../planning/PRD_CONNECTION_POOLING.md), [`DESIGN_CONNECTION_POOLING.md`](../../planning/DESIGN_CONNECTION_POOLING.md) |
| Validators / scopes / session | [`PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md`](../../planning/PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md), [`DESIGN_SESSION_UOW.md`](../../planning/DESIGN_SESSION_UOW.md) |
| `find_related` / scopes | [`DESIGN_FIND_RELATED_SCOPES.md`](../../planning/DESIGN_FIND_RELATED_SCOPES.md), [`DESIGN_INHERITED_PARENT_SCOPES_SPIKE.md`](../../planning/DESIGN_INHERITED_PARENT_SCOPES_SPIKE.md) |
| Compare-schema / index drift | [`DESIGN_INDEX_COMPARE_ROADMAP.md`](../../planning/DESIGN_INDEX_COMPARE_ROADMAP.md), [`DESIGN_INDEX_COMPARE_T2B_T3.md`](../../planning/DESIGN_INDEX_COMPARE_T2B_T3.md) |
| Schema inference CLI | [`DESIGN_SCHEMA_INFERENCE_CLI_CODEGEN.md`](../../planning/DESIGN_SCHEMA_INFERENCE_CLI_CODEGEN.md) |
| Derive / SeaORM parity | [`lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md`](../../planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md), [`AUTHORING_MODEL_TRAIT.md`](../../planning/lifeguard-derive/AUTHORING_MODEL_TRAIT.md) |
| Rustdoc / coverage discipline | [`DEV_RUSTDOC_AND_COVERAGE.md`](../../planning/DEV_RUSTDOC_AND_COVERAGE.md) |

## Audits subdirectory

[`docs/planning/audits/`](../../planning/audits/) — JSF panic safety, migration audits, SeaORM audits, etc. Use when hardening or investigating tech debt.

## Cross-references

- Wiki topics: [`topics/migrate-cli-integration.md`](../topics/migrate-cli-integration.md), [`topics/derive-macros-and-attributes.md`](../topics/derive-macros-and-attributes.md)
