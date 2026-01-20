# Complete Attribute Integration: select_as, save_as, and comment

## Summary

This PR completes the implementation of three critical column attributes (`select_as`, `save_as`, and `comment`) and adds comprehensive edge case validations. These features enable advanced query building, custom CRUD expressions, and column documentation support, fulfilling key promises in the README.

## Features Implemented

### 1. select_as - Custom SELECT Expressions ✅

Enables custom SQL expressions in SELECT queries, supporting computed columns, virtual columns, and complex SELECT logic.

**Implementation:**
- Integrated into `SelectQuery::new()` to use custom expressions when specified
- Added `Column::all_columns()` method to iterate over all column variants
- Added `LifeModelTrait::all_columns()` trait method
- Uses `Expr::cust()` for custom expressions, falls back to `Asterisk` when none present
- Handles mixed scenarios (some columns with `select_as`, some without)

**Example:**
```rust
#[derive(LifeModel)]
#[table_name = "users"]
pub struct User {
    #[primary_key]
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
    #[select_as = "CONCAT(first_name, ' ', last_name) AS full_name"]
    pub full_name: String,
}
```

### 2. save_as - Custom Save Expressions ✅

Enables custom SQL expressions in INSERT and UPDATE operations, supporting database-generated values, computed columns, and custom save logic.

**Implementation:**
- Integrated into `ActiveModel::insert()` - uses `Expr::cust()` for custom expressions
- Integrated into `ActiveModel::update()` - uses `Expr::cust()` for custom expressions in SET clauses
- Added `Column::column_save_as()` helper method
- Changed INSERT to use `Vec<Expr>` instead of `Vec<Value>` to support expressions
- Works with auto-increment primary keys (expressions override database generation when values are set)

**Example:**
```rust
#[derive(LifeModel)]
#[table_name = "posts"]
pub struct Post {
    #[primary_key]
    pub id: i32,
    pub title: String,
    #[save_as = "NOW()"]
    pub updated_at: String,
}
```

### 3. comment - Column Documentation ✅

Enables column comments for database schema documentation and introspection.

**Implementation:**
- Added `ColumnDefinition::comment_sql()` method to generate `COMMENT ON COLUMN` SQL
- Handles schema-qualified table names and escapes special characters
- Proper escaping of single quotes and backslashes for PostgreSQL compatibility

**Example:**
```rust
#[derive(LifeModel)]
#[table_name = "users"]
pub struct User {
    #[primary_key]
    pub id: i32,
    #[comment = "User's full name"]
    pub name: String,
}
```

## Edge Case Validations

### 1. Empty String Validation ✅

**Problem:** Empty strings in `select_as` and `save_as` would generate invalid SQL.

**Solution:**
- Added compile-time validation in `parse_column_attributes()`
- Returns `Result<ColumnAttributes, syn::Error>` with clear error messages
- UI tests verify compile errors are emitted correctly

**Error Message:**
```
error: Empty string not allowed in select_as attribute. select_as must contain a valid SQL expression.
```

### 2. Expression Length Limits ✅

**Problem:** Very long expressions (>1MB) could cause memory issues with `get_static_expr()` caching.

**Solution:**
- Added 64KB limit validation in `parse_column_attributes()`
- Compile-time validation with clear error messages
- Prevents memory issues while allowing reasonable expression sizes

### 3. Backslash Escaping in Comments ✅

**Problem:** Backslashes in comments weren't escaped, potentially causing issues in PostgreSQL.

**Solution:**
- Updated `comment_sql()` to escape backslashes as `\\` before single quote escaping
- Order matters: backslashes first, then single quotes
- Tests verify proper escaping behavior

### 4. Identifier Validation ✅

**Problem:** Table/column names in `comment_sql()` could potentially be used for SQL injection.

