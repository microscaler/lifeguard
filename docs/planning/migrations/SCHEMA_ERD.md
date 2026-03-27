# RERP Accounting Database - Entity Relationship Diagram

This document provides a visual representation of the accounting database schema using Mermaid ERD syntax.

## Full Schema ERD

```mermaid
erDiagram
    %% General Ledger Module
    chart_of_accounts ||--o{ chart_of_accounts : "parent_id (self-reference)"
    chart_of_accounts ||--o{ accounts : "has"
    accounts ||--o{ journal_entry_lines : "has"
    accounts ||--o{ account_balances : "has"
    accounts ||--o{ invoice_lines : "uses"
    journal_entries ||--o{ journal_entry_lines : "contains"
    journal_entries ||--o{ invoices : "posts"
    journal_entries ||--o{ ar_payments : "posts"
    journal_entries ||--o{ ap_payments : "posts"
    
    %% Invoice Management Module
    invoices ||--o{ invoice_lines : "contains"
    invoices ||--o{ customer_invoices : "extends"
    invoices ||--o{ vendor_invoices : "extends"
    
    %% Accounts Receivable Module
    customer_invoices ||--o{ ar_payment_applications : "receives"
    ar_payments ||--o{ ar_payment_applications : "applies_to"
    customer_invoices ||--o{ ar_agings : "tracks"
    
    %% Accounts Payable Module
    vendor_invoices ||--o{ ap_payment_applications : "receives"
    ap_payments ||--o{ ap_payment_applications : "applies_to"
    vendor_invoices ||--o{ ap_agings : "tracks"
    
    chart_of_accounts {
        uuid id PK
        varchar code UK
        varchar name
        varchar account_type
        uuid parent_id FK
        integer level
        boolean is_active
        text description
        timestamp created_at
        timestamp updated_at
    }
    
    accounts {
        uuid id PK
        uuid chart_of_account_id FK
        varchar code UK
        varchar name
        varchar account_type
        varchar normal_balance
        varchar currency_code
        boolean is_active
        boolean is_system_account
        text description
        jsonb metadata
        timestamp created_at
        timestamp updated_at
    }
    
    journal_entries {
        uuid id PK
        varchar entry_number UK
        date entry_date
        text description
        varchar reference_number
        varchar source_type
        uuid source_id
        uuid fiscal_period_id
        varchar status
        timestamp posted_at
        uuid posted_by
        numeric total_debit
        numeric total_credit
        varchar currency_code
        uuid company_id
        jsonb metadata
        timestamp created_at
        timestamp updated_at
        uuid created_by
        uuid updated_by
    }
    
    journal_entry_lines {
        uuid id PK
        uuid journal_entry_id FK
        uuid account_id FK
        integer line_number
        text description
        numeric debit_amount
        numeric credit_amount
        varchar currency_code
        numeric exchange_rate
        numeric base_debit_amount
        numeric base_credit_amount
        jsonb metadata
        timestamp created_at
    }
    
    account_balances {
        uuid id PK
        uuid account_id FK
        uuid fiscal_period_id
        date balance_date
        numeric debit_balance
        numeric credit_balance
        numeric net_balance "GENERATED"
        varchar currency_code
        uuid company_id
        timestamp updated_at
    }
    
    invoices {
        uuid id PK
        varchar invoice_number UK
        varchar invoice_type
        date invoice_date
        date due_date
        uuid customer_id
        uuid vendor_id
        varchar status
        numeric subtotal
        numeric tax_amount
        numeric discount_amount
        numeric total_amount
        varchar currency_code
        numeric exchange_rate
        numeric base_total_amount
        varchar payment_terms
        varchar payment_method
        varchar reference_number
        text notes
        text internal_notes
        timestamp approved_at
        uuid approved_by
        timestamp sent_at
        timestamp paid_at
        uuid company_id
        uuid journal_entry_id FK
        jsonb metadata
        timestamp created_at
        timestamp updated_at
        uuid created_by
        uuid updated_by
    }
    
    invoice_lines {
        uuid id PK
        uuid invoice_id FK
        integer line_number
        varchar item_type
        uuid item_id
        text description
        numeric quantity
        numeric unit_price
        numeric discount_percent
        numeric discount_amount
        numeric tax_rate
        numeric tax_amount
        numeric line_total
        varchar currency_code
        uuid account_id FK
        jsonb metadata
        timestamp created_at
    }
    
    customer_invoices {
        uuid id PK
        uuid invoice_id FK "UNIQUE"
        uuid customer_id
        varchar invoice_number UK
        date invoice_date
        date due_date
        varchar status
        numeric subtotal
        numeric tax_amount
        numeric total_amount
        numeric paid_amount
        numeric outstanding_amount "GENERATED"
        varchar currency_code
        varchar payment_terms
        integer days_overdue "GENERATED"
        uuid company_id
        jsonb metadata
        timestamp created_at
        timestamp updated_at
    }
    
    ar_payments {
        uuid id PK
        varchar payment_number UK
        date payment_date
        uuid customer_id
        varchar payment_method
        numeric payment_amount
        varchar currency_code
        numeric exchange_rate
        numeric base_payment_amount
        varchar reference_number
        uuid bank_account_id
        varchar status
        timestamp cleared_at
        text notes
        uuid company_id
        uuid journal_entry_id FK
        jsonb metadata
        timestamp created_at
        timestamp updated_at
        uuid created_by
    }
    
    ar_payment_applications {
        uuid id PK
        uuid payment_id FK
        uuid customer_invoice_id FK
        numeric applied_amount
        numeric discount_taken
        timestamp applied_at
        uuid applied_by
        text notes
    }
    
    ar_agings {
        uuid id PK
        uuid customer_id
        date aging_date
        numeric current_amount
        numeric days_31_60
        numeric days_61_90
        numeric days_over_90
        numeric total_outstanding "GENERATED"
        varchar currency_code
        uuid company_id
        timestamp created_at
    }
    
    vendor_invoices {
        uuid id PK
        uuid invoice_id FK "UNIQUE"
        uuid vendor_id
        varchar invoice_number
        varchar vendor_invoice_number
        date invoice_date
        date due_date
        varchar status
        numeric subtotal
        numeric tax_amount
        numeric total_amount
        numeric paid_amount
        numeric outstanding_amount "GENERATED"
        varchar currency_code
        varchar payment_terms
        uuid purchase_order_id
        uuid receipt_id
        integer days_overdue "GENERATED"
        uuid company_id
        jsonb metadata
        timestamp created_at
        timestamp updated_at
    }
    
    ap_payments {
        uuid id PK
        varchar payment_number UK
        date payment_date
        uuid vendor_id
        varchar payment_method
        numeric payment_amount
        varchar currency_code
        numeric exchange_rate
        numeric base_payment_amount
        varchar reference_number
        uuid bank_account_id
        varchar status
        timestamp cleared_at
        text notes
        uuid company_id
        uuid journal_entry_id FK
        jsonb metadata
        timestamp created_at
        timestamp updated_at
        uuid created_by
    }
    
    ap_payment_applications {
        uuid id PK
        uuid payment_id FK
        uuid vendor_invoice_id FK
        numeric applied_amount
        numeric discount_taken
        timestamp applied_at
        uuid applied_by
        text notes
    }
    
    ap_agings {
        uuid id PK
        uuid vendor_id
        date aging_date
        numeric current_amount
        numeric days_31_60
        numeric days_61_90
        numeric days_over_90
        numeric total_outstanding "GENERATED"
        varchar currency_code
        uuid company_id
        timestamp created_at
    }
```

