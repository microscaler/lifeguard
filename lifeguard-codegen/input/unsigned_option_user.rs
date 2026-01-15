//! UnsignedOptionUser entity for testing Option<u8>, Option<u16>, Option<u32>, Option<u64>

#[table_name = "unsigned_option_users"]
pub struct UnsignedOptionUser {
    #[primary_key]
    pub id: i32,
    
    pub name: String,
    
    // Test fields for unsigned Option types
    pub value_u8: Option<u8>,
    pub value_u16: Option<u16>,
    pub value_u32: Option<u32>,
    pub value_u64: Option<u64>,
}
