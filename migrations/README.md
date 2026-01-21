# Lifeguard Migrations

This directory contains database migrations for the Lifeguard accounting system.

## Directory Structure

```
migrations/
├── original/                    # Original SQL migrations (manually created)
│   └── accounting/              # Organized by service (matches entity structure)
│       ├── general-ledger/
│       │   └── 20240120120000_create_chart_of_accounts.sql
│       ├── invoice/
│       │   └── 20240120130000_create_invoices.sql
│       ├── accounts-receivable/
│       │   └── 20240120140000_create_accounts_receivable.sql
│       └── accounts-payable/
│           └── 20240120150000_create_accounts_payable.sql
│
├── generated/                   # Generated SQL migrations (from Lifeguard entities)
│   └── accounting/              # Organized by service (matches entity structure)
│       ├── general-ledger/
│       │   └── YYYYMMDDHHMMSS_generated_from_entities.sql
│       ├── invoice/
│       ├── accounts-receivable/
│       └── accounts-payable/
│
├── README.md                    # This file
├── SCHEMA_DESIGN.md             # Schema design documentation
└── SCHEMA_ERD.md                # Entity Relationship Diagram
```

## Migration Overview

The migrations create a comprehensive accounting database schema covering:

1. **General Ledger** (`accounting/general-ledger/20240120120000_create_chart_of_accounts.sql`)
   - Chart of Accounts (hierarchical structure)
   - Accounts (individual accounts)
   - Journal Entries (double-entry bookkeeping)
   - Journal Entry Lines (debit/credit lines)
   - Account Balances (denormalized for performance)

2. **Invoice Management** (`accounting/invoice/20240120130000_create_invoices.sql`)
   - Invoices (customer and vendor invoices)
   - Invoice Lines (line items)

3. **Accounts Receivable** (`accounting/accounts-receivable/20240120140000_create_accounts_receivable.sql`)
   - Customer Invoices
   - AR Payments
   - Payment Applications (linking payments to invoices)
   - AR Aging (aging analysis)

4. **Accounts Payable** (`accounting/accounts-payable/20240120150000_create_accounts_payable.sql`)
   - Vendor Invoices
   - AP Payments
   - Payment Applications (linking payments to invoices)
   - AP Aging (aging analysis)

## Schema Design Principles

- **Double-Entry Bookkeeping**: All journal entries must balance (debits = credits)
- **Multi-Currency Support**: All monetary fields include currency_code and exchange_rate
- **Multi-Company Support**: company_id fields throughout for multi-tenant scenarios
- **Audit Trail**: created_at, updated_at, created_by, updated_by fields
- **Soft Deletes**: is_active flags instead of hard deletes where appropriate
- **Performance**: Denormalized account_balances table for fast balance queries
- **Flexibility**: JSONB metadata fields for extensibility

## Chart of Accounts Structure

The chart of accounts is a hierarchical structure for organizing accounts:

### Chart of Accounts Table
- **Hierarchical**: Self-referencing `parent_id` for tree structure
- **Level-based**: `level` field tracks depth in hierarchy (0 = root)
- **Account Types**: ASSET, LIABILITY, EQUITY, REVENUE, EXPENSE
- **Active/Inactive**: `is_active` flag for soft deletes
- **Unique Codes**: `code` field must be unique across all accounts

### Accounts Table
- **Linked to Chart**: Each account references a chart_of_accounts entry
- **Account Types**: Inherits type from chart or can override
- **Normal Balance**: DEBIT or CREDIT (determines balance calculation)
- **Multi-Currency**: `currency_code` field (default: USD)
- **System Accounts**: `is_system_account` flag prevents deletion of critical accounts
- **Metadata**: JSONB field for flexible account-specific data

### Journal Entries
- **Double-Entry**: All entries must balance (total debits = total credits)
- **Entry Number**: Unique identifier for each journal entry
- **Posting Status**: DRAFT, POSTED, REVERSED
- **Audit Fields**: created_by, updated_by for tracking changes

