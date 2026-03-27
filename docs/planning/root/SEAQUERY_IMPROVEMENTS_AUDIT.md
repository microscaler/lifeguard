# Sea-Query Improvements Audit

This document tracks improvements needed in the `sea-query` crate that would benefit Lifeguard. These improvements could potentially be contributed back to the sea-query project.

**Purpose:** Track API limitations and enhancement opportunities in sea-query that impact Lifeguard's functionality.

**Status:** Active - Will be enriched as we discover more limitations and opportunities.

---

## 1. SelectStatement Clause Preservation

**Status:** 🔴 Blocking  
**Priority:** High  
**Impact:** Partial model queries lose WHERE, ORDER BY, LIMIT, OFFSET clauses

### Current Limitation

When using `SelectStatement`, there is no way to:
1. Extract existing clauses (WHERE, ORDER BY, LIMIT, OFFSET, GROUP BY, HAVING, JOINs)
2. Replace columns while preserving other clauses
3. Clone and modify only the SELECT columns

### Impact on Lifeguard

The `select_partial()` method in `src/partial_model/query.rs` cannot preserve query clauses when replacing columns. This forces users to call `select_partial()` early in the query chain, before adding filters/ordering.

**Current Workaround:**
```rust
// Users must do this:
let results = User::find()
    .select_partial::<UserPartial>()  // Must be early!
    .filter(Expr::col("status").eq("active"))
    .order_by("id", Order::Asc)
    .all(executor)?;
```

**Desired Behavior:**
```rust
// Users should be able to do this:
let results = User::find()
    .filter(Expr::col("status").eq("active"))
    .order_by("id", Order::Asc)
    .select_partial::<UserPartial>()  // Should preserve filters/ordering
    .all(executor)?;
```

### Proposed Solution

Add one or more of the following to `SelectStatement`:

#### Option 1: Clause Getters
```rust
impl SelectStatement {
    pub fn get_where(&self) -> Option<&Condition>;
    pub fn get_order_by(&self) -> &[OrderExpr];
    pub fn get_limit(&self) -> Option<u64>;
    pub fn get_offset(&self) -> Option<u64>;
    pub fn get_group_by(&self) -> &[ColumnRef];
    pub fn get_having(&self) -> Option<&Condition>;
    pub fn get_joins(&self) -> &[JoinExpr];
}
```

#### Option 2: Column Replacement
```rust
impl SelectStatement {
    /// Replace all columns while preserving other clauses
    pub fn replace_columns<I>(&mut self, columns: I)
    where
        I: IntoIterator<Item = impl IntoColumnRef>;
    
    /// Clear all columns (for rebuilding SELECT clause)
    pub fn clear_columns(&mut self);
}
```

#### Option 3: Query Builder Pattern
```rust
impl SelectStatement {
    /// Create a builder that allows modifying columns while preserving clauses
    pub fn with_column_replacement(&mut self) -> ColumnReplacementBuilder;
}
```

### Implementation Notes

- Would need to expose internal state of `SelectStatement`
- May require changes to internal structure visibility
- Should maintain backward compatibility
- Consider performance implications of cloning clauses

### Related Code

- **Lifeguard:** `src/partial_model/query.rs:144-192`
- **Sea-Query:** `sea_query::SelectStatement` (internal structure)

### Potential Contribution

This would be a valuable addition to sea-query as it enables more flexible query building patterns. The change would benefit any ORM or query builder that needs to modify SELECT columns while preserving other query clauses.

---

## Future Improvements

*This section will be populated as we discover more limitations and opportunities.*

---

## Contribution Guidelines

When contributing improvements to sea-query:

1. **Check existing issues:** Search sea-query's issue tracker for related requests
2. **Discuss first:** Open a discussion or issue before implementing
3. **Follow patterns:** Match sea-query's existing API design patterns
4. **Add tests:** Include comprehensive tests for new functionality
5. **Update docs:** Ensure documentation is clear and complete
6. **Backward compatibility:** Maintain compatibility with existing code

### Sea-Query Resources

- **Repository:** https://github.com/SeaQL/sea-query
- **Documentation:** https://docs.rs/sea-query
- **Issue Tracker:** https://github.com/SeaQL/sea-query/issues

---

## Notes

- This audit is focused on improvements that would benefit Lifeguard specifically
- Not all improvements may be suitable for upstream contribution
- Some improvements may require significant architectural changes in sea-query
- Priority is based on impact to Lifeguard's functionality
