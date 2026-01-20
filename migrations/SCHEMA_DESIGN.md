# RERP Accounting Database Schema Design

## Overview

This document describes the database schema designed for the RERP accounting system, serving as the first real-world use case for Lifeguard ORM. The schema implements a comprehensive double-entry accounting system with support for:

- General Ledger (Chart of Accounts, Accounts, Journal Entries)
- Invoice Management (Invoices, Invoice Lines)
- Accounts Receivable (Customer Invoices, Payments, AR Aging)
- Accounts Payable (Vendor Invoices, Payments, AP Aging)

## Design Principles

### 1. Double-Entry Bookkeeping
- All journal entries must balance (total_debit = total_credit)
- Each journal entry line must be either debit OR credit (not both, not neither)
- Enforced via CHECK constraints

### 2. Multi-Currency Support
- All monetary fields include `currency_code` (VARCHAR(3), default 'USD')
- Exchange rates stored for conversion
- Base currency amounts calculated and stored

### 3. Multi-Company Support
- `company_id` UUID fields throughout for multi-tenant scenarios
- Allows consolidated reporting across companies
- Supports inter-company transactions

### 4. Audit Trail
- `created_at`, `updated_at` timestamps on all tables
- `created_by`, `updated_by` UUID fields for user tracking
- `posted_at`, `posted_by` for journal entry posting

### 5. Soft Deletes
- `is_active` BOOLEAN flags instead of hard deletes
- Allows historical data retention
- Enables data recovery if needed

### 6. Performance Optimization
- Denormalized `account_balances` table for fast balance queries
- Comprehensive indexing on foreign keys and query patterns
- Generated columns for calculated fields (outstanding_amount, days_overdue)

### 7. Flexibility
- JSONB `metadata` fields for extensibility
- Supports custom fields without schema changes
- Allows integration with external systems

## Schema Structure

### General Ledger Module

#### `chart_of_accounts`
Hierarchical structure for organizing accounts.

**Key Fields:**
- `code`: Unique account code (e.g., "1000", "4000")
- `name`: Account name
- `account_type`: ASSET, LIABILITY, EQUITY, REVENUE, EXPENSE
- `parent_id`: Self-referencing for hierarchy
- `level`: Hierarchy depth (0 = root)

**Use Cases:**
- Organize accounts into logical groups
- Support hierarchical reporting
- Enable account rollups

#### `accounts`
Individual accounts linked to chart of accounts.

**Key Fields:**
- `chart_of_account_id`: Links to chart of accounts
- `code`: Unique account code
- `account_type`: Account classification
- `normal_balance`: DEBIT or CREDIT
- `is_system_account`: Prevents deletion of system accounts

**Use Cases:**
- Track individual GL accounts
- Support account-level reporting
- Enable account-specific rules

#### `journal_entries`
Double-entry bookkeeping records.

**Key Fields:**
- `entry_number`: Unique entry identifier
- `entry_date`: Transaction date
- `status`: DRAFT, POSTED, REVERSED
- `total_debit`, `total_credit`: Must be equal (CHECK constraint)
- `source_type`, `source_id`: Link to source document (invoice, payment, etc.)

**Use Cases:**
- Record all financial transactions
- Maintain audit trail
- Support period closing

#### `journal_entry_lines`
Individual debit/credit lines in journal entries.

**Key Fields:**
- `journal_entry_id`: Parent journal entry
- `account_id`: Account being debited/credited
- `debit_amount`, `credit_amount`: Must be one or the other (CHECK constraint)
- `line_number`: Ordering within entry

**Use Cases:**
- Detail-level transaction recording
- Account-level transaction history
- Support for complex multi-line entries

#### `account_balances`
Denormalized account balances for performance.

**Key Fields:**
- `account_id`: Account
- `fiscal_period_id`: Period
- `balance_date`: Snapshot date
- `debit_balance`, `credit_balance`: Current balances
- `net_balance`: Generated column (debit - credit)

