//! Integration tests for the ValueType system
//!
//! These tests verify that multiple traits work together correctly,
//! simulating real-world usage scenarios.

#[cfg(test)]
mod tests {
    use super::super::*;
    use sea_query::Value;
    
    // Integration: ValueType + TryGetable
    
    #[test]
    fn test_value_type_to_try_getable_roundtrip() {
        // Convert using ValueType, then extract using TryGetable
        let original = 42i32;
        let value = original.into_value();
        let extracted: Result<i32, _> = TryGetable::try_get(value);
        assert_eq!(extracted, Ok(original));
    }
    
    #[test]
    fn test_value_type_to_try_getable_string() {
        let original = "hello".to_string();
        let value = original.clone().into_value();
        let extracted: Result<String, _> = TryGetable::try_get(value);
        assert_eq!(extracted, Ok(original));
    }
    
    // Integration: ValueType + TryGetable + Option
    
    #[test]
    fn test_option_value_type_to_try_getable() {
        let original = Some(42i32);
        let value = original.into_value();
        let extracted: Result<Option<i32>, _> = TryGetable::try_get(value);
        assert_eq!(extracted, Ok(Some(42)));
    }
    
    #[test]
    fn test_option_none_value_type_to_try_getable() {
        let original: Option<i32> = None;
        let value = original.into_value();
        let extracted: Result<Option<i32>, _> = TryGetable::try_get(value);
        assert_eq!(extracted, Ok(None));
    }
    
    // Integration: IntoValueTuple + FromValueTuple
    
    #[test]
    fn test_tuple_roundtrip_2() {
        let original = (42i32, "hello".to_string());
        let value_tuple = original.clone().into_value_tuple();
        let extracted: Result<(i32, String), _> = FromValueTuple::from_value_tuple(value_tuple);
        assert_eq!(extracted, Ok(original));
    }
    
    #[test]
    fn test_tuple_roundtrip_3() {
        let original = (1i32, 2i32, 3i32);
        let value_tuple = original.clone().into_value_tuple();
        let extracted: Result<(i32, i32, i32), _> = FromValueTuple::from_value_tuple(value_tuple);
        assert_eq!(extracted, Ok(original));
    }
    
    #[test]
    fn test_tuple_roundtrip_mixed_types() {
        let original = (42i32, "hello".to_string(), true, 3.14f64);
        let value_tuple = original.clone().into_value_tuple();
        let extracted: Result<(i32, String, bool, f64), _> = FromValueTuple::from_value_tuple(value_tuple);
        let (i, s, b, f) = extracted.unwrap();
        assert_eq!(i, 42);
        assert_eq!(s, "hello");
        assert_eq!(b, true);
        assert!((f - 3.14).abs() < f64::EPSILON);
    }
    
    // Integration: TryFromU64 + ValueType
    
    #[test]
    fn test_try_from_u64_to_value_type() {
        let u64_value: u64 = 42;
        let i32_value: i32 = TryFromU64::try_from_u64(u64_value).unwrap();
        let value = i32_value.into_value();
        assert!(matches!(value, Value::Int(Some(42))));
    }
    
    #[test]
    fn test_try_from_u64_to_value_type_overflow() {
        let u64_value: u64 = i32::MAX as u64 + 1;
        let result: Result<i32, _> = TryFromU64::try_from_u64(u64_value);
        assert!(result.is_err());
        // Should not panic when trying to convert to Value
    }
    
    // Integration: TryGetableMany + ValueType
    
    #[test]
    fn test_try_get_many_from_value_type() {
        let values = vec![1i32, 2i32, 3i32];
        let value_vec: Vec<Value> = values.iter().map(|v| v.clone().into_value()).collect();
        let extracted: Result<Vec<i32>, _> = TryGetableMany::try_get_many(value_vec);
        assert_eq!(extracted, Ok(values));
    }
    
