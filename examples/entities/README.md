# Accounting Entities Library

A realistic example of a 3rd party project using Lifeguard ORM for accounting domain entities.

## Project Structure

This is a standalone Rust library crate that demonstrates how a real-world project would organize Lifeguard entities:

```
accounting-entities/
├── Cargo.toml          # Project dependencies (Lifeguard, etc.)
├── src/
│   ├── lib.rs          # Library root
│   └── accounting/
│       ├── mod.rs      # Accounting module
│       ├── general_ledger/
│       │   ├── mod.rs
│       │   ├── chart_of_accounts.rs
│       │   ├── account.rs
│       │   ├── journal_entry.rs
│       │   ├── journal_entry_line.rs
│       │   └── account_balance.rs
│       ├── invoice/
│       │   └── mod.rs  # Placeholder for future entities
│       ├── accounts_receivable/
│       │   └── mod.rs  # Placeholder for future entities
│       └── accounts_payable/
│           └── mod.rs  # Placeholder for future entities
└── README.md
```

## Usage

This library can be used as a dependency in other projects:

```toml
[dependencies]
accounting-entities = { path = "../examples/entities" }
```

Then in your code:

```rust
use accounting_entities::accounting::general_ledger::ChartOfAccount;
use lifeguard::LifeModelTrait;

// Access entity metadata
let entity = ChartOfAccount::Entity::default();
println!("Table: {}", entity.table_name());
```

## Entity Organization

Entities are organized by service domain:

- **General Ledger** (`accounting::general_ledger`) - Core accounting entities
  - `ChartOfAccount` - Hierarchical chart of accounts
  - `Account` - Individual accounts
  - `JournalEntry` - Double-entry journal entries
  - `JournalEntryLine` - Journal entry line items
  - `AccountBalance` - Denormalized account balances

- **Invoice** (`accounting::invoice`) - Invoice management (placeholder)
- **Accounts Receivable** (`accounting::accounts_receivable`) - AR management (placeholder)
- **Accounts Payable** (`accounting::accounts_payable`) - AP management (placeholder)

## Migration Generation

This project structure demonstrates how `lifeguard-migrate` can discover and process entities from a 3rd party project:

```bash
# From the lifeguard project root
lifeguard-migrate generate-from-entities \
    --source-dir ./examples/entities/src \
    --output-dir ./migrations/generated
```

The tool will:
1. Recursively scan `src/` for `*.rs` files
2. Discover all `#[derive(LifeModel)]` structs
3. Extract metadata and generate SQL migrations
4. Preserve service structure in output directory

## Building

This is a library crate that can be built independently:

```bash
cd examples/entities
cargo build
```

Note: Some entities use types (like `serde_json::Value`, `rust_decimal::Decimal`) that may not have `FromSql` implementations yet, so `#[skip_from_row]` is used on those entities for SQL generation purposes.

## Purpose

This directory serves as:
1. **Example Project**: Demonstrates realistic entity organization
2. **Test Case**: Used for testing `lifeguard-migrate` entity discovery
3. **Documentation**: Shows best practices for organizing Lifeguard entities
4. **Integration Test**: Validates that the migration tool works with external projects
