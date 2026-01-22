# Bug: has_many_through Relationship Missing Join Table FK Columns

## Status
✅ **FIXED** - Fixed in current commit

## Issue Description

The `has_many_through` relationship implementation stored the `through_tbl` reference but set `from_col` and `to_col` to the primary keys of source and target entities. For a many-to-many relationship like Post → PostTags → Tags, this caused `join_on_expr()` to generate `posts.id = tags.id` instead of the correct two-join pattern (`posts.id = post_tags.post_id` AND `post_tags.tag_id = tags.id`). The foreign key columns in the join table (`post_id`, `tag_id`) were never captured, making it impossible to generate correct SQL.

## Impact

- Using `find_related()` with `HasManyThrough` relationships would return incorrect results
- Using `*_with_def` methods with `HasManyThrough` relationships would generate wrong SQL
- The relationship definition was incomplete and couldn't be used to build proper queries

## Root Cause

The `RelationDef` struct lacked fields to store the foreign key columns in the join table:
- `through_from_col`: FK in join table pointing to source entity (e.g., "post_id" in PostTags)
- `through_to_col`: FK in join table pointing to target entity (e.g., "tag_id" in PostTags)

The `DeriveRelation` macro also didn't capture these columns when generating `has_many_through` relationships.

## Solution

1. **Added new fields to `RelationDef`**:
   - `through_from_col: Option<Identity>` - FK in join table pointing to source
   - `through_to_col: Option<Identity>` - FK in join table pointing to target

2. **Updated `DeriveRelation` macro** to infer and generate these fields for `has_many_through` relationships:
   - `through_from_col` is inferred from source entity name (e.g., "post_id" from "Post")
   - `through_to_col` is inferred from target entity name (e.g., "tag_id" from "Tag")

3. **Added `join_on_exprs()` method** to `RelationDef` that returns both join expressions:
   - First join: `source_table.primary_key = through_table.through_from_col`
   - Second join: `through_table.through_to_col = target_table.primary_key`

4. **Added `has_many_through_with_def()` method** to `RelationTrait` that uses `join_on_exprs()`

5. **Updated all `RelationDef` constructions** throughout the codebase to include the new fields

## Tests Added

- `test_derive_relation_has_many_through`: Verifies `through_from_col` and `through_to_col` are correctly set
- `test_derive_relation_has_many_through_join_exprs`: Verifies `join_on_exprs()` generates correct two joins
- `test_derive_relation_join_on_exprs_panics_on_non_has_many_through`: Negative test ensuring `join_on_exprs()` only works for `HasManyThrough`

## Files Changed

- `src/relation/def/struct_def.rs`: Added fields and `join_on_exprs()` method
- `lifeguard-derive/src/macros/relation.rs`: Updated macro to generate join table FK columns
- `src/relation/traits.rs`: Added `has_many_through_with_def()` method
- `src/relation/def/mod.rs`: Updated test RelationDef constructions
- `src/relation/def/condition.rs`: Updated test RelationDef constructions
- `src/relation/traits.rs`: Updated RelationDef constructions
- `src/relation/lazy.rs`: Updated RelationDef constructions
- `src/relation/eager.rs`: Updated RelationDef constructions
- `lifeguard-derive/tests/test_derive_relation.rs`: Added comprehensive tests

## Verification

The fix ensures that:
- `has_many_through` relationships correctly capture join table FK columns
- `join_on_exprs()` generates the correct two-join SQL pattern
- All existing code continues to work (backward compatible with `None` values for non-has_many_through relationships)
