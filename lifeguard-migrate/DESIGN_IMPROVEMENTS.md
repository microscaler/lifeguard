# Migration Tool Design Improvements

## Current Limitations

1. **Hardcoded Entity Matching** (lines 467-497 in main.rs)
   - Uses match statement with hardcoded table names
   - Not generic - only works for specific entities
   - Requires code changes for each new entity

2. **Static Entity Inclusion** (entities.rs)
   - Entities must be known at compile time via `#[path = "..."]`
   - Cannot handle arbitrary 3rd party entities
   - Requires recompilation for new entities

3. **Single Execution Mode**
   - Only supports `--entities-dir` and `--output-dir` flags
   - No config file support
   - No service tree mapping

4. **No Dynamic Discovery**
   - Cannot infer entities from directory structure
   - Must manually match table names to entity modules

## Design Goals

1. **Generic & Extensible**: Work with any 3rd party entities without code changes
2. **Multiple Execution Modes**: Support both oneshot and config-based execution
3. **Dynamic Discovery**: Automatically infer entities from source files
4. **Better UX**: Clear, intuitive CLI with helpful error messages

## Context: Integrated Rust Tool

**Important Insight**: Lifeguard is a Rust ORM, and users will always:
- Have Lifeguard as a dependency in their `Cargo.toml`
- Have the Rust toolchain available
- Integrate the migration tool into their build/development workflow
- Have their entities already compiled as part of their project

This means we can leverage:
- Existing compilation context
- Cargo's build system
- Proc-macros and build scripts
- The fact that entities are already type-checked and available

## Proposed Architecture

### Option A: Metadata-Based SQL Generation (Recommended)

**Core Idea**: Parse Rust source files to extract entity metadata, then generate SQL directly from metadata without requiring entity compilation.

#### Advantages
- ✅ Fully generic - works with any entities
- ✅ Fast - no compilation step required
- ✅ Works even if user's project has compilation errors (only needs valid syntax)
- ✅ Can run independently of user's build process
- ✅ Simple implementation - just parsing, no complex build integration

#### Disadvantages
- ❌ Must reimplement attribute parsing logic (though syn makes this straightforward)
- ❌ May miss edge cases that the macro system handles automatically
- ❌ Doesn't leverage existing compiled entity types

#### Implementation Approach

1. **Enhanced Entity Parser** (`entity_parser.rs`)
   - Parse `#[derive(LifeModel)]` structs from source files using `syn`
   - Extract all attributes: `#[table_name]`, `#[column_type]`, `#[primary_key]`, etc.
   - Build `EntityMetadata` structure with all information needed for SQL generation
   - Reuse the same parsing logic from `lifeguard-derive` where possible

2. **Metadata-Driven SQL Generator** (`metadata_sql_generator.rs`)
   - Accept `EntityMetadata` instead of requiring `LifeModelTrait`
   - Generate SQL from metadata structures
   - Same output quality as current approach
   - Can share logic with existing `sql_generator.rs`

3. **Dynamic Entity Discovery**
   - Scan directory tree for `*.rs` files
   - Identify entities by `#[derive(LifeModel)]` attribute
   - Extract metadata without compilation

#### Data Structures

```rust
// Entity metadata extracted from source
pub struct EntityMetadata {
    pub struct_name: String,
    pub table_name: String,
    pub schema_name: Option<String>,
    pub table_comment: Option<String>,
    pub columns: Vec<ColumnMetadata>,
    pub primary_keys: Vec<String>,
    pub indexes: Vec<IndexMetadata>,
    pub composite_unique: Vec<Vec<String>>,
    pub check_constraints: Vec<CheckConstraintMetadata>,
    pub foreign_keys: Vec<ForeignKeyMetadata>,
}

pub struct ColumnMetadata {
    pub name: String,
    pub rust_type: String,
    pub column_type: Option<String>,  // From #[column_type]
    pub nullable: bool,
    pub default_value: Option<String>,
    pub default_expr: Option<String>,
    pub unique: bool,
    pub indexed: bool,
    pub auto_increment: bool,
    pub foreign_key: Option<String>,
    pub check: Option<String>,
}

pub struct IndexMetadata {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub partial_where: Option<String>,
}
```

#### Workflow

```
1. User runs: lifeguard-migrate generate-from-entities --source-dir ./src/entities --output-dir ./migrations
2. Tool scans --source-dir recursively for *.rs files
3. For each file:
   a. Parse Rust AST to find #[derive(LifeModel)] structs
   b. Extract all attributes and field metadata
   c. Build EntityMetadata structure
4. Group entities by service path (from directory structure)
5. Generate SQL from metadata for each entity
6. Write SQL files to output directory preserving service structure
```

