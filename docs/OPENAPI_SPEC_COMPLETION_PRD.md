# OpenAPI Spec Completion PRD

## Overview
Complete the remaining 4 RERP OpenAPI accounting service specifications with comprehensive schemas based on the 36 implemented entities.

## Status
- ✅ **COMPLETE**: 9/9 services (100%)
  - ✅ Invoice
  - ✅ Accounts Receivable
  - ✅ Accounts Payable
  - ✅ Bank Sync
  - ✅ Asset
  - ✅ Budget
  - ✅ EDI
  - ✅ Financial Reports
  - ✅ General Ledger

## Requirements

### 1. Budget Service OpenAPI Spec
**File**: `../rerp/openapi/accounting/budget/openapi.yaml`

**Entities to Schema**:
- `Budget` → `Budget` schema
- `BudgetLineItem` → `BudgetLine` schema (API naming)
- `BudgetActual` → `BudgetVariance` schema (API naming)
- `BudgetPeriod` → Add if needed
- `BudgetVersion` → Add if needed

**Required Schemas**:
- `Budget` - Full entity schema
- `BudgetLine` - Full entity schema (maps to BudgetLineItem)
- `BudgetVariance` - Full entity schema (maps to BudgetActual)
- `CreateBudgetRequest` - Exclude read-only fields
- `UpdateBudgetRequest` - All fields optional
- `CreateBudgetLineRequest` - Exclude read-only fields
- `UpdateBudgetLineRequest` - All fields optional
- `CreateBudgetVarianceRequest` - Exclude read-only fields
- `UpdateBudgetVarianceRequest` - All fields optional

**Key Fields**:
- Budget: budget_number, name, fiscal_year, period_start, period_end, status, approval_status, totals
- BudgetLine: budget_id, version_id, account_id, period_id, budget_amount, actual_amount, variance
- BudgetVariance: budget_id, account_id, period_id, budget_amount, actual_amount, variance, variance_percent

### 2. EDI Service OpenAPI Spec
**File**: `../rerp/openapi/accounting/edi/openapi.yaml`

**Entities to Schema**:
- `EdiDocument` → `EdiDocument` schema
- `EdiFormat` → `EdiFormat` schema
- `EdiMapping` → `EdiMapping` schema
- `EdiAcknowledgment` → `EdiAcknowledgment` schema

**Required Schemas**:
- `EdiDocument` - Full entity schema
- `EdiFormat` - Full entity schema
- `EdiMapping` - Full entity schema
- `EdiAcknowledgment` - Full entity schema
- Create/Update request schemas for each entity

**Key Fields**:
- EdiDocument: document_number, document_type, format_id, status, sender_id, receiver_id, raw_content, parsed_data
- EdiFormat: code, name, version, supported_document_types
- EdiMapping: format_id, document_type, field_mappings, transformation_rules
- EdiAcknowledgment: document_id, acknowledgment_type, status, acknowledgment_content

### 3. Financial Reports Service OpenAPI Spec
**File**: `../rerp/openapi/accounting/financial-reports/openapi.yaml`

**Entities to Schema**:
- `FinancialReport` → `FinancialReport` schema
- `ReportTemplate` → `ReportTemplate` schema
- `ReportSchedule` → `ReportSchedule` schema
- `ReportData` → `ReportData` schema

**Required Schemas**:
- `FinancialReport` - Full entity schema
- `ReportTemplate` - Full entity schema
- `ReportSchedule` - Full entity schema
- `ReportData` - Full entity schema
- Create/Update request schemas for each entity

**Key Fields**:
- FinancialReport: report_code, name, report_type, template_id, report_date, status, report_data
- ReportTemplate: template_code, name, report_type, template_structure, formulas
- ReportSchedule: template_id, frequency, next_run_at, status, recipients
- ReportData: report_id, report_date, data, summary, data_version

### 4. General Ledger Service OpenAPI Spec
**File**: `../rerp/openapi/accounting/general-ledger/openapi.yaml`

**Status**: Needs verification - may already have schemas

**Entities to Verify**:
- `ChartOfAccount`
- `Account`
- `JournalEntry`
- `JournalEntryLine`
- `AccountBalance`

**Action**: Verify existing schemas, update if incomplete or missing

## Implementation Pattern

### Schema Structure
Each entity schema follows this pattern:

```yaml
EntityName:
  type: object
  required:
    - id
    - [other required fields]
  properties:
    id:
      type: string
      format: uuid
    [all entity fields with proper types]
```

### Type Mappings
- `uuid::Uuid` → `string` with `format: uuid`
- `String` → `string` (with `maxLength` from `VARCHAR(N)`)
- `Option<T>` → `T` with `nullable: true`
- `i32`, `i64` → `integer`
- `rust_decimal::Decimal` → `number` with `format: decimal`
- `chrono::NaiveDate` → `string` with `format: date`
- `chrono::NaiveDateTime` → `string` with `format: date-time`
- `bool` → `boolean`
- `serde_json::Value` → `object` (JSONB)

### Request Schemas
- **Create Request**: Exclude `id`, `created_at`, `updated_at`, `created_by`, `updated_by`
- **Update Request**: All fields optional (except `id` in path)

### Enum Values
Include all enum values from entity status fields:
- Budget: `DRAFT`, `ACTIVE`, `CLOSED`, `CANCELLED`
- EDI: `RECEIVED`, `PARSING`, `PARSED`, `VALIDATED`, `PROCESSED`, `ERROR`
- Financial Reports: `DRAFT`, `GENERATED`, `APPROVED`, `PUBLISHED`
- etc.

## Acceptance Criteria

1. ✅ All 4 remaining OpenAPI specs have complete schemas
2. ✅ All entity fields are properly mapped to OpenAPI types
3. ✅ Create and Update request schemas are provided for each entity
4. ✅ Enum values match entity definitions
5. ✅ All nullable fields are marked with `nullable: true`
6. ✅ String fields have appropriate `maxLength` constraints
7. ✅ Decimal fields use `format: decimal`
8. ✅ Date/DateTime fields use proper format strings

## Implementation Order

1. ✅ **Budget** - 5 entities, completed
2. ✅ **EDI** - 4 entities, completed
3. ✅ **Financial Reports** - 4 entities, completed
4. ✅ **General Ledger** - Verified and updated

## Success Metrics

- ✅ 9/9 OpenAPI specs complete (100%)
- ✅ All schemas validate against entity definitions
- ✅ All request/response schemas are consistent
- ✅ Documentation is complete and accurate

## Implementation Status

**COMPLETE** - All 9 OpenAPI specifications have been updated with comprehensive schemas.

### Summary
- **36 entities** mapped to OpenAPI schemas
- **All fields** properly typed and constrained
- **Enum values** match entity definitions
- **Nullable fields** properly marked
- **Create/Update requests** for all entities

### Files Updated
All OpenAPI specs in `../rerp/openapi/accounting/`:
- ✅ `invoice/openapi.yaml`
- ✅ `accounts-receivable/openapi.yaml`
- ✅ `accounts-payable/openapi.yaml`
- ✅ `bank-sync/openapi.yaml`
- ✅ `asset/openapi.yaml`
- ✅ `budget/openapi.yaml`
- ✅ `edi/openapi.yaml`
- ✅ `financial-reports/openapi.yaml`
- ✅ `general-ledger/openapi.yaml`

## Notes

- ✅ All entity definitions are complete and ready
- ✅ Pattern established and consistently applied
- ✅ All updates completed systematically
- ✅ Ready for migration generation and API implementation
