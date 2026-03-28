# Authoring custom `ModelTrait` implementations

## `get_by_column_name` and relations

[`FindRelated::find_related`](../../../src/relation/traits.rs) and [`build_where_condition`](../../../src/relation/def/condition.rs) read **source-side** join values using each `from_col` name on [`RelationDef`](../../../src/relation/def/struct_def.rs) via [`ModelTrait::get_by_column_name`](../../../src/model.rs).

- For **BelongsTo**, `from_col` is typically a foreign key on the current row (e.g. `user_id`). Implement `get_by_column_name("user_id")` → `Some(...)`.
- For **HasMany** from parent to children, `from_col` often matches the parent primary key; macro-generated models implement `get_by_column_name` for every column. A minimal hand-written model may rely on the **primary-key name fallback** only when `from_col` names match PK column names in order (see `build_where_condition` docs).

If `get_by_column_name` returns `None` and the PK fallback does not apply, `build_where_condition` returns [`LifeError::Other`](../../../src/executor.rs) with a message naming the missing column. Callers such as [`FindRelated::find_related`](../../../src/relation/traits.rs) and [`LazyLoader::load`](../../../src/relation/lazy.rs) propagate that error instead of panicking.

## Derived models

`#[derive(LifeModel)]` generates `get_by_column_name` match arms for each persisted column (using the SQL column name, including `#[column_name = "..."]`). See tests in `lifeguard-derive/tests/test_minimal.rs` (`get_by_column_name` coverage).