    #[test]
    fn test_try_get_many_opt_from_value_type() {
        let values = vec![Some(1i32), None, Some(3i32)];
        let value_vec: Vec<Value> = values.iter().map(|v| v.into_value()).collect();
        let extracted: Result<Vec<Option<i32>>, _> = TryGetableMany::try_get_many_opt(value_vec);
        assert_eq!(extracted, Ok(values));
    }
    
    // Integration: Composite key scenario
    
    #[test]
    fn test_composite_key_full_workflow() {
        // Simulate a composite primary key workflow
        let composite_key = (42i32, "tenant_1".to_string());
        
        // Convert to Value tuple
        let value_tuple = composite_key.clone().into_value_tuple();
        
        // Extract back using TryGetable
        let id: Result<i32, _> = TryGetable::try_get(value_tuple.0.clone());
        let tenant: Result<String, _> = TryGetable::try_get(value_tuple.1.clone());
        
        assert_eq!(id, Ok(42));
        assert_eq!(tenant, Ok("tenant_1".to_string()));
        
        // Extract as tuple
        let extracted: Result<(i32, String), _> = FromValueTuple::from_value_tuple(value_tuple);
        assert_eq!(extracted, Ok(composite_key));
    }
    
    // Integration: Error propagation
    
    #[test]
    fn test_error_propagation_through_traits() {
        // Start with wrong type
        let wrong_value = Value::String(Some("hello".to_string()));
        
        // TryGetable should fail
        let result: Result<i32, _> = TryGetable::try_get(wrong_value);
        assert!(matches!(result, Err(ValueExtractionError::TypeMismatch { .. })));
        
        // ValueType::from_value should also fail
        let result = <i32 as ValueType>::from_value(Value::String(Some("hello".to_string())));
        assert_eq!(result, None);
    }
    
    // Integration: Real-world primary key scenario
    
    #[test]
    fn test_primary_key_u64_to_i32_workflow() {
        // Database returns u64, but model uses i32
        let db_id: u64 = 42;
        
        // Convert safely
        let model_id: i32 = TryFromU64::try_from_u64(db_id).unwrap();
        
        // Convert to Value
        let value = model_id.into_value();
        
        // Extract back
        let extracted: Result<i32, _> = TryGetable::try_get(value);
        assert_eq!(extracted, Ok(42));
    }
    
    #[test]
    fn test_primary_key_u64_overflow_handling() {
        // Database returns u64 that's too large for i32
        let db_id: u64 = i32::MAX as u64 + 1;
        
        // Conversion should fail gracefully
        let result: Result<i32, _> = TryFromU64::try_from_u64(db_id);
        assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
    }
    
    // Integration: Batch operations
    
    #[test]
    fn test_batch_value_conversion() {
        let ids = vec![1i32, 2i32, 3i32, 4i32, 5i32];
        
        // Convert all to Values
        let values: Vec<Value> = ids.iter().map(|id| id.clone().into_value()).collect();
        
        // Extract all back
        let extracted: Result<Vec<i32>, _> = TryGetableMany::try_get_many(values);
        assert_eq!(extracted, Ok(ids));
    }
    
    #[test]
    fn test_batch_with_nulls() {
        let ids = vec![Some(1i32), None, Some(3i32)];
        
        // Convert all to Values
        let values: Vec<Value> = ids.iter().map(|id| id.into_value()).collect();
        
        // Extract all back (allowing nulls)
        let extracted: Result<Vec<Option<i32>>, _> = TryGetableMany::try_get_many_opt(values);
        assert_eq!(extracted, Ok(ids));
    }
    
    // Integration: Mixed type batch
    
    #[test]
    fn test_mixed_type_batch() {
        let strings = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let values: Vec<Value> = strings.iter().map(|s| s.clone().into_value()).collect();
        let extracted: Result<Vec<String>, _> = TryGetableMany::try_get_many(values);
        assert_eq!(extracted, Ok(strings));
    }
}
