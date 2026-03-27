# Story 19: F() Expressions (Database-Level Expressions)

## Description

Implement F() expressions that allow database-level calculations and column references in queries. This matches Django ORM's F() expressions.

## Acceptance Criteria

- [ ] `F()` expressions for column references
- [ ] Database function calls in expressions
- [ ] Arithmetic operations on columns
- [ ] Comparison operations with expressions
- [ ] Expression chaining
- [ ] Unit tests demonstrate F() expressions

## Technical Details

- F() expression API:
  ```rust
  // Column reference
  let price_doubled = F(User::Price) * 2;
  
  // Database functions
  let upper_email = F(User::Email).upper();
  let length = F(User::Name).length();
  
  // Arithmetic
  let total = F(Order::Price) + F(Order::Tax);
  let discount = F(Order::Price) * 0.9;
  
  // In queries
  Order::find()
      .filter(F(Order::Price) + F(Order::Tax).gt(100))
      .all(&pool)?;
  
  // Updates
  Order::update_many(
      F(Order::Price) * 1.1, // 10% increase
      &pool
  )?;
  ```
- Expression types:
  - `F(Column)` - Column reference
  - `F(Column).function()` - Database function
  - `F(Column) + F(Column)` - Arithmetic
  - `F(Column).gt(value)` - Comparison

## Dependencies

- Story 05: Type-Safe Query Builders
- Story 04: Integrate SeaQuery for SQL Building

## Notes

- F() expressions enable powerful queries
- Database-level calculations
- Matches Django ORM pattern
- Consider adding more database functions

