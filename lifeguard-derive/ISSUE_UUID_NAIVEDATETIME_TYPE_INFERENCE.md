# Issue: Type Inference Error with UUID/NaiveDateTime in LifeModel Macro

## Status: ✅ RESOLVED

## Problem

When using `uuid::Uuid` or `chrono::NaiveDateTime` types in `LifeModel` entities, the compiler failed with:

```
error[E0284]: type annotations needed
  --> lifeguard-migrate/tests/test_sql_generation.rs:18:14
   |
18 |     #[derive(LifeModel)]
   |              ^^^^^^^^^ cannot infer type
   |
  = note: cannot satisfy <_ as Try>::Residual == _`
```

## Root Cause

The `?` operator in the generated `FromRow` implementation could not infer the error type conversion when parsing UUID/NaiveDateTime from strings. This was particularly problematic with `Option<uuid::Uuid>` and `Option<chrono::NaiveDateTime>`.

## Solution

Replaced `?` operator with explicit `match` expressions that use early returns:

```rust
// For nullable UUID
let uuid_str: Option<String> = match row.try_get(#column_name_str) {
    Ok(v) => v,
    Err(e) => return Err(e),
};
match uuid_str {
    None => None,
    Some(s) => {
        match uuid::Uuid::parse_str(&s) {
            Ok(u) => Some(u),
            Err(_) => return Err(may_postgres::Error::__private_api_timeout()),
        }
    }
}
```

This avoids the type inference issue by explicitly handling all error cases with early returns instead of relying on the `?` operator's type inference.

## Key Changes

1. Changed `row.get()` to `row.try_get()` for non-nullable UUID/NaiveDateTime (since `get()` doesn't return a `Result`)
2. Used explicit `match` expressions with early returns instead of `?` operator
3. Applied the same pattern to both nullable and non-nullable cases

## Related Files

- `lifeguard-derive/src/macros/life_model.rs` (lines 981-1054)
- `lifeguard-migrate/tests/test_sql_generation.rs` (test entity definition)