**Use Cases:**
- Fast balance queries (no aggregation needed)
- Historical balance snapshots
- Trial balance generation

### Invoice Management Module

#### `invoices`
Customer and vendor invoices.

**Key Fields:**
- `invoice_number`: Unique invoice identifier
- `invoice_type`: CUSTOMER or VENDOR
- `invoice_date`, `due_date`: Invoice dates
- `customer_id` or `vendor_id`: Party
- `status`: DRAFT, PENDING_APPROVAL, APPROVED, SENT, PAID, OVERDUE, CANCELLED
- `subtotal`, `tax_amount`, `discount_amount`, `total_amount`: Amounts
- `journal_entry_id`: Link to posted GL entry

**Use Cases:**
- Invoice creation and management
- Approval workflows
- Integration with AR/AP modules

#### `invoice_lines`
Line items on invoices.

**Key Fields:**
- `invoice_id`: Parent invoice
- `line_number`: Ordering
- `item_type`, `item_id`: Reference to product/service
- `quantity`, `unit_price`: Pricing
- `discount_percent`, `discount_amount`: Discounts
- `tax_rate`, `tax_amount`: Taxes
- `line_total`: Calculated total
- `account_id`: Revenue/expense account

**Use Cases:**
- Detailed invoice line items
- Product/service tracking
- Revenue/expense allocation

### Accounts Receivable Module

#### `customer_invoices`
Customer-facing invoices with AR tracking.

**Key Fields:**
- `invoice_id`: Link to base invoice
- `customer_id`: Customer
- `paid_amount`: Amount paid
- `outstanding_amount`: Generated column (total - paid)
- `days_overdue`: Generated column (calculated from due_date)

**Use Cases:**
- AR tracking
- Payment application
- Aging analysis

#### `ar_payments`
Customer payments.

**Key Fields:**
- `payment_number`: Unique payment identifier
- `payment_date`: Payment date
- `customer_id`: Customer
- `payment_method`: CASH, CHECK, WIRE, CREDIT_CARD, ACH
- `payment_amount`: Amount
- `status`: PENDING, CLEARED, BOUNCED, REVERSED
- `journal_entry_id`: Link to posted GL entry

**Use Cases:**
- Payment recording
- Payment tracking
- Bank reconciliation

#### `ar_payment_applications`
Links payments to specific invoices.

**Key Fields:**
- `payment_id`: Payment
- `customer_invoice_id`: Invoice
- `applied_amount`: Amount applied
- `discount_taken`: Early payment discount

**Use Cases:**
- Payment allocation
- Partial payment handling
- Discount tracking

#### `ar_agings`
Aging analysis for accounts receivable.

**Key Fields:**
- `customer_id`: Customer
- `aging_date`: Snapshot date
- `current_amount`: 0-30 days
- `days_31_60`: 31-60 days
- `days_61_90`: 61-90 days
- `days_over_90`: Over 90 days
- `total_outstanding`: Generated column (sum of all buckets)

**Use Cases:**
- Aging reports
- Collection prioritization
- Bad debt analysis

### Accounts Payable Module

#### `vendor_invoices`
Vendor invoices with AP tracking.

**Key Fields:**
- `invoice_id`: Link to base invoice
- `vendor_id`: Vendor
- `vendor_invoice_number`: Vendor's invoice number
- `paid_amount`: Amount paid
- `outstanding_amount`: Generated column (total - paid)
- `days_overdue`: Generated column (calculated from due_date)
- `purchase_order_id`, `receipt_id`: Links to procurement

**Use Cases:**
- AP tracking
- Payment scheduling
- Three-way matching

#### `ap_payments`
Vendor payments.

