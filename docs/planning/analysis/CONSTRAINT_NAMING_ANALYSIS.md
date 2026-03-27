# Constraint Naming Strategy Analysis

## Current State

### Existing Patterns in Codebase

**Indexes (All migrations):**
- Pattern: `idx_{table}_{column(s)}`
- Examples:
  - `idx_journal_entries_entry_number`
  - `idx_chart_of_accounts_parent_id`
  - `idx_account_balances_company_id`
- **Conclusion:** Table-based, but descriptive (includes column names)

**CHECK Constraints (Original SQL):**
- Pattern: Descriptive business rule names
- Examples:
  - `check_balanced_entry` (journal_entries)
  - `check_debit_or_credit` (journal_entry_lines)
  - `check_positive_payment` (ar_payments, ap_payments)
  - `check_positive_quantity` (invoice_lines)
  - `check_positive_unit_price` (invoice_lines)
  - `check_positive_applied_amount` (ar_payment_applications, ap_payment_applications)
- **Conclusion:** Descriptive, business-rule focused

**Generated SQL (Table-based):**
- Pattern: `check_{table_name}`
- Examples:
  - `check_journal_entries`
  - `check_journal_entry_lines`
- **Conclusion:** Table-based, less descriptive

## Analysis

### Option 1: Descriptive Names (Current Original SQL)
**Examples:** `check_balanced_entry`, `check_debit_or_credit`

**Pros:**
1. ✅ **Self-documenting** - Name explains the business rule
2. ✅ **Better error messages** - "violates constraint check_balanced_entry" is clearer than "violates constraint check_journal_entries"
3. ✅ **Easier debugging** - Developers immediately understand what failed
4. ✅ **Consistent with existing migrations** - All other CHECK constraints use descriptive names
5. ✅ **Matches index pattern philosophy** - Indexes are descriptive (`idx_{table}_{column}`)
6. ✅ **Migration-friendly** - Predictable names across environments
7. ✅ **Schema comparison** - Easier to diff schemas when names are meaningful

**Cons:**
1. ❌ **Manual effort** - Requires specifying names in entity attributes
2. ❌ **Potential inconsistency** - If not enforced, names might vary
3. ❌ **Refactoring overhead** - If constraint logic changes, name might need updating

### Option 2: Table-Based Names (Generated Default)
**Examples:** `check_journal_entries`, `check_journal_entry_lines`

**Pros:**
1. ✅ **Automatic** - No manual naming required
2. ✅ **Consistent generation** - Always follows same pattern
3. ✅ **Less maintenance** - No need to think about names

**Cons:**
1. ❌ **Less informative** - Doesn't explain what the constraint does
2. ❌ **Poor error messages** - "violates constraint check_journal_entries" doesn't tell you what rule failed
3. ❌ **Inconsistent with existing codebase** - All other CHECK constraints are descriptive
4. ❌ **Inconsistent with index pattern** - Indexes are descriptive, constraints would be generic
5. ❌ **Harder to debug** - Must look up constraint definition to understand failure
6. ❌ **Multiple constraints per table** - If a table has multiple CHECK constraints, table-based names become ambiguous

## Recommendation: **Keep Descriptive Names**

### Rationale

1. **Codebase Consistency**
   - All existing CHECK constraints use descriptive names
   - Indexes use descriptive pattern (`idx_{table}_{column}`)
   - Maintaining consistency is important for developer experience

2. **Error Message Clarity**
   - When a constraint violation occurs, the name appears in the error
   - `check_balanced_entry` immediately tells you the entry isn't balanced
   - `check_journal_entries` tells you nothing about what failed

3. **Multiple Constraints Per Table**
   - If a table has multiple CHECK constraints, table-based names become ambiguous
   - Descriptive names allow multiple constraints: `check_balanced_entry`, `check_valid_status`, etc.

4. **Self-Documentation**
   - Descriptive names serve as inline documentation
   - Schema readers understand business rules without looking up definitions

5. **Migration & CI/CD**
   - Descriptive names are predictable and consistent across environments
   - Easier to compare schemas and track changes

6. **Infrastructure Already Supports It**
   - We just implemented custom constraint name support
   - Format: `#[check = "name: expression"]`
   - No additional work needed

### Naming Convention Recommendation

For consistency with existing patterns, use:

**Format:** `check_{business_rule_description}`

**Examples:**
- `check_balanced_entry` - Journal entries must balance
- `check_debit_or_credit` - Lines must be debit OR credit
- `check_positive_payment` - Payments must be positive
- `check_positive_quantity` - Quantities must be positive
- `check_positive_unit_price` - Prices must be non-negative

**Guidelines:**
- Use lowercase with underscores
- Describe the business rule, not the table
- Be concise but clear
- Match existing patterns in the codebase

## Implementation

The current implementation supports both approaches:

```rust
// Descriptive name (recommended)
#[check = "balanced_entry: total_debit = total_credit"]

// Table-based (default if no name provided)
#[check = "total_debit = total_credit"]  // Generates: check_journal_entries
```

**Recommendation:** Always specify descriptive names in entity definitions to maintain consistency with the codebase.

## Conclusion

**Use descriptive constraint names** because:
1. They're already the established pattern in the codebase
2. They provide better developer experience (error messages, debugging)
3. They're self-documenting
4. They support multiple constraints per table
5. The infrastructure already supports them with minimal effort

The table-based approach is acceptable for simple cases, but descriptive names provide significantly more value with minimal additional effort.
