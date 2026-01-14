# Lifeguard Codegen

Code generation tool for Lifeguard ORM entities. This tool generates Entity, Model, Column, and related code from entity definitions, avoiding macro expansion ordering issues (like E0223) by generating actual Rust source files.

## Installation

```bash
cargo install --path lifeguard-codegen
```

Or use from the repository:

```bash
cd lifeguard-codegen
cargo build --release
```

## Usage

### Basic Generation

```bash
lifeguard-codegen generate --input <input> --output <output>
```

**Options:**
- `--input, -i`: Input file or directory containing entity definitions
- `--output, -o`: Output directory for generated code (default: `src/entities`)
- `--format, -f`: Format: `expanded` (default) or `compact`

### Defining Entities

**Rust Structs** (idiomatic approach)

```rust
// entities/user.rs
#[table_name = "users"]
pub struct User {
    #[primary_key]
    #[auto_increment]
    pub id: i32,
    
    pub email: String,
    
    pub name: Option<String>,
}
```

Then generate:
```bash
lifeguard-codegen generate --input entities/user.rs --output src/entities
```

### Examples

```bash
# From Rust struct
lifeguard-codegen generate --input entities/user.rs --output src/entities

# From directory of structs
lifeguard-codegen generate --input entities/ --output src/entities
```

## Generated Code Structure

The tool generates:

1. **Entity** - Unit struct implementing `LifeEntityName` and `Iden`
2. **Column** - Enum with variants for each field
3. **PrimaryKey** - Enum with variants for primary key fields
4. **Model** - Struct representing database rows
5. **FromRow** - Implementation for converting `may_postgres::Row` to `Model`
6. **ModelTrait** - Implementation for dynamic column access
7. **LifeModelTrait** - Implementation linking Entity to Model

## Advantages Over Procedural Macros

1. **No E0223 Errors**: Code is generated before compilation, avoiding macro expansion ordering issues
2. **Better Error Messages**: Generated code compiles normally, providing standard Rust error messages
3. **Inspectable Code**: Generated files can be reviewed and debugged
4. **Flexible**: Can generate multiple files, format code, add comments, etc.

## Integration

### Option 1: Manual Generation

Run the tool before compilation:

```bash
lifeguard-codegen generate --input entities/ --output src/entities
cargo build
```

### Option 2: Build Script

Add to `build.rs`:

```rust
fn main() {
    lifeguard_codegen::generate_entities("entities/", "src/entities");
}
```

### Option 3: Pre-commit Hook

Generate entities as part of your development workflow.

## Current Status

- ✅ Basic entity generation working
- ✅ Entity, Model, Column, PrimaryKey generation
- ✅ FromRow and ModelTrait implementations
- ✅ Rust struct parsing (idiomatic approach)
- ✅ Code formatting with rustfmt
- ✅ Type support (i32, i64, i16, u8, u16, u32, u64, f32, f64, bool, String, Option<T>)

## Future Enhancements

- Database schema introspection
- Full type support (all Rust types)
- Incremental generation (only regenerate changed entities)
- Validation and error checking
- LifeRecord generation support