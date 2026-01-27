//! Test that indexes on parent table columns fail on child entities
//! This is the exact bug pattern: CustomerInvoice/VendorInvoice trying to index
//! columns from the parent 'invoices' table

use lifeguard_derive::LifeModel;

// Simulates the CustomerInvoice entity pattern
#[derive(LifeModel)]
#[table_name = "customer_invoices"]
#[index = "idx_customer_invoices_invoice_number(invoice_number)"]
//~^ ERROR Column 'invoice_number' in index 'idx_customer_invoices_invoice_number' does not exist on this struct
#[index = "idx_customer_invoices_due_date(due_date)"]
//~^ ERROR Column 'due_date' in index 'idx_customer_invoices_due_date' does not exist on this struct
#[index = "idx_customer_invoices_status(status)"]
//~^ ERROR Column 'status' in index 'idx_customer_invoices_status' does not exist on this struct
#[index = "idx_customer_invoices_payment_state(payment_state)"]
//~^ ERROR Column 'payment_state' in index 'idx_customer_invoices_payment_state' does not exist on this struct
pub struct CustomerInvoice {
    #[primary_key]
    pub id: i32,
    
    // Link to base invoice (parent table)
    #[foreign_key = "invoices(id) ON DELETE CASCADE"]
    pub invoice_id: i32,
    
    // Customer reference
    #[foreign_key = "customers(id) ON DELETE RESTRICT"]
    pub customer_id: i32,
    
    // AR-specific fields (only these exist on this table)
    pub outstanding_amount: i64,
    pub days_overdue: i32,
}

fn main() {}