## Module Breakdown

### 1. General Ledger Module

```mermaid
erDiagram
    chart_of_accounts ||--o{ chart_of_accounts : "parent"
    chart_of_accounts ||--o{ accounts : "organizes"
    accounts ||--o{ journal_entry_lines : "debited/credited"
    accounts ||--o{ account_balances : "tracked"
    journal_entries ||--o{ journal_entry_lines : "contains"
    
    chart_of_accounts {
        uuid id PK
        varchar code UK
        varchar name
        varchar account_type
        uuid parent_id FK
    }
    
    accounts {
        uuid id PK
        uuid chart_of_account_id FK
        varchar code UK
        varchar account_type
        varchar normal_balance
    }
    
    journal_entries {
        uuid id PK
        varchar entry_number UK
        date entry_date
        numeric total_debit
        numeric total_credit
        varchar status
    }
    
    journal_entry_lines {
        uuid id PK
        uuid journal_entry_id FK
        uuid account_id FK
        numeric debit_amount
        numeric credit_amount
    }
    
    account_balances {
        uuid id PK
        uuid account_id FK
        date balance_date
        numeric debit_balance
        numeric credit_balance
    }
```

### 2. Invoice Management Module

```mermaid
erDiagram
    invoices ||--o{ invoice_lines : "contains"
    invoices ||--o{ customer_invoices : "extends"
    invoices ||--o{ vendor_invoices : "extends"
    invoices }o--|| journal_entries : "posts_to"
    accounts ||--o{ invoice_lines : "allocates_to"
    
    invoices {
        uuid id PK
        varchar invoice_number UK
        varchar invoice_type
        date invoice_date
        date due_date
        numeric total_amount
        uuid journal_entry_id FK
    }
    
    invoice_lines {
        uuid id PK
        uuid invoice_id FK
        integer line_number
        numeric quantity
        numeric unit_price
        numeric line_total
        uuid account_id FK
    }
    
    customer_invoices {
        uuid id PK
        uuid invoice_id FK
        uuid customer_id
        numeric outstanding_amount
    }
    
    vendor_invoices {
        uuid id PK
        uuid invoice_id FK
        uuid vendor_id
        numeric outstanding_amount
    }
```

