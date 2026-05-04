# Session, identity map, `SessionIdentityModelCell`

- **Status**: `partially-verified`
- **Source docs**: [`docs/planning/DESIGN_SESSION_UOW.md`](../../planning/DESIGN_SESSION_UOW.md), [`docs/planning/PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md`](../../planning/PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md)
- **Code anchors**: `lifeguard/src/session/`
- **Last updated**: 2026-04-17

## What it is

The **`session`** module provides an identity map pattern and dirty notification for batched writes / unit-of-work style usage (PRD Phase E). **`SessionIdentityModelCell`** uses internal mutability; module docs describe **Send**/`Rc` protocol — read before changing.

## Cross-references

- [`query-select-and-active-model.md`](./query-select-and-active-model.md)