**Key Fields:**
- `payment_number`: Unique payment identifier
- `payment_date`: Payment date
- `vendor_id`: Vendor
- `payment_method`: CASH, CHECK, WIRE, ACH
- `payment_amount`: Amount
- `status`: PENDING, CLEARED, CANCELLED, REVERSED
- `journal_entry_id`: Link to posted GL entry

**Use Cases:**
- Payment processing
- Check printing
- Bank reconciliation

#### `ap_payment_applications`
Links payments to specific vendor invoices.

**Key Fields:**
- `payment_id`: Payment
- `vendor_invoice_id`: Invoice
- `applied_amount`: Amount applied
- `discount_taken`: Early payment discount

**Use Cases:**
- Payment allocation
- Partial payment handling
- Discount tracking

#### `ap_agings`
Aging analysis for accounts payable.

**Key Fields:**
- `vendor_id`: Vendor
- `aging_date`: Snapshot date
- `current_amount`: 0-30 days
- `days_31_60`: 31-60 days
- `days_61_90`: 61-90 days
- `days_over_90`: Over 90 days
- `total_outstanding`: Generated column (sum of all buckets)

**Use Cases:**
- Aging reports
- Payment prioritization
- Cash flow planning

## Indexing Strategy

### Primary Indexes
- All tables have UUID primary keys with default `gen_random_uuid()`
- Unique constraints on business keys (invoice_number, entry_number, etc.)

### Foreign Key Indexes
- All foreign keys are indexed for join performance
- Composite indexes on (source_type, source_id) for polymorphic relationships

### Query Pattern Indexes
- Date-based indexes for time-series queries
- Status indexes for filtering
- Company ID indexes for multi-tenant queries

### Partial Indexes
- `outstanding_amount` indexes with `WHERE outstanding_amount > 0` for active records only

## Data Types

### Monetary Amounts
- `NUMERIC(19, 4)`: Supports up to 15 digits before decimal, 4 after
- Sufficient for most business scenarios
- Precision maintained for financial calculations

### Identifiers
- `UUID`: Primary keys (PostgreSQL `gen_random_uuid()`)
- `VARCHAR(50-100)`: Business keys (invoice_number, entry_number)
- `VARCHAR(3)`: Currency codes (ISO 4217)

### Dates
- `DATE`: Transaction dates, due dates
- `TIMESTAMP`: Created/updated timestamps, posted dates

### Text
- `TEXT`: Descriptions, notes (unlimited length)
- `VARCHAR(255)`: Names, titles (reasonable length limits)

### JSON
- `JSONB`: Metadata fields (indexed, queryable)

## Constraints

### Check Constraints
- `check_balanced_entry`: Journal entries must balance (debit = credit)
- `check_debit_or_credit`: Journal entry lines must be debit OR credit
- `check_positive_quantity`: Quantities must be positive
- `check_positive_unit_price`: Prices must be non-negative
- `check_positive_payment`: Payments must be positive

### Unique Constraints
- Business keys (invoice_number, entry_number, payment_number)
- Composite uniques (account_id, fiscal_period_id, balance_date, currency_code, company_id)

### Foreign Key Constraints
- `ON DELETE RESTRICT`: Prevents deletion of referenced records
- `ON DELETE CASCADE`: Cascades deletion to child records
- `ON DELETE SET NULL`: Sets foreign key to NULL on deletion

## Generated Columns

PostgreSQL generated columns (computed at storage time):
- `outstanding_amount`: total_amount - paid_amount
- `days_overdue`: Calculated from due_date and current date
- `net_balance`: debit_balance - credit_balance
- `total_outstanding`: Sum of aging buckets

## Next Steps

1. **Create Lifeguard Entities**: Use `LifeModel` and `LifeRecord` derives
2. **Implement Business Logic**:
   - Journal entry posting and validation
   - Invoice approval workflows
   - Payment processing
   - Aging calculations
3. **Add Validation**: Business rule enforcement
4. **Add Tests**: Comprehensive test coverage
5. **Performance Tuning**: Query optimization, additional indexes as needed
