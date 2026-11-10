//! The metric recording on the query path must not slow it down: once a series exists, recording
//! it allocates nothing (no label formatting, no per-call `Vec`/`String`). A counting global
//! allocator checks that for every recorder the executors, the pool and the derive call per
//! statement or operation, and the label helpers are checked for bounded output.
#![cfg(feature = "metrics")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::Duration;

use lifeguard::metrics::entity::{record_tx, CacheResult, EntityMetrics, Op};
use lifeguard::metrics::{sqlstate_class, statement, ERROR_CLASSES};

struct Counting;

thread_local! {
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
}

// SAFETY: forwards to the system allocator; only counts calls on the current thread.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.with(|c| c.set(c.get() + 1));
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn allocations(f: impl FnOnce()) -> u64 {
    let before = ALLOCS.with(Cell::get);
    f();
    ALLOCS.with(Cell::get) - before
}

#[test]
fn recording_allocates_nothing_once_the_series_exist() {
    let m: &'static EntityMetrics = Box::leak(Box::new(EntityMetrics::new("noalloc_items")));
    let d = Duration::from_micros(250);
    let record_all = || {
        statement::record(d, Some("primary"), "SELECT id FROM items WHERE id = $1");
        statement::record(d, Some("replica"), "with x as (select 1) select * from x");
        statement::record(d, None, "  INSERT INTO items (id) VALUES ($1)");
        statement::record(d, Some("primary"), "VACUUM items");
        m.record(Op::Select, d, 3, true);
        m.record(Op::Upsert, d, 1, false);
        m.record_cache(CacheResult::Hit);
        m.record_cache(CacheResult::Miss);
        record_tx(d, true);
        record_tx(d, false);
        assert_eq!(sqlstate_class(Some("23505")), "unique_violation");
    };
    record_all(); // the first use of each series registers it
    let n = allocations(|| {
        for _ in 0..10_000 {
            record_all();
        }
    });
    assert_eq!(n, 0, "{n} allocations in 10 000 rounds of recording");
}

#[test]
fn sqlstate_classes_are_a_fixed_set() {
    let cases = [
        (Some("23505"), "unique_violation"),
        (Some("23503"), "integrity"),
        (Some("23502"), "integrity"),
        (Some("40001"), "serialization"),
        (Some("40P01"), "serialization"),
        (Some("40002"), "transaction"),
        (Some("08006"), "connection"),
        (Some("53300"), "resource"),
        (Some("57014"), "operator"),
        (Some("42P01"), "syntax"),
        (Some("22P02"), "data"),
        (Some("XX000"), "other"),
        (Some(""), "other"),
        (None, "client"),
    ];
    for (code, class) in cases {
        assert_eq!(sqlstate_class(code), class, "{code:?}");
        assert!(ERROR_CLASSES.contains(&class));
    }
}

#[test]
fn statement_kinds_and_tiers_are_bounded() {
    assert_eq!(
        statement::KINDS[statement::kind_index("select 1")],
        "select"
    );
    assert_eq!(
        statement::KINDS[statement::kind_index("\n  Update t set a = 1")],
        "update"
    );
    assert_eq!(statement::KINDS[statement::kind_index("WITH(x)")], "with");
    assert_eq!(statement::KINDS[statement::kind_index("commit;")], "commit");
    assert_eq!(
        statement::KINDS[statement::kind_index("TRUNCATE t")],
        "other"
    );
    assert_eq!(statement::KINDS[statement::kind_index("")], "other");
    assert_eq!(
        statement::TIERS[statement::tier_index(Some("replica"))],
        "replica"
    );
    assert_eq!(statement::TIERS[statement::tier_index(None)], "direct");
    assert_eq!(statement::TIERS[statement::tier_index(Some("x"))], "direct");
}

#[test]
fn the_new_series_are_exported() {
    let m: &'static EntityMetrics = Box::leak(Box::new(EntityMetrics::new("export_items")));
    m.record_cache(CacheResult::Error);
    record_tx(Duration::from_millis(2), true);
    statement::record(Duration::from_millis(1), Some("primary"), "DELETE FROM t");
    let text = lifeguard::metrics::prometheus_scrape_text();
    assert!(
        text.contains(r#"lifeguard_entity_cache_total{entity="export_items",result="error"} 1"#),
        "{text}"
    );
    assert!(text.contains(r#"lifeguard_entity_op_duration_seconds_count{entity="_tx",op="tx"}"#));
    assert!(text.contains(
        r#"lifeguard_statement_duration_seconds_count{kind="delete",pool_tier="primary"}"#
    ));
}
