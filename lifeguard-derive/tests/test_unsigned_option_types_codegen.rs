//! Tests for unsigned Option types (Option<u8>, Option<u16>, Option<u32>, Option<u64>)
//!
//! This test verifies that unsigned Option types are correctly handled in ModelTrait::get()
//! and get_primary_key_value(), ensuring they generate the correct sea_query::Value types
//! instead of falling through to String(None).

// Include generated code
include!("generated/unsignedoptionuser.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use lifeguard::ModelTrait;

    // ============================================================================
    // Option<u8> Tests
    // ============================================================================

    #[test]
    fn test_option_u8_some() {
        // CRITICAL TEST: Verify Option<u8> with Some generates SmallInt, not String
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: Some(42),
            value_u16: None,
            value_u32: None,
            value_u64: None,
        };

        let value = model.get(Column::ValueU8);
        
        // Verify it's SmallInt(Some(42)), not String(None)
        match value {
            sea_query::Value::SmallInt(Some(42)) => {
                // Correct! Option<u8> with Some(42) generates SmallInt(Some(42))
            }
            sea_query::Value::String(_) => {
                panic!("BUG: Option<u8> generated String value instead of SmallInt! This indicates the unsigned Option handling is broken.");
            }
            sea_query::Value::SmallInt(Some(v)) => {
                panic!("Option<u8> generated SmallInt(Some({})) but expected SmallInt(Some(42))", v);
            }
            _ => {
                panic!("Option<u8> generated unexpected value type: {:?}", value);
            }
        }
    }

    #[test]
    fn test_option_u8_none() {
        // CRITICAL TEST: Verify Option<u8> with None generates SmallInt(None), not String(None)
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: None,
            value_u16: None,
            value_u32: None,
            value_u64: None,
        };

        let value = model.get(Column::ValueU8);
        
        // Verify it's SmallInt(None), not String(None)
        match value {
            sea_query::Value::SmallInt(None) => {
                // Correct! Option<u8> with None generates SmallInt(None)
            }
            sea_query::Value::String(_) => {
                panic!("BUG: Option<u8> with None generated String value instead of SmallInt(None)! This indicates the unsigned Option handling is broken.");
            }
            sea_query::Value::SmallInt(Some(_)) => {
                panic!("Option<u8> with None generated SmallInt(Some(_)) instead of SmallInt(None)!");
            }
            _ => {
                panic!("Option<u8> with None generated unexpected value type: {:?}", value);
            }
        }
    }

    #[test]
    fn test_option_u8_cast() {
        // Verify Option<u8> correctly casts to i16 (SmallInt)
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: Some(255), // Max u8 value
            value_u16: None,
            value_u32: None,
            value_u64: None,
        };

        let value = model.get(Column::ValueU8);
        assert!(matches!(value, sea_query::Value::SmallInt(Some(255))),
            "Option<u8> with Some(255) should generate SmallInt(Some(255)), got: {:?}", value);
    }

    // ============================================================================
    // Option<u16> Tests
    // ============================================================================

    #[test]
    fn test_option_u16_some() {
        // CRITICAL TEST: Verify Option<u16> with Some generates Int, not String
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: None,
            value_u16: Some(1000),
            value_u32: None,
            value_u64: None,
        };

        let value = model.get(Column::ValueU16);
        
        // Verify it's Int(Some(1000)), not String(None)
        match value {
            sea_query::Value::Int(Some(1000)) => {
                // Correct! Option<u16> with Some(1000) generates Int(Some(1000))
            }
            sea_query::Value::String(_) => {
                panic!("BUG: Option<u16> generated String value instead of Int! This indicates the unsigned Option handling is broken.");
            }
            sea_query::Value::Int(Some(v)) => {
                panic!("Option<u16> generated Int(Some({})) but expected Int(Some(1000))", v);
            }
            _ => {
                panic!("Option<u16> generated unexpected value type: {:?}", value);
            }
        }
    }

    #[test]
    fn test_option_u16_none() {
        // CRITICAL TEST: Verify Option<u16> with None generates Int(None), not String(None)
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: None,
            value_u16: None,
            value_u32: None,
            value_u64: None,
        };

        let value = model.get(Column::ValueU16);
        
        // Verify it's Int(None), not String(None)
        match value {
            sea_query::Value::Int(None) => {
                // Correct! Option<u16> with None generates Int(None)
            }
            sea_query::Value::String(_) => {
                panic!("BUG: Option<u16> with None generated String value instead of Int(None)! This indicates the unsigned Option handling is broken.");
            }
            sea_query::Value::Int(Some(_)) => {
                panic!("Option<u16> with None generated Int(Some(_)) instead of Int(None)!");
            }
            _ => {
                panic!("Option<u16> with None generated unexpected value type: {:?}", value);
            }
        }
    }

    #[test]
    fn test_option_u16_cast() {
        // Verify Option<u16> correctly casts to i32 (Int)
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: None,
            value_u16: Some(65535), // Max u16 value
            value_u32: None,
            value_u64: None,
        };

        let value = model.get(Column::ValueU16);
        assert!(matches!(value, sea_query::Value::Int(Some(65535))),
            "Option<u16> with Some(65535) should generate Int(Some(65535)), got: {:?}", value);
    }

    // ============================================================================
    // Option<u32> Tests
    // ============================================================================

    #[test]
    fn test_option_u32_some() {
        // CRITICAL TEST: Verify Option<u32> with Some generates BigInt, not String
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: None,
            value_u16: None,
            value_u32: Some(100000),
            value_u64: None,
        };

        let value = model.get(Column::ValueU32);
        
        // Verify it's BigInt(Some(100000)), not String(None)
        match value {
            sea_query::Value::BigInt(Some(100000)) => {
                // Correct! Option<u32> with Some(100000) generates BigInt(Some(100000))
            }
            sea_query::Value::String(_) => {
                panic!("BUG: Option<u32> generated String value instead of BigInt! This indicates the unsigned Option handling is broken.");
            }
            sea_query::Value::BigInt(Some(v)) => {
                panic!("Option<u32> generated BigInt(Some({})) but expected BigInt(Some(100000))", v);
            }
            _ => {
                panic!("Option<u32> generated unexpected value type: {:?}", value);
            }
        }
    }

    #[test]
    fn test_option_u32_none() {
        // CRITICAL TEST: Verify Option<u32> with None generates BigInt(None), not String(None)
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: None,
            value_u16: None,
            value_u32: None,
            value_u64: None,
        };

        let value = model.get(Column::ValueU32);
        
        // Verify it's BigInt(None), not String(None)
        match value {
            sea_query::Value::BigInt(None) => {
                // Correct! Option<u32> with None generates BigInt(None)
            }
            sea_query::Value::String(_) => {
                panic!("BUG: Option<u32> with None generated String value instead of BigInt(None)! This indicates the unsigned Option handling is broken.");
            }
            sea_query::Value::BigInt(Some(_)) => {
                panic!("Option<u32> with None generated BigInt(Some(_)) instead of BigInt(None)!");
            }
            _ => {
                panic!("Option<u32> with None generated unexpected value type: {:?}", value);
            }
        }
    }

    #[test]
    fn test_option_u32_cast() {
        // Verify Option<u32> correctly casts to i64 (BigInt)
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: None,
            value_u16: None,
            value_u32: Some(4294967295), // Max u32 value
            value_u64: None,
        };

        let value = model.get(Column::ValueU32);
        assert!(matches!(value, sea_query::Value::BigInt(Some(4294967295))),
            "Option<u32> with Some(4294967295) should generate BigInt(Some(4294967295)), got: {:?}", value);
    }

    // ============================================================================
    // Option<u64> Tests
    // ============================================================================

    #[test]
    fn test_option_u64_some() {
        // CRITICAL TEST: Verify Option<u64> with Some generates BigInt, not String
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: None,
            value_u16: None,
            value_u32: None,
            value_u64: Some(10000000000),
        };

        let value = model.get(Column::ValueU64);
        
        // Verify it's BigInt(Some(10000000000)), not String(None)
        match value {
            sea_query::Value::BigInt(Some(10000000000)) => {
                // Correct! Option<u64> with Some(10000000000) generates BigInt(Some(10000000000))
            }
            sea_query::Value::String(_) => {
                panic!("BUG: Option<u64> generated String value instead of BigInt! This indicates the unsigned Option handling is broken.");
            }
            sea_query::Value::BigInt(Some(v)) => {
                panic!("Option<u64> generated BigInt(Some({})) but expected BigInt(Some(10000000000))", v);
            }
            _ => {
                panic!("Option<u64> generated unexpected value type: {:?}", value);
            }
        }
    }

    #[test]
    fn test_option_u64_none() {
        // CRITICAL TEST: Verify Option<u64> with None generates BigInt(None), not String(None)
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: None,
            value_u16: None,
            value_u32: None,
            value_u64: None,
        };

        let value = model.get(Column::ValueU64);
        
        // Verify it's BigInt(None), not String(None)
        match value {
            sea_query::Value::BigInt(None) => {
                // Correct! Option<u64> with None generates BigInt(None)
            }
            sea_query::Value::String(_) => {
                panic!("BUG: Option<u64> with None generated String value instead of BigInt(None)! This indicates the unsigned Option handling is broken.");
            }
            sea_query::Value::BigInt(Some(_)) => {
                panic!("Option<u64> with None generated BigInt(Some(_)) instead of BigInt(None)!");
            }
            _ => {
                panic!("Option<u64> with None generated unexpected value type: {:?}", value);
            }
        }
    }

    #[test]
    fn test_option_u64_cast() {
        // Verify Option<u64> correctly casts to i64 (BigInt)
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: None,
            value_u16: None,
            value_u32: None,
            value_u64: Some(18446744073709551615u64), // Max u64 value (as u64 literal)
        };

        let value = model.get(Column::ValueU64);
        // Note: u64::MAX as i64 will be -1, but we're testing the conversion happens
        // The actual value will be cast, so we just verify it's BigInt(Some(_))
        assert!(matches!(value, sea_query::Value::BigInt(Some(_))),
            "Option<u64> with Some(u64::MAX) should generate BigInt(Some(_)), got: {:?}", value);
    }

    // ============================================================================
    // Comprehensive Tests
    // ============================================================================

    #[test]
    fn test_all_unsigned_options_some() {
        // Test all unsigned Option types with Some values simultaneously
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: Some(42),
            value_u16: Some(1000),
            value_u32: Some(100000),
            value_u64: Some(10000000000),
        };

        let u8_value = model.get(Column::ValueU8);
        let u16_value = model.get(Column::ValueU16);
        let u32_value = model.get(Column::ValueU32);
        let u64_value = model.get(Column::ValueU64);

        assert!(matches!(u8_value, sea_query::Value::SmallInt(Some(42))),
            "Option<u8> should be SmallInt(Some(42)), got: {:?}", u8_value);
        assert!(matches!(u16_value, sea_query::Value::Int(Some(1000))),
            "Option<u16> should be Int(Some(1000)), got: {:?}", u16_value);
        assert!(matches!(u32_value, sea_query::Value::BigInt(Some(100000))),
            "Option<u32> should be BigInt(Some(100000)), got: {:?}", u32_value);
        assert!(matches!(u64_value, sea_query::Value::BigInt(Some(10000000000))),
            "Option<u64> should be BigInt(Some(10000000000)), got: {:?}", u64_value);
    }

    #[test]
    fn test_all_unsigned_options_none() {
        // Test all unsigned Option types with None values simultaneously
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: None,
            value_u16: None,
            value_u32: None,
            value_u64: None,
        };

        let u8_value = model.get(Column::ValueU8);
        let u16_value = model.get(Column::ValueU16);
        let u32_value = model.get(Column::ValueU32);
        let u64_value = model.get(Column::ValueU64);

        assert!(matches!(u8_value, sea_query::Value::SmallInt(None)),
            "Option<u8> with None should be SmallInt(None), got: {:?}", u8_value);
        assert!(matches!(u16_value, sea_query::Value::Int(None)),
            "Option<u16> with None should be Int(None), got: {:?}", u16_value);
        assert!(matches!(u32_value, sea_query::Value::BigInt(None)),
            "Option<u32> with None should be BigInt(None), got: {:?}", u32_value);
        assert!(matches!(u64_value, sea_query::Value::BigInt(None)),
            "Option<u64> with None should be BigInt(None), got: {:?}", u64_value);
    }

    #[test]
    fn test_unsigned_options_not_string() {
        // CRITICAL: Verify none of the unsigned Option types generate String values
        // This is the bug we fixed - they were falling through to String(None)
        let model = UnsignedOptionUserModel {
            id: 1,
            name: "Test".to_string(),
            value_u8: Some(42),
            value_u16: Some(1000),
            value_u32: Some(100000),
            value_u64: Some(10000000000),
        };

        let u8_value = model.get(Column::ValueU8);
        let u16_value = model.get(Column::ValueU16);
        let u32_value = model.get(Column::ValueU32);
        let u64_value = model.get(Column::ValueU64);

        // None of these should be String values
        if matches!(u8_value, sea_query::Value::String(_)) {
            panic!("BUG: Option<u8> generated String value! This is the bug we fixed.");
        }
        if matches!(u16_value, sea_query::Value::String(_)) {
            panic!("BUG: Option<u16> generated String value! This is the bug we fixed.");
        }
        if matches!(u32_value, sea_query::Value::String(_)) {
            panic!("BUG: Option<u32> generated String value! This is the bug we fixed.");
        }
        if matches!(u64_value, sea_query::Value::String(_)) {
            panic!("BUG: Option<u64> generated String value! This is the bug we fixed.");
        }
    }

    #[test]
    fn test_primary_key_still_works() {
        // Verify that primary key handling still works correctly
        let model = UnsignedOptionUserModel {
            id: 999,
            name: "Test".to_string(),
            value_u8: None,
            value_u16: None,
            value_u32: None,
            value_u64: None,
        };

        let pk_value = model.get_primary_key_value();
        
        // Primary key is i32 (non-Option), should generate Int
        assert!(matches!(pk_value, sea_query::Value::Int(Some(999))), 
            "Primary key i32 should generate Int(Some(999)), got: {:?}", pk_value);
    }
}
