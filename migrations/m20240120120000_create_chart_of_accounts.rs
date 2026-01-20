//! Migration: Create Chart of Accounts
//! Version: 20240120120000
//! Description: Creates the chart of accounts structure for the accounting system

use lifeguard::migration::{Migration, SchemaManager};
use lifeguard::LifeError;

pub struct CreateChartOfAccounts;

impl Migration for CreateChartOfAccounts {
    fn name(&self) -> &str {
        "create_chart_of_accounts"
    }

    fn version(&self) -> i64 {
        20240120120000
    }

    fn up(&self, manager: &SchemaManager<'_>) -> Result<(), LifeError> {
        // Chart of Accounts: Hierarchical structure for organizing accounts
        manager.execute(
            r#"
            CREATE TABLE IF NOT EXISTS chart_of_accounts (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                code VARCHAR(50) NOT NULL UNIQUE,
                name VARCHAR(255) NOT NULL,
                account_type VARCHAR(50) NOT NULL,
                parent_id UUID REFERENCES chart_of_accounts(id) ON DELETE SET NULL,
                level INTEGER NOT NULL DEFAULT 0,
                is_active BOOLEAN NOT NULL DEFAULT true,
                description TEXT,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
            &[],
        )?;

        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_chart_of_accounts_code ON chart_of_accounts(code)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_chart_of_accounts_parent_id ON chart_of_accounts(parent_id)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_chart_of_accounts_account_type ON chart_of_accounts(account_type)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_chart_of_accounts_is_active ON chart_of_accounts(is_active)",
            &[],
        )?;

        // Accounts: Individual accounts linked to chart of accounts
        manager.execute(
            r#"
            CREATE TABLE IF NOT EXISTS accounts (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                chart_of_account_id UUID NOT NULL REFERENCES chart_of_accounts(id) ON DELETE RESTRICT,
                code VARCHAR(50) NOT NULL UNIQUE,
                name VARCHAR(255) NOT NULL,
                account_type VARCHAR(50) NOT NULL,
                normal_balance VARCHAR(10) NOT NULL,
                currency_code VARCHAR(3) NOT NULL DEFAULT 'USD',
                is_active BOOLEAN NOT NULL DEFAULT true,
                is_system_account BOOLEAN NOT NULL DEFAULT false,
                description TEXT,
                metadata JSONB,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
            &[],
        )?;

        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_accounts_chart_of_account_id ON accounts(chart_of_account_id)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_accounts_code ON accounts(code)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_accounts_account_type ON accounts(account_type)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_accounts_is_active ON accounts(is_active)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_accounts_currency_code ON accounts(currency_code)",
            &[],
        )?;

        // Journal Entries: Double-entry bookkeeping records
        manager.execute(
            r#"
            CREATE TABLE IF NOT EXISTS journal_entries (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                entry_number VARCHAR(50) NOT NULL UNIQUE,
                entry_date DATE NOT NULL,
                description TEXT NOT NULL,
                reference_number VARCHAR(100),
                source_type VARCHAR(50),
                source_id UUID,
                fiscal_period_id UUID,
                status VARCHAR(20) NOT NULL DEFAULT 'DRAFT',
                posted_at TIMESTAMP,
                posted_by UUID,
                total_debit NUMERIC(19, 4) NOT NULL DEFAULT 0,
                total_credit NUMERIC(19, 4) NOT NULL DEFAULT 0,
                currency_code VARCHAR(3) NOT NULL DEFAULT 'USD',
                company_id UUID,
                metadata JSONB,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                created_by UUID,
                updated_by UUID,
                CONSTRAINT check_balanced_entry CHECK (total_debit = total_credit)
            )
            "#,
            &[],
        )?;

        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_journal_entries_entry_number ON journal_entries(entry_number)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_journal_entries_entry_date ON journal_entries(entry_date)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_journal_entries_status ON journal_entries(status)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_journal_entries_source ON journal_entries(source_type, source_id)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_journal_entries_fiscal_period_id ON journal_entries(fiscal_period_id)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_journal_entries_company_id ON journal_entries(company_id)",
            &[],
        )?;

        // Journal Entry Lines: Individual debit/credit lines in a journal entry
        manager.execute(
            r#"
            CREATE TABLE IF NOT EXISTS journal_entry_lines (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                journal_entry_id UUID NOT NULL REFERENCES journal_entries(id) ON DELETE CASCADE,
                account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
                line_number INTEGER NOT NULL,
                description TEXT,
                debit_amount NUMERIC(19, 4) NOT NULL DEFAULT 0,
                credit_amount NUMERIC(19, 4) NOT NULL DEFAULT 0,
                currency_code VARCHAR(3) NOT NULL DEFAULT 'USD',
                exchange_rate NUMERIC(19, 6) DEFAULT 1.0,
                base_debit_amount NUMERIC(19, 4),
                base_credit_amount NUMERIC(19, 4),
                metadata JSONB,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                CONSTRAINT check_debit_or_credit CHECK (
                    (debit_amount > 0 AND credit_amount = 0) OR 
                    (debit_amount = 0 AND credit_amount > 0)
                )
            )
            "#,
            &[],
        )?;

        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_journal_entry_lines_journal_entry_id ON journal_entry_lines(journal_entry_id)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_journal_entry_lines_account_id ON journal_entry_lines(account_id)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_journal_entry_lines_line_number ON journal_entry_lines(journal_entry_id, line_number)",
            &[],
        )?;

        // Account Balances: Current balance for each account (denormalized for performance)
        manager.execute(
            r#"
            CREATE TABLE IF NOT EXISTS account_balances (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                fiscal_period_id UUID NOT NULL,
                balance_date DATE NOT NULL,
                debit_balance NUMERIC(19, 4) NOT NULL DEFAULT 0,
                credit_balance NUMERIC(19, 4) NOT NULL DEFAULT 0,
                net_balance NUMERIC(19, 4) NOT NULL GENERATED ALWAYS AS (debit_balance - credit_balance) STORED,
                currency_code VARCHAR(3) NOT NULL DEFAULT 'USD',
                company_id UUID,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(account_id, fiscal_period_id, balance_date, currency_code, company_id)
            )
            "#,
            &[],
        )?;

        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_account_balances_account_id ON account_balances(account_id)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_account_balances_fiscal_period_id ON account_balances(fiscal_period_id)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_account_balances_balance_date ON account_balances(balance_date)",
            &[],
        )?;
        manager.execute(
            "CREATE INDEX IF NOT EXISTS idx_account_balances_company_id ON account_balances(company_id)",
            &[],
        )?;

        Ok(())
    }

    fn down(&self, manager: &SchemaManager<'_>) -> Result<(), LifeError> {
        // Drop in reverse order of dependencies
        manager.execute("DROP TABLE IF EXISTS account_balances", &[])?;
        manager.execute("DROP TABLE IF EXISTS journal_entry_lines", &[])?;
        manager.execute("DROP TABLE IF EXISTS journal_entries", &[])?;
        manager.execute("DROP TABLE IF EXISTS accounts", &[])?;
        manager.execute("DROP TABLE IF EXISTS chart_of_accounts", &[])?;
        Ok(())
    }
}
