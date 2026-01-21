# Issue: Type Inference Error with UUID/NaiveDateTime in LifeModel Macro

## Problem

When using `uuid::Uuid` or `chrono::NaiveDateTime` types in `LifeModel` entities, the compiler fails with:

```
error[E0284]: type annotations needed
  --> lifeguard-migrate/tests/test_sql_generation.rs:18:14
   |
18 |     #[derive(LifeModel)]
   |              ^^^^^^^^^ cannot infer type
   |
  = note: cannot satisfy `<_ as Try>::Residual == _`
```

## Root Cause

The `?` operator in the generated `FromRow` implementation cannot infer the error type conversion when parsing UUID/NaiveDateTime from strings. This is particularly problematic with `Option<uuid::Uuid>` and `Option<chrono::NaiveDateTime>`.

## Attempted Solutions

1. Using explicit `match` expressions with early returns
2. Using `transpose()` with explicit type annotations
3. Using `if let` with explicit Result handling
4. Separating the Result creation from the `?` operator

None of these approaches resolved the type inference issue.

## Workaround

Temporarily use `String` types instead of `uuid::Uuid` and `chrono::NaiveDateTime` until this issue is resolved.

## Next Steps

1. Investigate if `may_postgres::Error::__private_api_timeout()` is the correct way to create errors
2. Consider using a different error creation method
3. Review the macro structure to see if the issue is with how `get_expr` is used
4. Consider separating UUID/NaiveDateTime handling into a helper function or trait

## Related Files

- `lifeguard-derive/src/macros/life_model.rs` (lines 981-1054)
- `lifeguard-migrate/tests/test_sql_generation.rs` (test entity definition)
