# Lifeguard LLM Wiki Schema

## Purpose

This wiki is a **persistent, compounding knowledge layer** for the Lifeguard ORM and migration tooling, maintained per [Karpathy's llm-wiki pattern](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f). The LLM maintains cross-links and synthesis; humans curate sources (PRDs, design docs, postmortems, code).

## Source of truth order

When claims disagree, the higher-ranked source wins:

1. **Runtime + public API:** `src/`, `lifeguard-derive/src/`, `lifeguard-migrate/src/`, `lifeguard-codegen/`, `lifeguard-reflector/`, `examples/`, integration tests.
2. **Generated artifacts:** `migrations/generated/`, outputs of `lifeguard-migrate` / `infer-schema` when committed.
3. **Human-authored technical docs:** `docs/*.md` (operational references), `docs/planning/**` (designs and PRDs), crate `README.md` files, `book/src/**` (mdBook).
4. **Root narrative docs:** `README.md`, `ARCHITECTURE.md`, `VISION.md`, `COMPARISON.md`, `DEVELOPMENT.md`, `ROADMAP.md`, `CHANGELOG.md`.
5. **This wiki** (`docs/llmwiki/**`) — reconciled synthesis; never overrides code.

If the wiki contradicts `cargo doc` or the sources above, update the wiki.

## Page layout (flat, two subfolders)

```
docs/llmwiki/
├── SCHEMA.md              ← this file
├── README.md              ← entry point
├── index.md               ← content catalog
├── log.md                 ← append-only activity log
├── docs-catalog.md        ← inventory of sources outside the wiki
├── topics/                ← cross-cutting themes
├── entities/              ← long-lived concepts (optional)
└── reconciliation/        ← drift audits (optional)
```

`topics/` and `entities/` are **flat** (no deeper nesting). Use relative links.

## Page conventions

Substantive pages use a short header:

```
# <Title>

- **Status**: `verified` | `partially-verified` | `unverified`
- **Source docs**: paths under `docs/`, `book/`, or crate READMEs
- **Code anchors**: repo-relative (`lifeguard-derive/src/attributes.rs`)
- **Last updated**: YYYY-MM-DD
```

Then: **What it is**, **Where it lives**, **Gotchas** (`> **Open:**` / `> **Drift:**`), **Cross-references**.

## Operations (Karpathy's three)

### Ingest

1. Read the new or updated source.
2. Decide which wiki pages it touches.
3. Update or create pages; prefer updating in place.
4. Update `index.md` and append `log.md`.
5. Flag contradictions with `> **Drift:**`.

### Query

1. Read `index.md` first.
2. Follow links; verify against code when uncertain.
3. File substantive answers back into `topics/` or `entities/`.

### Lint

Check for stale claims, orphan pages, missing cross-links, gaps between `docs/planning` and shipped behavior.

## Agent workflow

### Session start

1. Read [`AGENT.md`](../../AGENT.md) at the repo root (strict rules).
2. Read this `SCHEMA.md` and [`index.md`](./index.md).
3. Tail [`log.md`](./log.md).
4. Open topic pages for the task’s area.

### Session end

Summarize learning into the relevant topic pages, append `log.md`, update Memory Bank if your environment uses it.

## Related

- [`README.md`](./README.md) — one-paragraph entry.
- [`docs-catalog.md`](./docs-catalog.md) — raw source inventory.
- [Hauliage sibling wiki](../../../hauliage/docs/llmwiki/) — consumer patterns, seeds, BFF (three levels up from `docs/llmwiki/` to the `microscaler/` checkout parent).