### Journal Entry Lines
- **Debit/Credit**: Each line is either a debit or credit
- **Account Reference**: Links to accounts table
- **Amount**: NUMERIC(19, 4) for monetary precision
- **Currency**: currency_code and exchange_rate for multi-currency support

### Account Balances
- **Denormalized**: Pre-calculated balances for performance
- **Period-based**: Tracks balances by period (month/year)
- **Currency-aware**: Separate balances per currency
- **Auto-updated**: Maintained by triggers or application logic

## Migration Strategy

### Current State: Transitioning to Entity-Driven Generation

We are transitioning from manually-written SQL migrations to **entity-driven migration generation**. This process involves:

1. **Original SQL** (`original/accounting/{service}/`): The existing SQL migrations that were manually created, organized by service
2. **Entity Definitions**: Lifeguard entities (using `#[derive(LifeModel)]`) organized by service in `examples/entities/accounting/{service}/`
3. **Generated SQL** (`generated/accounting/{service}/`): SQL migrations automatically generated by diffing current entities vs previous state, organized by service

### Migration Process

1. **Build Entities**: Create Lifeguard entities that match the SQL in `original/accounting/{service}/`
   - Start with Chart of Accounts entities (general-ledger service)
   - Then Accounts, Journal Entries, etc.
   - Ensure all fields, types, constraints, and indexes match
   - Organize entities by service in `examples/entities/accounting/{service}/`
2. **Generate Migrations**: Run `lifeguard-migrate generate-from-entities` to create SQL in `generated/accounting/{service}/`
   - SQL files are automatically organized by service based on entity location
3. **Compare & Validate**: Diff `original/accounting/{service}/` vs `generated/accounting/{service}/` to ensure they produce equivalent SQL
   - Compare table structures
   - Compare indexes
   - Compare constraints
   - Compare default values
4. **Iterate**: Adjust entities until generated SQL matches original SQL
5. **Finalize**: Once satisfied, use `generated/` as the source of truth

### File Naming Convention

Migrations follow the pattern: `YYYYMMDDHHMMSS_description.sql`

- `YYYYMMDDHHMMSS`: Timestamp (year, month, day, hour, minute, second)
- `description`: Human-readable description (snake_case)

Example: `20240120120000_create_chart_of_accounts.sql`

### Migration Execution

Migrations are executed in order by timestamp. The migration system:
- Discovers all `.sql` files in the migrations directory
- Sorts them by version (timestamp)
- Executes pending migrations that haven't been applied
- Tracks applied migrations in the `lifeguard_tracking.lifeguard_migrations` table

## Running Migrations

These migrations are designed to be run using Lifeguard's migration system:

```bash
# Using the CLI tool
lifeguard-migrate up --migrations-dir ./migrations

# Or programmatically
use lifeguard::migration::startup_migrations;
startup_migrations(&executor, "./migrations", None)?;
```

## Next Steps

1. Create Lifeguard entities using `LifeModel` and `LifeRecord` derives
2. Implement business logic for:
   - Chart of accounts management
   - Journal entry posting and validation
   - Invoice creation and approval workflows
   - Payment processing
   - Aging analysis calculations
3. Add indexes for common query patterns
4. Implement data validation and constraints
5. Add comprehensive tests

## Notes

- **No Rust wrapper files**: We use SQL-only migrations (no `.rs` files wrapping SQL)
- **Entity-driven**: Future migrations are generated from entity diffs
- **SQL is source of truth**: Migrations are readable SQL files that can be executed with `psql`
- All UUIDs use PostgreSQL's `gen_random_uuid()` for primary keys
- Timestamps use `TIMESTAMP` (not `TIMESTAMPTZ`) for simplicity
- Numeric precision: `NUMERIC(19, 4)` for monetary amounts (supports up to 15 digits before decimal, 4 after)
- Generated columns are used for calculated fields (outstanding_amount, days_overdue, etc.)

## Related Documentation

- `SCHEMA_DESIGN.md`: Detailed schema design documentation
- `SCHEMA_ERD.md`: Entity Relationship Diagram
- `../docs/MIGRATION_PROCESS_DIAGRAMS.md`: Migration process diagrams and architecture
