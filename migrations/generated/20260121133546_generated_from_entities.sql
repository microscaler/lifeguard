-- Migration: Generated from Lifeguard entities
-- Version: 20260121133546
-- Generated: 2026-01-21 13:35:46 UTC

-- This migration was automatically generated from entity definitions.
-- DO NOT EDIT MANUALLY - regenerate from entities instead.

-- Table: chart_of_accounts
CREATE TABLE IF NOT EXISTS chart_of_accounts (
    id UUID NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    code VARCHAR(50) NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL,
    account_type VARCHAR(50) NOT NULL,
    parent_id UUID NULL,
    level INTEGER NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT true,
    description TEXT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_chart_of_accounts_code ON chart_of_accounts(code);
CREATE INDEX idx_chart_of_accounts_parent_id ON chart_of_accounts(parent_id);
CREATE INDEX idx_chart_of_accounts_account_type ON chart_of_accounts(account_type);
CREATE INDEX idx_chart_of_accounts_is_active ON chart_of_accounts(is_active);
ALTER TABLE chart_of_accounts ADD CONSTRAINT fk_chart_of_accounts_parent_id FOREIGN KEY (parent_id) REFERENCES chart_of_accounts(id) ON DELETE SET NULL;
COMMENT ON TABLE chart_of_accounts IS 'Hierarchical chart of accounts structure';


