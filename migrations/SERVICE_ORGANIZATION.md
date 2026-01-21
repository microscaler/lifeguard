# Migration Service Organization

Migrations are organized by service to match the entity structure and RERP OpenAPI accounting services.

## Directory Structure

```
migrations/
├── original/                    # Original SQL migrations (manually created)
│   └── accounting/              # Organized by service
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
│   └── accounting/              # Organized by service (auto-generated)
│       ├── general-ledger/
│       │   └── YYYYMMDDHHMMSS_generated_from_entities.sql
│       ├── invoice/
│       ├── accounts-receivable/
│       └── accounts-payable/
│
├── README.md                    # Main migration documentation
├── SCHEMA_DESIGN.md             # Schema design documentation
└── SCHEMA_ERD.md                # Entity Relationship Diagram
```

## Service Mapping

### General Ledger (`accounting/general-ledger/`)
- **Original:** `20240120120000_create_chart_of_accounts.sql`
- **Entities:** `examples/entities/accounting/general-ledger/`
- **Tables:**
  - `chart_of_accounts`
  - `accounts`
  - `journal_entries`
  - `journal_entry_lines`
  - `account_balances`

### Invoice (`accounting/invoice/`)
- **Original:** `20240120130000_create_invoices.sql`
- **Entities:** `examples/entities/accounting/invoice/` (to be created)
- **Tables:**
  - `invoices`
  - `invoice_lines`

### Accounts Receivable (`accounting/accounts-receivable/`)
- **Original:** `20240120140000_create_accounts_receivable.sql`
- **Entities:** `examples/entities/accounting/accounts-receivable/` (to be created)
- **Tables:**
  - `customer_invoices`
  - `ar_payments`
  - `ar_payment_applications`
  - `ar_agings`

### Accounts Payable (`accounting/accounts-payable/`)
- **Original:** `20240120150000_create_accounts_payable.sql`
- **Entities:** `examples/entities/accounting/accounts-payable/` (to be created)
- **Tables:**
  - `vendor_invoices`
  - `ap_payments`
  - `ap_payment_applications`
  - `ap_agings`

## Benefits

1. **Service Alignment**: Matches RERP OpenAPI accounting service structure
2. **BRRTRouter Integration**: Each service directory maps directly to API endpoints
3. **Clear Organization**: Easy to find migrations for a specific service
4. **Scalability**: New services can be added following the same pattern
5. **Entity Mapping**: Entity files and generated SQL are in corresponding locations

## Usage

The `lifeguard-migrate generate-from-entities` command automatically:
- Discovers entities recursively from `examples/entities/`
- Groups entities by service path
- Generates SQL files in corresponding service directories under `migrations/generated/accounting/{service}/`

Example:
```bash
# Generates: migrations/generated/accounting/general-ledger/YYYYMMDDHHMMSS_generated_from_entities.sql
lifeguard-migrate generate-from-entities
```
