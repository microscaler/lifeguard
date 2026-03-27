# RERP OpenAPI Spec Update Plan

## Overview
Update all RERP OpenAPI accounting service specs with comprehensive schemas based on the 36 entities we've implemented.

## Update Strategy

### Approach
1. **Manual Updates** - Update each OpenAPI spec file with schemas matching our entity definitions
2. **Schema Mapping** - Map Rust entity fields to OpenAPI schema properties
3. **Validation** - Ensure schemas match entity structure exactly

### Schema Mapping Rules

**Rust Types → OpenAPI Types:**
- `uuid::Uuid` → `string` with `format: uuid`
- `String` → `string` (with maxLength from column_type if specified)
- `Option<T>` → `T` with `nullable: true`
- `i32`, `i64` → `integer`
- `rust_decimal::Decimal` → `number` with `format: decimal`
- `chrono::NaiveDate` → `string` with `format: date`
- `chrono::NaiveDateTime` → `string` with `format: date-time`
- `bool` → `boolean`
- `serde_json::Value` → `object` (JSONB)

**Field Attributes:**
- `#[primary_key]` → Required in schemas
- `#[unique]` → Add `uniqueItems: true` if array
- `#[default_value]` → Add `default` in schema
- `#[column_type = "VARCHAR(N)"]` → Add `maxLength: N`
- `#[column_type = "NUMERIC(19, 4)"]` → Use `decimal` format

## Services to Update

1. ✅ **Invoice** - Updated with comprehensive schemas
2. ⏳ **Accounts Receivable** - Needs CustomerInvoice, ArPayment, ArPaymentApplication, ArAging schemas
3. ⏳ **Accounts Payable** - Needs VendorInvoice, ApPayment, ApPaymentApplication, ApAging schemas
4. ⏳ **Bank Sync** - Needs BankAccount, BankTransaction, BankStatement, BankReconciliation schemas
5. ⏳ **Asset** - Needs Asset, AssetCategory, AssetDepreciation, AssetTransaction schemas
6. ⏳ **Budget** - Needs Budget, BudgetPeriod, BudgetLineItem, BudgetVersion, BudgetActual schemas
7. ⏳ **EDI** - Needs EdiDocument, EdiFormat, EdiMapping, EdiAcknowledgment schemas
8. ⏳ **Financial Reports** - Needs FinancialReport, ReportTemplate, ReportSchedule, ReportData schemas
9. ⏳ **General Ledger** - Already has migrations, may need OpenAPI update

## Priority

**High Priority:**
- Accounts Receivable (core business function)
- Accounts Payable (core business function)
- Bank Sync (operational necessity)

**Medium Priority:**
- Asset Management
- Budget
- EDI

**Lower Priority:**
- Financial Reports (can be generated from other services)

## Implementation Notes

- Each service OpenAPI file needs `components.schemas` section populated
- Create/Update request schemas should exclude read-only fields (id, created_at, etc.)
- Update request schemas should make all fields optional
- Include enum values for status fields
- Add descriptions for complex fields

