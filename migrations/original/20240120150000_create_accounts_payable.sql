-- Migration: Create Accounts Payable Tables
-- Version: 20240120150000
-- Description: Creates tables for accounts payable (vendor invoices, payments, AP aging)

-- Vendor Invoices: Vendor invoices (subset of invoices)
CREATE TABLE IF NOT EXISTS vendor_invoices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invoice_id UUID NOT NULL UNIQUE REFERENCES invoices(id) ON DELETE CASCADE,
    vendor_id UUID NOT NULL,
    invoice_number VARCHAR(100) NOT NULL,
    vendor_invoice_number VARCHAR(100), -- Invoice number from vendor
    invoice_date DATE NOT NULL,
    due_date DATE NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'DRAFT', -- DRAFT, PENDING_APPROVAL, APPROVED, PAID, OVERDUE, CANCELLED
    subtotal NUMERIC(19, 4) NOT NULL DEFAULT 0,
    tax_amount NUMERIC(19, 4) NOT NULL DEFAULT 0,
    total_amount NUMERIC(19, 4) NOT NULL DEFAULT 0,
    paid_amount NUMERIC(19, 4) NOT NULL DEFAULT 0,
    outstanding_amount NUMERIC(19, 4) NOT NULL GENERATED ALWAYS AS (total_amount - paid_amount) STORED,
    currency_code VARCHAR(3) NOT NULL DEFAULT 'USD',
    payment_terms VARCHAR(50),
    purchase_order_id UUID, -- Link to purchase order
    receipt_id UUID, -- Link to goods receipt
    days_overdue INTEGER GENERATED ALWAYS AS (
        CASE 
            WHEN status IN ('APPROVED', 'OVERDUE') AND due_date < CURRENT_DATE 
            THEN CURRENT_DATE - due_date 
            ELSE 0 
        END
    ) STORED,
    company_id UUID,
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_vendor_invoices_invoice_id ON vendor_invoices(invoice_id);
CREATE INDEX idx_vendor_invoices_vendor_id ON vendor_invoices(vendor_id);
CREATE INDEX idx_vendor_invoices_invoice_number ON vendor_invoices(invoice_number);
CREATE INDEX idx_vendor_invoices_vendor_invoice_number ON vendor_invoices(vendor_invoice_number);
CREATE INDEX idx_vendor_invoices_status ON vendor_invoices(status);
CREATE INDEX idx_vendor_invoices_due_date ON vendor_invoices(due_date);
CREATE INDEX idx_vendor_invoices_outstanding_amount ON vendor_invoices(outstanding_amount) WHERE outstanding_amount > 0;

-- Payments: Vendor payments
CREATE TABLE IF NOT EXISTS ap_payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    payment_number VARCHAR(100) NOT NULL UNIQUE,
    payment_date DATE NOT NULL,
    vendor_id UUID NOT NULL,
    payment_method VARCHAR(50) NOT NULL, -- CASH, CHECK, WIRE, ACH, etc.
    payment_amount NUMERIC(19, 4) NOT NULL,
    currency_code VARCHAR(3) NOT NULL DEFAULT 'USD',
    exchange_rate NUMERIC(19, 6) DEFAULT 1.0,
    base_payment_amount NUMERIC(19, 4),
    reference_number VARCHAR(100), -- Check number, transaction ID, etc.
    bank_account_id UUID, -- Bank account from which payment was made
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING', -- PENDING, CLEARED, CANCELLED, REVERSED
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

CREATE INDEX idx_ap_payments_payment_number ON ap_payments(payment_number);
CREATE INDEX idx_ap_payments_payment_date ON ap_payments(payment_date);
CREATE INDEX idx_ap_payments_vendor_id ON ap_payments(vendor_id);
CREATE INDEX idx_ap_payments_status ON ap_payments(status);
CREATE INDEX idx_ap_payments_company_id ON ap_payments(company_id);
CREATE INDEX idx_ap_payments_journal_entry_id ON ap_payments(journal_entry_id);

-- Payment Applications: Links payments to specific vendor invoices
CREATE TABLE IF NOT EXISTS ap_payment_applications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    payment_id UUID NOT NULL REFERENCES ap_payments(id) ON DELETE CASCADE,
    vendor_invoice_id UUID NOT NULL REFERENCES vendor_invoices(id) ON DELETE RESTRICT,
    applied_amount NUMERIC(19, 4) NOT NULL,
    discount_taken NUMERIC(19, 4) NOT NULL DEFAULT 0,
    applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    applied_by UUID,
    notes TEXT,
    CONSTRAINT check_positive_applied_amount CHECK (applied_amount > 0)
);

CREATE INDEX idx_ap_payment_applications_payment_id ON ap_payment_applications(payment_id);
CREATE INDEX idx_ap_payment_applications_vendor_invoice_id ON ap_payment_applications(vendor_invoice_id);

-- AP Aging: Aging analysis for accounts payable
CREATE TABLE IF NOT EXISTS ap_agings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    vendor_id UUID NOT NULL,
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
    UNIQUE(vendor_id, aging_date, currency_code, company_id)
);

CREATE INDEX idx_ap_agings_vendor_id ON ap_agings(vendor_id);
CREATE INDEX idx_ap_agings_aging_date ON ap_agings(aging_date);
CREATE INDEX idx_ap_agings_company_id ON ap_agings(company_id);

COMMENT ON TABLE vendor_invoices IS 'Vendor invoices with AP tracking';
COMMENT ON TABLE ap_payments IS 'Vendor payments';
COMMENT ON TABLE ap_payment_applications IS 'Links payments to specific vendor invoices';
COMMENT ON TABLE ap_agings IS 'Aging analysis for accounts payable';
