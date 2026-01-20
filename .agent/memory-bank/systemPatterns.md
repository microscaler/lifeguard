# System Patterns

## Test Organization Pattern

**Rule:** Tests should be placed alongside the code they test, using `#[cfg(test)] mod tests { ... }` blocks within the same file.

**Rationale:**
- Follows idiomatic Rust conventions
- Makes it easier to maintain tests close to their implementation
- Reduces need for separate test files that can become orphaned
- Tests are co-located with code, making it easier to find and update them together

**Examples:**
- Edge case tests for `PartialModelTrait` are in `src/partial_model.rs`
- Edge case tests for `RelationTrait` are in `src/relation.rs`
- Edge case tests for `SelectQuery` JOIN operations are in `src/query.rs`
- Edge case tests for `ActiveModelBehavior` hooks are in `src/active_model.rs`
- Integration tests for macro-generated code remain in `lifeguard-derive/tests/` (appropriate for integration tests)

**Exception:** Integration tests that test multiple modules together or require special setup can remain in separate test files.

## Development Workflow
1. Use farm tools for all operations (CLI-first development)
2. Always start with `farm agent startup` to load Memory Bank
3. Use `farm git` commands for Git operations
4. Use `farm test` for running tests
5. Use `farm git create-pr` for PR creation
6. **Always respect `./rust-guidelines.txt`** - All code must follow the Rust coding guidelines defined in this file
7. **Track all bugs** - Use the bug tracking system for any bugs discovered (see Bug Tracking Pattern below)

## Code Patterns
- **Always respect `./rust-guidelines.txt`** - All Rust code must follow the guidelines defined in this file
- Follow SeaORM architectural patterns where applicable
- Use procedural macros for code generation
- Comprehensive error handling with detailed error messages
- Type-safe column access via enums

## Procedural Macro Patterns

### Macro Expansion Order
1. **LifeModel** expands first, generating:
   - Entity struct (with `#[derive(DeriveEntity)]`)
   - Model struct
   - Column enum
   - PrimaryKey enum
   - FromRow implementation
   - ModelTrait implementation

2. **DeriveEntity** expands second (nested), generating:
   - LifeModelTrait implementation
   - LifeEntityName implementation
   - Iden implementation
   - IdenStatic implementation
   - Default implementation

### Attribute Patterns
- `#[table_name = "table"]`: Specifies database table name
- `#[model = "ModelName"]`: Specifies Model struct name (optional, defaults to `{Struct}Model`)
- `#[column = "ColumnName"]`: Specifies Column enum name (optional, defaults to `Column` or `{Entity}Column`)
- `#[primary_key]`: Marks field as primary key

### Type Conversion Patterns
- **Extract inner type**: Use `extract_option_inner_type()` for `Option<T>`
- **Match on type string**: Compare `type_name` string for type identification
- **Generate match arms**: Build vectors of match arms, then quote! them together
- **Value conversion**: Convert Rust types to `sea_query::Value` variants

### Code Generation Best Practices
1. **Build incrementally**: Collect match arms, then generate match statement
2. **Use quote! macro**: For code generation, use `quote!` with interpolation
3. **Handle edge cases**: Add fallbacks with warning comments
4. **Type safety**: Use `syn::Ident` for type names, preserve spans
5. **Error messages**: Include detailed context in error messages

### Common Macro Issues & Solutions
- **E0223 (Ambiguous Associated Type)**: Use direct type references instead of `Entity::Column`
- **Name conflicts**: Use separate modules in tests
- **Missing implementations**: Ensure all required traits are generated
- **Type resolution**: Define types before using in quote! blocks

## Testing Patterns
- Test-driven development (TDD)
- Comprehensive edge case coverage
- Separate test modules to avoid name conflicts
- Manual model construction for complex types (JSON)
- **Test Structure**: Create `mod option_tests` and `mod json_tests` to isolate generated types

## Bug Tracking Pattern