### 3. Accounts Receivable Module

```mermaid
erDiagram
    customer_invoices ||--o{ ar_payment_applications : "receives"
    ar_payments ||--o{ ar_payment_applications : "applies"
    customer_invoices ||--o{ ar_agings : "tracks"
    ar_payments }o--|| journal_entries : "posts_to"
    
    customer_invoices {
        uuid id PK
        uuid invoice_id FK
        uuid customer_id
        numeric total_amount
        numeric paid_amount
        numeric outstanding_amount
    }
    
    ar_payments {
        uuid id PK
        varchar payment_number UK
        uuid customer_id
        numeric payment_amount
        uuid journal_entry_id FK
    }
    
    ar_payment_applications {
        uuid id PK
        uuid payment_id FK
        uuid customer_invoice_id FK
        numeric applied_amount
    }
    
    ar_agings {
        uuid id PK
        uuid customer_id
        date aging_date
        numeric current_amount
        numeric days_31_60
        numeric days_61_90
        numeric days_over_90
    }
```

### 4. Accounts Payable Module

```mermaid
erDiagram
    vendor_invoices ||--o{ ap_payment_applications : "receives"
    ap_payments ||--o{ ap_payment_applications : "applies"
    vendor_invoices ||--o{ ap_agings : "tracks"
    ap_payments }o--|| journal_entries : "posts_to"
    
    vendor_invoices {
        uuid id PK
        uuid invoice_id FK
        uuid vendor_id
        varchar vendor_invoice_number
        numeric total_amount
        numeric paid_amount
        numeric outstanding_amount
    }
    
    ap_payments {
        uuid id PK
        varchar payment_number UK
        uuid vendor_id
        numeric payment_amount
        uuid journal_entry_id FK
    }
    
    ap_payment_applications {
        uuid id PK
        uuid payment_id FK
        uuid vendor_invoice_id FK
        numeric applied_amount
    }
    
    ap_agings {
        uuid id PK
        uuid vendor_id
        date aging_date
        numeric current_amount
        numeric days_31_60
        numeric days_61_90
        numeric days_over_90
    }
```

