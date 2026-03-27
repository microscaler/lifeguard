# OpenAPI Spec Update Progress

## ✅ Completed (3/9)
1. **Invoice** - Comprehensive schemas for Invoice and InvoiceLine
2. **Accounts Receivable** - Schemas for CustomerInvoice, ArPayment, ArPaymentApplication, ArAging
3. **Accounts Payable** - Schemas for VendorInvoice, ApPayment, ApPaymentApplication, ApAging

## ⏳ Remaining (6/9)
4. **Bank Sync** - Needs: BankAccount, BankTransaction, BankStatement, BankReconciliation
5. **Asset** - Needs: Asset, AssetCategory, AssetDepreciation, AssetTransaction
6. **Budget** - Needs: Budget, BudgetPeriod, BudgetLineItem, BudgetVersion, BudgetActual
7. **EDI** - Needs: EdiDocument, EdiFormat, EdiMapping, EdiAcknowledgment
8. **Financial Reports** - Needs: FinancialReport, ReportTemplate, ReportSchedule, ReportData
9. **General Ledger** - May need update (already has migrations)

## Schema Pattern
Each service needs:
- Entity schemas (full field definitions)
- Create request schemas (exclude read-only fields)
- Update request schemas (all fields optional)

## Next Steps
Continue updating remaining 6 services systematically.

