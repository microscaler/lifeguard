# Migration Tool Test Project

This directory has been transformed into a realistic 3rd party project structure to test `lifeguard-migrate` end-to-end.

## Structure Transformation

**Before:**
```
examples/entities/
└── accounting/
    └── general-ledger/
        └── *.rs files
```

**After:**
```
examples/entities/
├── Cargo.toml          # Standalone project
├── src/
│   ├── lib.rs          # Library root
│   └── accounting/
│       ├── mod.rs
│       ├── general_ledger/
│       │   ├── mod.rs
│       │   └── *.rs    # Entity files
│       ├── invoice/
│       ├── accounts_receivable/
│       └── accounts_payable/
└── README.md
```

## Testing Migration Tool

This structure allows us to test the migration tool as if it were processing a real 3rd party project:

```bash
# From lifeguard project root
lifeguard-migrate generate-from-entities \
    --source-dir ./examples/entities/src \
    --output-dir ./migrations/generated
```

## Key Features

1. **Standalone Cargo Project**: Has its own `Cargo.toml` with `[workspace]` to exclude from parent
2. **Proper Rust Module Structure**: Uses standard Rust module organization
3. **Realistic Entity Organization**: Entities organized by service domain
4. **Multiple Services**: Demonstrates how tool handles multiple service directories
5. **Documentation**: Includes README and service mapping documentation

## Purpose

This serves as:
- **Integration Test**: Validates migration tool works with external projects
- **Example**: Shows best practices for organizing Lifeguard entities
- **Documentation**: Demonstrates tool capabilities with real project structure
