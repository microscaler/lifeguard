# RERP Accounting Database Migrations

This directory contains database migrations for the RERP accounting system, designed as the first real-world use case for Lifeguard ORM.

## Migration Overview

The migrations create a comprehensive accounting database schema covering:

1. **General Ledger** (`20240120120000_create_chart_of_accounts.sql`)
   - Chart of Accounts (hierarchical structure)
   - Accounts (individual accounts)
   - Journal Entries (double-entry bookkeeping)
   - Journal Entry Lines (debit/credit lines)
   - Account Balances (denormalized for performance)

2. **Invoice Management** (`20240120130000_create_invoices.sql`)
   - Invoices (customer and vendor invoices)
   - Invoice Lines (line items)

3. **Accounts Receivable** (`20240120140000_create_accounts_receivable.sql`)
   - Customer Invoices
   - AR Payments
   - Payment Applications (linking payments to invoices)
   - AR Aging (aging analysis)

4. **Accounts Payable** (`20240120150000_create_accounts_payable.sql`)
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

- All UUIDs use PostgreSQL's `gen_random_uuid()` for primary keys
- Timestamps use `TIMESTAMP` (not `TIMESTAMPTZ`) for simplicity
- Numeric precision: `NUMERIC(19, 4)` for monetary amounts (supports up to 15 digits before decimal, 4 after)
- Generated columns are used for calculated fields (outstanding_amount, days_overdue, etc.)