### Option B: Cargo Build Script Integration

**Core Idea**: Use a Cargo build script that discovers entities at build time, generates a registry module, and compiles it as part of the user's project.

#### Advantages
- ✅ Can use existing `sql_generator` that requires `LifeModelTrait`
- ✅ Leverages existing type system and trait implementations
- ✅ Type-safe at compile time
- ✅ Entities are already compiled as part of user's project
- ✅ No runtime discovery needed - entities known at compile time
- ✅ Natural integration with Cargo workflow

#### Disadvantages
- ❌ Requires build script setup in user's project
- ❌ Must regenerate registry when entities change
- ❌ More complex integration - users must configure build script
- ❌ Slower - requires compilation step
- ❌ May fail if entities have compilation errors

#### Implementation Approach

1. **Build Script** (`build.rs`)
   - Scan user's source directory for entities
   - Generate a registry module that includes all discovered entities
   - Compile as part of user's project

2. **Registry Module** (generated)
   ```rust
   // Generated by build script
   pub mod entity_registry {
       #[path = "path/to/entity1.rs"]
       pub mod entity1;
       // ... more entities
   }
   ```

3. **CLI Tool Integration**
   - Use the compiled registry to access entities
   - Can use existing `sql_generator` with `LifeModelTrait`
   - No parsing needed - entities are already compiled

**Note**: This approach requires users to add a build script to their `Cargo.toml`, which may be acceptable for an integrated tool.

### Option C: Proc-Macro with Registry Pattern

**Core Idea**: Use a proc-macro that automatically registers entities when they're compiled, building a registry at compile time.

#### Advantages
- ✅ Fully automatic - no user configuration needed
- ✅ Type-safe - leverages existing compilation
- ✅ Can use existing `sql_generator` with `LifeModelTrait`
- ✅ No parsing needed - uses compiled entity metadata
- ✅ Works seamlessly with user's existing build process

#### Disadvantages
- ❌ More complex implementation - requires proc-macro changes
- ❌ Registry must be built during compilation
- ❌ May need to handle incremental compilation carefully
- ❌ Requires changes to `lifeguard-derive` to support registration

#### Implementation Approach

1. **Enhanced LifeModel Derive**
   - Add registration code to `#[derive(LifeModel)]` macro
   - Register entity metadata in a compile-time registry
   - Store registry in a generated module

2. **Registry Access**
   - CLI tool reads the compiled registry
   - Can iterate over all registered entities
   - Access entities via their `LifeModelTrait` implementation

3. **CLI Tool**
   - Load registry from compiled artifacts
   - Use existing `sql_generator` with registered entities
   - No discovery or parsing needed

**Note**: This is the most elegant solution but requires changes to the derive macro system.

## Recommended Solution: Option A (Metadata-Based) with Option C as Future Enhancement

**Primary Recommendation**: Option A (Metadata-Based) for immediate implementation because:
- ✅ Fastest to implement
- ✅ No changes needed to existing codebase
- ✅ Works immediately with any project structure
- ✅ No user configuration required
- ✅ Can be enhanced later with Option C

**Future Enhancement**: Option C (Proc-Macro Registry) for long-term because:
- ✅ Most elegant and automatic
- ✅ Leverages existing compilation
- ✅ Type-safe and uses existing SQL generator
- ✅ Zero configuration for users

**Hybrid Approach**: Start with Option A, then add Option C as an optimization that can be enabled via feature flag or automatic detection.

### Implementation Plan

#### Phase 1: Enhanced Entity Parser

**File**: `lifeguard-migrate/src/entity_parser.rs`

```rust
//! Entity parser that extracts metadata from Rust source files
//! without requiring compilation.

use syn::{File, ItemStruct, Attribute, Field};
use std::path::PathBuf;

pub struct EntityParser;

impl EntityParser {
    /// Parse a Rust source file and extract entity metadata
    pub fn parse_file(file_path: &PathBuf) -> Result<Vec<EntityMetadata>, ParseError> {
        // 1. Read file content
        // 2. Parse with syn crate
        // 3. Find #[derive(LifeModel)] structs
        // 4. Extract all attributes
        // 5. Build EntityMetadata
    }
    
    /// Extract table name from attributes
    fn extract_table_name(attrs: &[Attribute]) -> Option<String> { }
    
    /// Extract column metadata from field
    fn extract_column_metadata(field: &Field) -> ColumnMetadata { }
    
    /// Extract table-level attributes
    fn extract_table_attributes(attrs: &[Attribute]) -> TableAttributes { }
}
```

#### Phase 2: Metadata SQL Generator

