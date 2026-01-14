# Lifeguard Codegen Usage Guide

## Quick Start

### 1. Build the Tool

```bash
cd lifeguard-codegen
cargo build --release
```

### 2. Generate Entity Code

```bash
./target/release/lifeguard-codegen generate --input . --output src/entities
```

This generates a `user.rs` file in `src/entities/` with a complete User entity.

### 3. Use Generated Code

Add the generated file to your project:

```rust
// In your main.rs or lib.rs
mod entities;
pub use entities::user::*;
```

## Generated Code Structure

The generated code includes:

- **Entity** (`User`): Unit struct implementing `LifeEntityName`
- **Column** enum: Variants for each field (`Id`, `Email`, `Name`)
- **PrimaryKey** enum: Variants for primary key fields
- **Model** (`UserModel`): Struct with all fields
- **FromRow**: Converts `may_postgres::Row` to `UserModel`
- **ModelTrait**: Dynamic column access
- **LifeModelTrait**: Links Entity to Model with `type Column = Column`

## Key Advantage: No E0223

The generated code sets `type Column = Column` directly in the `LifeModelTrait` implementation:

```rust
impl LifeModelTrait for User {
    type Model = UserModel;
    type Column = Column;  // ✅ No E0223 - Column is already defined
}
```

This works because:
1. Code is generated before compilation
2. All types are defined in the same file
3. No nested macro expansion
4. Normal Rust type resolution applies

## Comparison: Codegen vs Proc-Macro

| Aspect | Codegen | Proc-Macro |
|--------|---------|------------|
| E0223 Errors | ✅ None | ❌ Present |
| Type Resolution | ✅ Full | ⚠️ Limited |
| Error Messages | ✅ Standard Rust | ⚠️ Macro expansion |
| Code Inspection | ✅ Yes | ❌ No |
| Build Integration | ⚠️ Manual step | ✅ Automatic |

## Next Steps

1. **LifeRecord Support**: Generate Record structs with from_model/to_model methods
2. **Type Support**: Add support for all Rust types (currently supports common types)
3. **Build Integration**: Add build.rs support for automatic generation
4. **Database Introspection**: Generate entities directly from database schema
