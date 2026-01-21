# Entity Service Mapping

This document maps Lifeguard entities to RERP OpenAPI accounting services, enabling service-based organization for BRRTRouter integration.

## Service Structure

The entity files are organized to match the RERP OpenAPI accounting service structure (`../rerp/openapi/accounting/`):

```
examples/entities/accounting/
├── general-ledger/      # Core accounting entities
├── invoice/             # Invoice management
├── accounts-receivable/ # AR management
└── accounts-payable/    # AP management
```

## Entity to Service Mapping

### General Ledger (`accounting/general-ledger/`)

**Migration:** `20240120120000_create_chart_of_accounts.sql`

| Entity File | Table Name | Description |
|------------|------------|-------------|
| `chart_of_accounts.rs` | `chart_of_accounts` | Hierarchical chart of accounts structure |
| `account.rs` | `accounts` | Individual accounts linked to chart of accounts |
| `journal_entry.rs` | `journal_entries` | Double-entry journal entries |
| `journal_entry_line.rs` | `journal_entry_lines` | Individual debit/credit lines in journal entries |
| `account_balance.rs` | `account_balances` | Denormalized account balances for performance |

**RERP Service:** `general-ledger/`

### Invoice (`accounting/invoice/`)

**Migration:** `20240120130000_create_invoices.sql`

| Entity File | Table Name | Description |
|------------|------------|-------------|
| `invoice.rs` | `invoices` | Customer and vendor invoices |
| `invoice_line.rs` | `invoice_lines` | Line items on invoices |

**RERP Service:** `invoice/`

**Status:** ⚠️ Not yet implemented (entities need to be created)

### Accounts Receivable (`accounting/accounts-receivable/`)

**Migration:** `20240120140000_create_accounts_receivable.sql`

| Entity File | Table Name | Description |
|------------|------------|-------------|
| `customer_invoice.rs` | `customer_invoices` | Customer-facing invoices with AR tracking |
| `ar_payment.rs` | `ar_payments` | Customer payments |
| `ar_payment_application.rs` | `ar_payment_applications` | Links payments to specific invoices |
| `ar_aging.rs` | `ar_agings` | Aging analysis for accounts receivable |

**RERP Service:** `accounts-receivable/`

**Status:** ⚠️ Not yet implemented (entities need to be created)

### Accounts Payable (`accounting/accounts-payable/`)

**Migration:** `20240120150000_create_accounts_payable.sql`

| Entity File | Table Name | Description |
|------------|------------|-------------|
| `vendor_invoice.rs` | `vendor_invoices` | Vendor invoices with AP tracking |
| `ap_payment.rs` | `ap_payments` | Vendor payments |
| `ap_payment_application.rs` | `ap_payment_applications` | Links payments to specific vendor invoices |
| `ap_aging.rs` | `ap_agings` | Aging analysis for accounts payable |

**RERP Service:** `accounts-payable/`

**Status:** ⚠️ Not yet implemented (entities need to be created)

## Implementation Status

- ✅ **General Ledger**: All 5 entities implemented
- ⚠️ **Invoice**: 0/2 entities (invoices, invoice_lines)
- ⚠️ **Accounts Receivable**: 0/4 entities
- ⚠️ **Accounts Payable**: 0/4 entities

## RERP Integration

This structure enables:

1. **Service-based Entity Discovery**: Entities are organized by service, matching RERP OpenAPI structure
2. **BRRTRouter Integration**: Each service directory can map directly to RERP API endpoints
3. **Clear Separation**: Accounting domains are clearly separated for maintainability
4. **Scalability**: New services can be added following the same pattern

## Usage

The `lifeguard-migrate generate-from-entities` command automatically discovers all entities recursively, regardless of their service directory location. This allows:

- Entities to be organized by service
- Multiple services to coexist
- Easy addition of new services
- Service-specific documentation and metadata