**Solution:**
- Added `validate_identifier()` helper function
- Checks for dangerous characters (`'`, `"`, `;`, `\`) and SQL keywords
- Emits warnings for invalid identifiers (defensive measure)
- Since identifiers come from macro-generated code, this is primarily defensive

### 5. save_as on Auto-Increment PK Documentation ✅

**Problem:** Behavior of `save_as` on auto-increment primary keys with RETURNING clause was unclear.

**Solution:**
- Comprehensive documentation in `SAVE_AS_AUTO_INCREMENT_PK.md`
- Code comments added explaining behavior
- Documents that expressions override database auto-increment when values are set

## Code Changes

### Core Files Modified

1. **`lifeguard-derive/src/attributes.rs`**:
   - Added empty string and length validation for `select_as` and `save_as`
   - Changed `parse_column_attributes()` to return `Result<ColumnAttributes, syn::Error>`
   - Validates expressions at compile-time

2. **`lifeguard-derive/src/macros/life_model.rs`**:
   - Added `Column::all_columns()` method generation
   - Added `Column::column_save_as()` helper method generation
   - Generates static array of all non-ignored column variants

3. **`lifeguard-derive/src/macros/entity.rs`**:
   - Added `LifeModelTrait::all_columns()` implementation
   - Calls generated `Column::all_columns()` method

4. **`lifeguard-derive/src/macros/life_record.rs`**:
   - Integrated `save_as` into INSERT operations (uses `Expr::cust()` for custom expressions)
   - Integrated `save_as` into UPDATE operations (uses `Expr::cust()` in SET clauses)
   - Added documentation comments for save_as on auto-increment PKs

5. **`src/query/traits.rs`**:
   - Added `all_columns()` method to `LifeModelTrait`

6. **`src/query/select.rs`**:
   - Integrated `select_as` into `SelectQuery::new()`
   - Checks if any column has `select_as` using `E::all_columns()`
   - Uses `Expr::cust()` for custom expressions, `IntoColumnRef` for regular columns

7. **`src/query/column/definition.rs`**:
   - Made `get_static_expr()` public for use in `SelectQuery`
   - Added `comment_sql()` method with proper escaping
   - Added `validate_identifier()` helper function

### New Files Created

1. **`lifeguard-derive/EDGE_CASES_ATTRIBUTES.md`**:
   - Comprehensive edge case documentation
   - Test coverage summary
   - Implementation status tracking

2. **`lifeguard-derive/SAVE_AS_AUTO_INCREMENT_PK.md`**:
   - Detailed documentation of save_as behavior on auto-increment PKs
   - Use cases and recommendations

3. **`lifeguard-derive/tests/ui/compile_error_select_as_empty_string.rs`**:
   - UI test for empty string validation in select_as

4. **`lifeguard-derive/tests/ui/compile_error_save_as_empty_string.rs`**:
   - UI test for empty string validation in save_as

## Testing

### Test Coverage

- **56+ tests** in `test_column_attributes.rs` covering:
  - Basic attribute functionality
  - Edge cases (empty strings, special characters, Option<T> fields)
  - Integration scenarios (multiple attributes, ignored fields)
  - All combinations of select_as, save_as, and comment

- **30 UI tests** including:
  - Compile error tests for empty string validation
  - All existing UI tests continue to pass

### Test Results

```
✅ 56 tests passing in test_column_attributes.rs
✅ 30 tests passing in UI tests
✅ All code compiles without errors
```

## Benefits

1. **Query Builder Completeness** - `select_as` enables advanced SELECT expressions
2. **CRUD Operations Completeness** - `save_as` enables custom INSERT/UPDATE expressions
3. **Developer Experience** - `comment` enables column documentation
4. **Type Safety** - Compile-time validation prevents invalid SQL generation
5. **Memory Safety** - Expression length limits prevent memory issues
6. **Security** - Identifier validation provides defensive SQL injection prevention

## Breaking Changes

⚠️ **Empty String Validation is a Breaking Change**

Code that previously compiled with empty strings in `select_as` or `save_as` will now fail to compile:

```rust
// ❌ This will now fail to compile
#[select_as = ""]
pub name: String,

// ✅ This is valid
#[select_as = "UPPER(name)"]
pub name: String,
```

This is intentional and prevents invalid SQL generation. Users should update their code to provide valid SQL expressions.

## Example Usage

### Complete Example

```rust
use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "users"]
pub struct User {
    #[primary_key]
    #[auto_increment]
    pub id: i32,
    
    pub first_name: String,
    pub last_name: String,
    
    #[select_as = "CONCAT(first_name, ' ', last_name) AS full_name"]
    pub full_name: String,
    
    #[save_as = "NOW()"]
    #[comment = "Timestamp when user was created"]
    pub created_at: String,
    
    #[save_as = "NOW()"]
    pub updated_at: String,
}

// Usage
let users = User::find()
    .filter(User::Column::Id.eq(1))
    .all(&executor)?;

let mut new_user = UserActiveModel::default();
new_user.first_name = Some("John".to_string());
new_user.last_name = Some("Doe".to_string());
// created_at and updated_at will use NOW() expression
let saved_user = new_user.insert(&executor)?;
```

## Related Documentation

- `lifeguard-derive/EDGE_CASES_ATTRIBUTES.md` - Comprehensive edge case coverage
- `lifeguard-derive/SAVE_AS_AUTO_INCREMENT_PK.md` - save_as behavior documentation
- `lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md` - Feature tracking (sections 600-617)

## Impact

### Feature Completeness

- ✅ **Query Builder** - Advanced SELECT expressions now supported
- ✅ **CRUD Operations** - Custom INSERT/UPDATE expressions now supported
- ✅ **Migrations** - Column comments now supported for documentation
- ✅ **Developer Experience** - Better error messages and validation

### Impact Score

- **select_as**: 🟠 **HIGH** - Enables promised "Query Builder" advanced features
- **save_as**: 🟠 **HIGH** - Enables promised "CRUD Operations" completeness
- **comment**: 🟡 **MEDIUM** - Improves developer experience and documentation

**Overall Impact:** 🟠 **HIGH** - Completes critical attribute features promised in README

## Status

All features are:
- ✅ **Implemented** - Full functionality available
- ✅ **Tested** - Comprehensive test coverage
- ✅ **Documented** - Edge cases and behavior documented
- ✅ **Validated** - Compile-time validations prevent common errors

This PR completes the attribute integration work and enables advanced ORM features as promised in the README.