## Key Relationships

### Hierarchical Relationships
- **chart_of_accounts** → **chart_of_accounts** (self-reference via `parent_id`)
  - Creates hierarchical account structure
  - Supports account rollups and reporting

### One-to-Many Relationships
- **chart_of_accounts** → **accounts** (1:N)
  - One chart of accounts entry can have many accounts
- **journal_entries** → **journal_entry_lines** (1:N)
  - One journal entry contains multiple debit/credit lines
- **invoices** → **invoice_lines** (1:N)
  - One invoice contains multiple line items
- **accounts** → **journal_entry_lines** (1:N)
  - One account can appear in many journal entry lines
- **accounts** → **account_balances** (1:N)
  - One account can have multiple balance snapshots

### One-to-One Relationships
- **invoices** → **customer_invoices** (1:1 via `invoice_id`)
  - Customer invoice extends base invoice
- **invoices** → **vendor_invoices** (1:1 via `invoice_id`)
  - Vendor invoice extends base invoice

### Many-to-Many Relationships (via junction tables)
- **customer_invoices** ↔ **ar_payments** (via `ar_payment_applications`)
  - Multiple payments can apply to one invoice
  - One payment can apply to multiple invoices
- **vendor_invoices** ↔ **ap_payments** (via `ap_payment_applications`)
  - Multiple payments can apply to one invoice
  - One payment can apply to multiple invoices

### Posting Relationships
- **journal_entries** ← **invoices** (via `journal_entry_id`)
  - Invoices post to journal entries when approved
- **journal_entries** ← **ar_payments** (via `journal_entry_id`)
  - AR payments post to journal entries when cleared
- **journal_entries** ← **ap_payments** (via `journal_entry_id`)
  - AP payments post to journal entries when cleared

## Data Flow

### Invoice to Journal Entry Flow
```
Invoice Created → Invoice Approved → Journal Entry Created → Journal Entry Posted
```

### Payment Application Flow
```
Payment Received → Payment Application → Invoice Updated (paid_amount) → Journal Entry Posted
```

### Balance Calculation Flow
```
Journal Entry Posted → Journal Entry Lines → Account Balances Updated → Aging Calculated
```

## Cardinality Summary

| Relationship | Cardinality | Notes |
|-------------|-------------|-------|
| chart_of_accounts → accounts | 1:N | One chart entry, many accounts |
| accounts → journal_entry_lines | 1:N | One account, many transactions |
| journal_entries → journal_entry_lines | 1:N | One entry, many lines |
| invoices → invoice_lines | 1:N | One invoice, many line items |
| invoices → customer_invoices | 1:1 | One-to-one extension |
| invoices → vendor_invoices | 1:1 | One-to-one extension |
| customer_invoices ↔ ar_payments | M:N | Via ar_payment_applications |
| vendor_invoices ↔ ap_payments | M:N | Via ap_payment_applications |
| journal_entries ← invoices | N:1 | Many invoices, one journal entry |
| journal_entries ← ar_payments | N:1 | Many payments, one journal entry |
| journal_entries ← ap_payments | N:1 | Many payments, one journal entry |

## Notes

- **PK** = Primary Key
- **FK** = Foreign Key
- **UK** = Unique Key
- **GENERATED** = Generated column (computed)
- All monetary amounts use `NUMERIC(19, 4)` precision
- All timestamps use `TIMESTAMP` (not `TIMESTAMPTZ`)
- All primary keys are `UUID` with `gen_random_uuid()` default
- Multi-company support via `company_id` fields
- Multi-currency support via `currency_code` and `exchange_rate` fields
