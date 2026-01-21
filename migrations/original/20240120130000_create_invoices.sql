-- Migration: Create Invoice Management Tables
-- Version: 20240120130000
-- Description: Creates tables for invoice management (invoices and invoice lines)

-- Invoices: Customer and vendor invoices
CREATE TABLE IF NOT EXISTS invoices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invoice_number VARCHAR(100) NOT NULL UNIQUE,
    invoice_type VARCHAR(20) NOT NULL, -- CUSTOMER, VENDOR
    invoice_date DATE NOT NULL,
    due_date DATE NOT NULL,
    customer_id UUID, -- For customer invoices
    vendor_id UUID, -- For vendor invoices
    status VARCHAR(20) NOT NULL DEFAULT 'DRAFT', -- DRAFT, PENDING_APPROVAL, APPROVED, SENT, PAID, OVERDUE, CANCELLED
    subtotal NUMERIC(19, 4) NOT NULL DEFAULT 0,
    tax_amount NUMERIC(19, 4) NOT NULL DEFAULT 0,
    discount_amount NUMERIC(19, 4) NOT NULL DEFAULT 0,
    total_amount NUMERIC(19, 4) NOT NULL DEFAULT 0,
    currency_code VARCHAR(3) NOT NULL DEFAULT 'USD',
    exchange_rate NUMERIC(19, 6) DEFAULT 1.0,
    base_total_amount NUMERIC(19, 4), -- Base currency amount
    payment_terms VARCHAR(50), -- NET_30, NET_60, DUE_ON_RECEIPT, etc.
    payment_method VARCHAR(50), -- CASH, CHECK, WIRE, CREDIT_CARD, etc.
    reference_number VARCHAR(100), -- PO number, contract number, etc.
    notes TEXT,
    internal_notes TEXT, -- Internal notes not visible to customer/vendor
    approved_at TIMESTAMP,
    approved_by UUID,
    sent_at TIMESTAMP,
    paid_at TIMESTAMP,
    company_id UUID,
    journal_entry_id UUID, -- Link to posted journal entry
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by UUID,
    updated_by UUID
);

CREATE INDEX idx_invoices_invoice_number ON invoices(invoice_number);
CREATE INDEX idx_invoices_invoice_type ON invoices(invoice_type);
CREATE INDEX idx_invoices_invoice_date ON invoices(invoice_date);
CREATE INDEX idx_invoices_due_date ON invoices(due_date);
CREATE INDEX idx_invoices_status ON invoices(status);
CREATE INDEX idx_invoices_customer_id ON invoices(customer_id) WHERE customer_id IS NOT NULL;
CREATE INDEX idx_invoices_vendor_id ON invoices(vendor_id) WHERE vendor_id IS NOT NULL;
CREATE INDEX idx_invoices_company_id ON invoices(company_id);
CREATE INDEX idx_invoices_journal_entry_id ON invoices(journal_entry_id);

-- Invoice Lines: Line items on invoices
CREATE TABLE IF NOT EXISTS invoice_lines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    invoice_id UUID NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    line_number INTEGER NOT NULL,
    item_type VARCHAR(50), -- PRODUCT, SERVICE, EXPENSE, etc.
    item_id UUID, -- Reference to product/service
    description TEXT NOT NULL,
    quantity NUMERIC(19, 4) NOT NULL DEFAULT 1,
    unit_price NUMERIC(19, 4) NOT NULL,
    discount_percent NUMERIC(5, 2) NOT NULL DEFAULT 0,
    discount_amount NUMERIC(19, 4) NOT NULL DEFAULT 0,
    tax_rate NUMERIC(5, 2) NOT NULL DEFAULT 0,
    tax_amount NUMERIC(19, 4) NOT NULL DEFAULT 0,
    line_total NUMERIC(19, 4) NOT NULL, -- quantity * unit_price - discount + tax
    currency_code VARCHAR(3) NOT NULL DEFAULT 'USD',
    account_id UUID REFERENCES accounts(id), -- Revenue/expense account
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT check_positive_quantity CHECK (quantity > 0),
    CONSTRAINT check_positive_unit_price CHECK (unit_price >= 0)
);

CREATE INDEX idx_invoice_lines_invoice_id ON invoice_lines(invoice_id);
CREATE INDEX idx_invoice_lines_line_number ON invoice_lines(invoice_id, line_number);
CREATE INDEX idx_invoice_lines_item_id ON invoice_lines(item_type, item_id) WHERE item_id IS NOT NULL;
CREATE INDEX idx_invoice_lines_account_id ON invoice_lines(account_id);

COMMENT ON TABLE invoices IS 'Customer and vendor invoices';
COMMENT ON TABLE invoice_lines IS 'Line items on invoices';
