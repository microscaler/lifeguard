# Entity Definitions

This directory contains Lifeguard entity definitions organized by service, matching the RERP OpenAPI accounting service structure.

## Directory Structure

```
examples/entities/
└── accounting/
    ├── general-ledger/      # Chart of accounts, accounts, journal entries
    ├── invoice/             # Invoices and invoice lines
    ├── accounts-receivable/ # Customer invoices, AR payments, aging
    └── accounts-payable/    # Vendor invoices, AP payments, aging
```

## Service Mapping

### General Ledger (`accounting/general-ledger/`)
Core accounting entities:
- `chart_of_accounts.rs` - Hierarchical chart of accounts structure
- `account.rs` - Individual accounts linked to chart of accounts
- `journal_entry.rs` - Double-entry journal entries
- `journal_entry_line.rs` - Individual debit/credit lines in journal entries
- `account_balance.rs` - Denormalized account balances for performance

### Invoice (`accounting/invoice/`)
Invoice management:
- `invoice.rs` - Customer and vendor invoices
- `invoice_line.rs` - Line items on invoices

### Accounts Receivable (`accounting/accounts-receivable/`)
AR management:
- `customer_invoice.rs` - Customer-facing invoices with AR tracking
- `ar_payment.rs` - Customer payments
- `ar_payment_application.rs` - Links payments to specific invoices
- `ar_aging.rs` - Aging analysis for accounts receivable

### Accounts Payable (`accounting/accounts-payable/`)
AP management:
- `vendor_invoice.rs` - Vendor invoices with AP tracking
- `ap_payment.rs` - Vendor payments
- `ap_payment_application.rs` - Links payments to specific vendor invoices
- `ap_aging.rs` - Aging analysis for accounts payable

## Usage

Entities are automatically discovered by the `lifeguard-migrate generate-from-entities` command, which recursively scans all `.rs` files in this directory structure.

## RERP Integration

This structure matches the RERP OpenAPI accounting service organization (`../rerp/openapi/accounting/`), allowing for:
- Direct mapping between OpenAPI schemas and Lifeguard entities
- Service-based organization for BRRTRouter integration
- Clear separation of concerns by accounting domain