**Location:** `.agent/bugs.md` (index) and `.agent/bugs/BUG-YYYY-MM-DD-NN.md` (individual bugs)

**When to Create a Bug:**
- Any bug discovered via Cursor's "Verify this issue exists and fix it" workflow
- Compilation errors that require investigation
- Runtime errors or incorrect behavior
- Test failures that indicate bugs (not just test issues)

**Bug Tracking Workflow:**
1. **Create bug file**: Create `.agent/bugs/BUG-YYYY-MM-DD-NN.md` with:
   - Full description of the issue
   - Root cause analysis
   - Impact assessment
   - Fix description
   - Verification checklist
   - Related files

2. **Update index**: Add entry to `.agent/bugs.md` with:
   - ID (links to detailed bug file)
   - Date, source, status, severity
   - Location (file and line numbers)
   - Brief impact description
   - Hyperlink to detailed bug report

3. **Update statistics**: Update bug counts in `.agent/bugs.md` statistics section

**Bug Status Values:**
- `open`: Bug identified but not yet fixed
- `fixed`: Bug fixed but not yet verified with tests
- `verified`: Bug fixed and verified with passing tests

**Severity Levels:**
- `critical`: Prevents compilation or causes data loss/corruption
- `high`: Breaks core functionality or causes crashes
- `medium`: Breaks non-critical functionality or causes incorrect behavior
- `low`: Minor issues, edge cases, or cosmetic problems

**Naming Convention:**
- Bug files: `BUG-YYYY-MM-DD-NN.md` where NN is sequential number for the day
- Example: `BUG-2025-01-27-01.md` is the first bug discovered on January 27, 2025

**Important:** Every bug from Cursor's "Verify this issue exists and fix it" workflow MUST be tracked in the bug tracking system.

## Documentation Patterns
- Document edge cases and limitations clearly
- Add warnings in generated code
- Update analysis documents with current status
- Mark completed items with ✅
- **Generated Code Comments**: Add warnings for missing primary keys, unknown types
- **Bug Tracking**: All bugs must be documented in `.agent/bugs/` with detailed reports

## JSON Support Pattern
- **Core Feature**: JSON is always enabled, no feature flags
- **Dependencies**: `serde_json = "1.0"` in Cargo.toml
- **sea-query**: Use `features = ["with-json"]`
- **Serialization**: Use `serde_json::to_string()` for query parameters
- **Type Handling**: `serde_json::Value` → `Value::Json(Some(Box::new(v)))`

## Rust Guidelines Compliance

**Critical Rule:** All code must respect `./rust-guidelines.txt`. This file contains project-specific Rust coding standards that must be followed for all code changes.

**Key Guidelines from `rust-guidelines.txt`:**
- **Documentation**: Follow canonical doc sections (Summary, Examples, Errors, Panics, Safety, Abort)
- **First sentence**: Keep summary sentence to ~15 words, one line
- **Module docs**: All public modules must have comprehensive `//!` documentation
- **Error handling**: Use canonical error structs with backtraces (libraries) or anyhow/eyre (applications)
- **Unsafe code**: Only use `unsafe` for valid reasons (novel abstractions, performance, FFI) with proper safety documentation
- **Panics**: Panics mean "stop the program" - not for error communication
- **Testing**: Tests should be alongside code using `#[cfg(test)] mod tests { ... }`
- **Static verification**: Use compiler lints, clippy, rustfmt, cargo-audit, miri
- **Type safety**: Use strong types, avoid primitive obsession
- **API design**: Follow Rust API Guidelines, prefer regular functions over associated functions where appropriate
- **Features**: Must be additive, any combination must work
- **Send/Sync**: Public types should be `Send` for compatibility with async runtimes

**When to Reference:**
- Before writing new code
- When modifying existing code
- During code reviews
- When refactoring

**Enforcement:**
- Code should be checked against `rust-guidelines.txt` before committing
- Violations should be fixed before PR submission
- Guidelines take precedence over general Rust conventions when they conflict
