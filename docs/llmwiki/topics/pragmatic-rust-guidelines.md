# Microsoft Pragmatic Rust Guidelines — Lifeguard stance

> **Status:** ACTIVE
> **Last-synced:** 2026-04-18 — against [`../references/rust-guidelines.md`](../references/rust-guidelines.md) (Microsoft Pragmatic Rust Guidelines; MIT-licensed upstream).
> **Authority:** Upstream [Pragmatic Rust Guidelines](https://microsoft.github.io/pragmatic-rust-guidelines/) — the repo copy is the **verbatim** reference; this page is the **Lifeguard** synthesis.
> **Related:** [`coding-standards-jsf-inspired.md`](./coding-standards-jsf-inspired.md), [`entities/transaction-boundaries.md`](../entities/transaction-boundaries.md).

## What this page covers

Microsoft’s guide names rules (`M-*`) for API design, errors, docs, performance, and AI-friendly surfaces. Lifeguard is a **library** (ORM + migrate + derive ecosystem), not an application — rules about app allocators or `anyhow` in binaries are mostly **downstream** (Hauliage). This page lists what we **honour today**, what we **aspire to** in public rustdoc, and what we **decline** for a typed library API.

## Rules we honour (library / workspace)

| Rule id | Summary | Lifeguard application |
|--------|---------|------------------------|
| **M-PANIC-IS-STOP** | Panic = process stop; libraries return `Result` for recoverable cases. | Errors from queries, pool, migrate — typed `LifeError` / migrate errors; no `unwrap` on user-driven input paths. |
| **M-PUBLIC-DEBUG** | Public types implement `Debug` where appropriate. | Public model and config types derive `Debug` unless explicitly opaque. |
| **M-UNSAFE** | Minimize `unsafe`; document soundness if used. | `unsafe` only where unavoidable; documented and reviewed. |
| **M-DESIGN-FOR-AI** | Predictable names, strong types, good docs. | `LifeModel`, `SelectQuery`, migrate CLI — rustdoc and book chapters in [`book/`](../../../book/). |
| **M-LINT-OVERRIDE-EXPECT** | Prefer `#[expect(lint, reason = "...")]` over stale `#[allow]`. | New suppressions use `expect` with rationale. |
| **M-CANONICAL-DOCS** | Public items document errors, panics, examples. | Required for new public API; see `DEVELOPMENT.md` + rustdoc lints. |

## Rules we adopt gradually (documentation / PR hygiene)

| Rule id | Summary | When |
|--------|---------|------|
| **M-MODULE-DOCS** | Every module has `//!` purpose. | Expand as modules are touched in substantive PRs. |
| **M-FIRST-DOC-SENTENCE** | First rustdoc sentence standalone. | Enforce in code review for new public items. |
| **M-LOG-STRUCTURED** | Structured tracing fields vs format-only strings. | OTEL/tracing paths per [`observability-and-logging.md`](./observability-and-logging.md). |

## Rules we decline or defer (wrong layer for this crate)

| Rule id | Summary | Why |
|--------|---------|-----|
| **M-APP-ERROR** | Apps may use `anyhow`. | Lifeguard stays with typed errors; hosts choose `anyhow` if they wish. |
| **M-MIMALLOC-APPS** | Allocator choice for apps. | Host binary decision, not ORM crate. |

## Raw source

Full text: [`../references/rust-guidelines.md`](../references/rust-guidelines.md). Prefer editing **this synthesis** when stance changes; update the reference only when refreshing from upstream.

## Open questions

> **Open:** Periodic diff against [microsoft.github.io/pragmatic-rust-guidelines](https://microsoft.github.io/pragmatic-rust-guidelines/) when Microsoft publishes a new revision — bump `Last-synced` and add a `log.md` entry.
