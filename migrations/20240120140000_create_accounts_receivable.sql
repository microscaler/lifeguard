-- Migration: Create Accounts Receivable Tables
-- Version: 20240120140000
-- Description: Creates tables for accounts receivable (customer invoices, payments, AR aging)

-- Customer Invoices: Customer-facing invoices (subset of invoices)
CREATE TABLE IF NOT EXISTS customer_invoices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invoice_id UUID NOT NULL UNIQUE REFERENCES invoices(id) ON DELETE CASCADE,
    customer_id UUID NOT NULL,
    invoice_number VARCHAR(100) NOT NULL UNIQUE,
    invoice_date DATE NOT NULL,
    due_date DATE NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'DRAFT',
    subtotal NUMERIC(19, 4) NOT NULL DEFAULT 0,
    tax_amount NUMERIC(19, 4) NOT NULL DEFAULT 0,
    total_amount NUMERIC(19, 4) NOT NULL DEFAULT 0,
    paid_amount NUMERIC(19, 4) NOT NULL DEFAULT 0,
    outstanding_amount NUMERIC(19, 4) NOT NULL GENERATED ALWAYS AS (total_amount - paid_amount) STORED,
    currency_code VARCHAR(3) NOT NULL DEFAULT 'USD',
    payment_terms VARCHAR(50),
    days_overdue INTEGER GENERATED ALWAYS AS (
        CASE 
            WHEN status IN ('SENT', 'OVERDUE') AND due_date < CURRENT_DATE 
            THEN CURRENT_DATE - due_date 
            ELSE 0 
        END
    ) STORED,
    company_id UUID,
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_customer_invoices_invoice_id ON customer_invoices(invoice_id);
CREATE INDEX idx_customer_invoices_customer_id ON customer_invoices(customer_id);
CREATE INDEX idx_customer_invoices_invoice_number ON customer_invoices(invoice_number);
CREATE INDEX idx_customer_invoices_status ON customer_invoices(status);
CREATE INDEX idx_customer_invoices_due_date ON customer_invoices(due_date);
CREATE INDEX idx_customer_invoices_outstanding_amount ON customer_invoices(outstanding_amount) WHERE outstanding_amount > 0;

-- Payments: Customer payments against invoices
CREATE TABLE IF NOT EXISTS ar_payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    payment_number VARCHAR(100) NOT NULL UNIQUE,
    payment_date DATE NOT NULL,
    customer_id UUID NOT NULL,
    payment_method VARCHAR(50) NOT NULL, -- CASH, CHECK, WIRE, CREDIT_CARD, ACH, etc.
    payment_amount NUMERIC(19, 4) NOT NULL,
    currency_code VARCHAR(3) NOT NULL DEFAULT 'USD',
    exchange_rate NUMERIC(19, 6) DEFAULT 1.0,
    base_payment_amount NUMERIC(19, 4),
    reference_number VARCHAR(100), -- Check number, transaction ID, etc.
    bank_account_id UUID, -- Bank account where payment was received
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING', -- PENDING, CLEARED, BOUNCED, REVERSED
    cleared_at TIMESTAMP,
    notes TEXT,
    company_id UUID,
    journal_entry_id UUID, -- Link to posted journal entry
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by UUID,
    CONSTRAINT check_positive_payment CHECK (payment_amount > 0)
);

CREATE INDEX idx_ar_payments_payment_number ON ar_payments(payment_number);
CREATE INDEX idx_ar_payments_payment_date ON ar_payments(payment_date);
CREATE INDEX idx_ar_payments_customer_id ON ar_payments(customer_id);
CREATE INDEX idx_ar_payments_status ON ar_payments(status);
CREATE INDEX idx_ar_payments_company_id ON ar_payments(company_id);
CREATE INDEX idx_ar_payments_journal_entry_id ON ar_payments(journal_entry_id);

-- Payment Applications: Links payments to specific invoices
CREATE TABLE IF NOT EXISTS ar_payment_applications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    payment_id UUID NOT NULL REFERENCES ar_payments(id) ON DELETE CASCADE,
    customer_invoice_id UUID NOT NULL REFERENCES customer_invoices(id) ON DELETE RESTRICT,
    applied_amount NUMERIC(19, 4) NOT NULL,
    discount_taken NUMERIC(19, 4) NOT NULL DEFAULT 0,
    applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    applied_by UUID,
    notes TEXT,
    CONSTRAINT check_positive_applied_amount CHECK (applied_amount > 0)
);

CREATE INDEX idx_ar_payment_applications_payment_id ON ar_payment_applications(payment_id);
CREATE INDEX idx_ar_payment_applications_customer_invoice_id ON ar_payment_applications(customer_invoice_id);

-- AR Aging: Aging analysis for accounts receivable
CREATE TABLE IF NOT EXISTS ar_agings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    customer_id UUID NOT NULL,
    aging_date DATE NOT NULL,
    current_amount NUMERIC(19, 4) NOT NULL DEFAULT 0, -- 0-30 days
    days_31_60 NUMERIC(19, 4) NOT NULL DEFAULT 0,
    days_61_90 NUMERIC(19, 4) NOT NULL DEFAULT 0,
    days_over_90 NUMERIC(19, 4) NOT NULL DEFAULT 0,
    total_outstanding NUMERIC(19, 4) NOT NULL GENERATED ALWAYS AS (
        current_amount + days_31_60 + days_61_90 + days_over_90
    ) STORED,
    currency_code VARCHAR(3) NOT NULL DEFAULT 'USD',
    company_id UUID,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(customer_id, aging_date, currency_code, company_id)
);

CREATE INDEX idx_ar_agings_customer_id ON ar_agings(customer_id);
CREATE INDEX idx_ar_agings_aging_date ON ar_agings(aging_date);
CREATE INDEX idx_ar_agings_company_id ON ar_agings(company_id);

COMMENT ON TABLE customer_invoices IS 'Customer-facing invoices with AR tracking';
COMMENT ON TABLE ar_payments IS 'Customer payments';
COMMENT ON TABLE ar_payment_applications IS 'Links payments to specific invoices';
COMMENT ON TABLE ar_agings IS 'Aging analysis for accounts receivable';