**File**: `lifeguard-migrate/src/metadata_sql_generator.rs`

```rust
//! SQL generator that works from EntityMetadata instead of LifeModelTrait

pub fn generate_sql_from_metadata(
    metadata: &EntityMetadata
) -> Result<String, String> {
    // Similar logic to current sql_generator.rs
    // but works from metadata structures instead of trait methods
}
```

#### Phase 3: CLI Enhancements

**Enhanced Command Structure**:

```rust
#[derive(Subcommand)]
enum Commands {
    /// Generate SQL migrations from entity definitions
    GenerateFromEntities {
        /// Source directory containing entity files (oneshot mode)
        #[arg(long)]
        source_dir: Option<PathBuf>,
        
        /// Output directory for generated SQL files (oneshot mode)
        #[arg(long)]
        output_dir: Option<PathBuf>,
        
        /// Config file path (config-based mode)
        #[arg(long)]
        config: Option<PathBuf>,
        
        /// Service filter - only generate for specific service
        #[arg(long)]
        service: Option<String>,
        
        /// Dry run - show what would be generated without writing files
        #[arg(long)]
        dry_run: bool,
        
        /// Verbose output - show detailed parsing information
        #[arg(long)]
        verbose: bool,
    },
    // ... other commands
}
```

#### Phase 4: Config File Support

**Config File Format** (`lifeguard-migrate.toml`):

```toml
[migration]
# Entity source tree mapping
# Maps service paths to entity source directories
entity_source_tree = {
    "accounting/general-ledger" = "./src/entities/accounting/general-ledger",
    "accounting/accounts-payable" = "./src/entities/accounting/accounts-payable",
    "accounting/accounts-receivable" = "./src/entities/accounting/accounts-receivable",
    "accounting/invoice" = "./src/entities/accounting/invoice",
}

# Migration output tree mapping
# Maps service paths to migration output directories
migration_output_tree = {
    "accounting/general-ledger" = "./migrations/generated/accounting/general-ledger",
    "accounting/accounts-payable" = "./migrations/generated/accounting/accounts-payable",
    "accounting/accounts-receivable" = "./migrations/generated/accounting/accounts-receivable",
    "accounting/invoice" = "./migrations/generated/accounting/invoice",
}

# Default output directory (used if service not in mapping)
default_output_dir = "./migrations/generated"

# Entity discovery settings
[discovery]
# File patterns to include (default: ["*.rs"])
include_patterns = ["*.rs"]

# File patterns to exclude
exclude_patterns = ["*_test.rs", "*_tests.rs", "mod.rs", "lib.rs", "main.rs"]

# Directories to exclude
exclude_dirs = ["target", ".git", "node_modules"]
```

### Execution Modes

#### Mode 1: Oneshot Execution

```bash
lifeguard-migrate generate-from-entities \
    --source-dir ./src/entities \
    --output-dir ./migrations/generated
```

**Behavior**:
- Scans `--source-dir` recursively for entities
- Groups by directory structure (service path)
- Writes SQL files to `--output-dir` preserving structure

#### Mode 2: Config-Based Execution

```bash
lifeguard-migrate generate-from-entities --config ./lifeguard-migrate.toml
```

**Behavior**:
- Reads config file
- For each service in `entity_source_tree`:
  - Discover entities in source directory
  - Generate SQL
  - Write to corresponding `migration_output_tree` directory
- If service not in mapping, use `default_output_dir`

#### Mode 3: Service-Specific Execution

```bash
lifeguard-migrate generate-from-entities \
    --config ./lifeguard-migrate.toml \
    --service accounting/general-ledger
```

**Behavior**:
- Only process entities for specified service
- Useful for incremental updates

### Entity Discovery Algorithm

```
1. Start at source directory (or config-specified directories)
2. Recursively scan for *.rs files
3. For each .rs file:
   a. Parse with syn crate
   b. Look for structs with #[derive(LifeModel)]
   c. Extract:
      - Struct name
      - #[table_name] attribute → table_name
      - #[schema_name] attribute → schema_name
      - #[table_comment] attribute → table_comment
      - All fields → columns
      - Field attributes → column metadata
      - Table-level attributes → indexes, constraints
4. Build EntityMetadata for each discovered entity
5. Group by service path (directory structure relative to source root)
```

### Error Handling & UX Improvements

#### Clear Error Messages

```rust
// Instead of: "Unknown entity table: xyz (skipping)"
// Provide:
Error: Failed to parse entity in src/entities/user.rs:42:5
  └─ Missing #[table_name] attribute on struct User
  └─ Hint: Add #[table_name = "users"] above the struct definition
```

#### Validation

