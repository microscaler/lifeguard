# Planning Documents Archive

This directory contains planning, design, analysis, and tracking documents that were used during Lifeguard development. These files are preserved for historical reference and documentation website generation, but are excluded from git tracking.

## Directory Structure

- **analysis/** - Codebase and feature-specific analysis documents
- **audits/** - Pattern, testing, and migration audit documents
- **epics/** - Epic-level analysis and summary documents
- **epics-stories/** - Individual epic and story planning documents
- **examples-entities/** - Entity implementation status and migration tool test documents
- **lifeguard-derive/** - Procedural macro design and analysis documents
- **lifeguard-migrate/** - Migration tool design documents
- **migrations/** - Schema design documents (showcase examples)
- **root/** - Root-level planning documents

## Purpose

These documents were moved here to:
1. Keep them available for documentation website generation
2. Exclude them from git tracking to reduce repository clutter
3. Preserve historical context and design decisions
4. Organize planning documents separately from active documentation

## Important Notes

- **`.agent/` files** - Preserved in original location (active agent context)
- **`migrations/README.md`** - Preserved in original location (important documentation)
- All other planning markdown files have been moved here

## Usage

These files can be used by documentation generators to create comprehensive documentation websites that include the full development history, design decisions, and planning context.

Generated: 2026-01-22

## Active design docs (pooling)

- [`DESIGN_CONNECTION_POOLING.md`](./DESIGN_CONNECTION_POOLING.md) — in-process pool behavior, metrics, PRD §9 decisions (companion to [`PRD_CONNECTION_POOLING.md`](./PRD_CONNECTION_POOLING.md)).
- [`DESIGN_FIND_RELATED_SCOPES.md`](./DESIGN_FIND_RELATED_SCOPES.md) — how named scopes interact with `find_related` / loaders (PRD Phase C follow-on).

## Active PRDs (ORM parity)

- [`PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md`](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md) — schema inference, validators, scopes, F() expressions, session/UoW (**slug:** `schema_validators_session_and_scopes`).
- [`PRD_FOLLOWON_NEXT_THREE.md`](./PRD_FOLLOWON_NEXT_THREE.md) — expanded follow-on items (G6, `find_related`+scope example surface, inherited parent+loader).
- [`DESIGN_INHERITED_PARENT_SCOPES_SPIKE.md`](./DESIGN_INHERITED_PARENT_SCOPES_SPIKE.md) — inherited parent scopes + loaders (**spike completed** — recommendation A + D).
- [`DESIGN_INDEX_COMPARE_ROADMAP.md`](./DESIGN_INDEX_COMPARE_ROADMAP.md) — `compare-schema` index parity (**T1** / **T2** / **T4** shipped; **T2b** / **T3** backlog). Detailed design: [`DESIGN_INDEX_COMPARE_T2B_T3.md`](./DESIGN_INDEX_COMPARE_T2B_T3.md).
- [`DEV_RUSTDOC_AND_COVERAGE.md`](./DEV_RUSTDOC_AND_COVERAGE.md) — checklist for rustdoc and tests while building features.