- ✅ Verify all entities have `#[table_name]`
- ✅ Check for duplicate table names
- ✅ Validate column types are valid SQL types
- ✅ Warn about missing primary keys
- ✅ Check foreign key references exist

#### Progress Indicators

```
🔍 Discovering entities...
   📁 Scanning ./src/entities...
   ✅ Found 12 entity files

📋 Parsing entities...
   ✅ chart_of_accounts.rs → ChartOfAccount (table: chart_of_accounts)
   ✅ account.rs → Account (table: accounts)
   ⚠️  user.rs → Warning: No primary key found
   ❌ invalid.rs → Error: Missing #[table_name] attribute

🔨 Generating SQL...
   ✅ Generated SQL for chart_of_accounts
   ✅ Generated SQL for accounts
   ⏭️  Skipped user (has errors)
   ⏭️  Skipped invalid (has errors)

📊 Summary:
   ✅ 10 entities processed successfully
   ⚠️  1 entity with warnings
   ❌ 1 entity with errors
   📁 4 SQL files written to ./migrations/generated
```

### Migration Path

1. **Phase 1**: Implement metadata parser (can coexist with current approach)
2. **Phase 2**: Implement metadata SQL generator
3. **Phase 3**: Add config file support
4. **Phase 4**: Update CLI with new modes
5. **Phase 5**: Deprecate old hardcoded approach
6. **Phase 6**: Remove hardcoded entity matching

### Benefits

1. **Generic**: Works with any entities without code changes
2. **Fast**: No compilation step required (faster than Option B/C)
3. **Flexible**: Multiple execution modes
4. **User-Friendly**: Clear errors, progress indicators, validation
5. **Maintainable**: No hardcoded entity lists
6. **Extensible**: Easy to add new features (e.g., diff generation, validation rules)
7. **Works with Broken Builds**: Can generate migrations even if user's project has compilation errors (only needs valid syntax)
8. **No User Configuration**: Works out of the box without requiring build script setup

### Dependencies

- `syn` - Rust AST parsing
- `toml` - Config file parsing (already in Cargo.toml)
- `walkdir` - Recursive directory traversal (or use std::fs)

### Testing Strategy

1. **Unit Tests**: Test parser with various entity definitions
2. **Integration Tests**: Test full workflow with sample entities
3. **Edge Cases**: 
   - Entities with all attribute types
   - Nested modules
   - Multiple entities per file
   - Invalid syntax handling

## Alternative: Keep Current Approach but Make It Dynamic

If Option A is too complex, we could:

1. **Remove hardcoded match statement** - Use a registry pattern
2. **Dynamic module loading** - Use `include!()` macro with generated code
3. **Build script** - Generate `entities.rs` from discovered files

However, this still requires compilation and is less elegant than Option A.

## Implementation Strategy

### Phase 1: Option A (Metadata-Based) - Immediate

**Why Start Here**:
- Fastest path to a working solution
- No changes to existing codebase needed
- Works with any project structure
- Can be implemented and tested independently

**Implementation Steps**:
1. Add `syn` dependency for Rust AST parsing
2. Implement entity parser that extracts metadata from source files
3. Create metadata-based SQL generator
4. Update CLI with new execution modes
5. Add config file support

### Phase 2: Option C (Proc-Macro Registry) - Future Enhancement

**Why Add Later**:
- More elegant long-term solution
- Leverages existing compilation
- Can use existing SQL generator without modification
- Automatic - no discovery needed

**Implementation Steps**:
1. Enhance `lifeguard-derive` to support entity registration
2. Generate registry module during compilation
3. Update CLI to use registry when available
4. Fall back to Option A if registry not found

### Hybrid Approach

The tool can support both approaches:
- **Automatic Detection**: Check if registry exists (Option C), fall back to parsing (Option A)
- **User Choice**: Allow `--use-registry` flag to force registry mode
- **Best of Both**: Use registry when available (faster, type-safe), use parsing as fallback (works always)

## Recommendation

**Implement Option A (Metadata-Based) first** because:
- ✅ Fully generic solution
- ✅ Fast implementation path
- ✅ Better UX than current hardcoded approach
- ✅ More maintainable long-term
- ✅ Can be implemented incrementally
- ✅ Works even when user's project has compilation errors
- ✅ No user configuration required

**Plan Option C (Proc-Macro Registry) as future enhancement** because:
- ✅ Most elegant long-term solution
- ✅ Leverages existing compilation context
- ✅ Can be added without breaking changes
- ✅ Provides automatic entity discovery

The parsing approach is well-established (syn crate is mature) and the SQL generation logic can be adapted to work from metadata structures. Since users always have Lifeguard as a dependency, we can also leverage that context for future optimizations.
