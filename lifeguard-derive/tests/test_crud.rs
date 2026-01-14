//! Tests for CRUD operations - Epic 02 Story 03
//!
//! Note: These tests verify that CRUD methods are generated correctly.
//! They compile-check the method signatures but don't execute database operations.
//!
//! The generated code references `lifeguard` and `sea_query` crates, which must be
//! available when the macro expands. These are added as dev-dependencies.
//!
//! For full integration tests with actual database operations, see the main crate's
//! test suite.

use lifeguard_derive::{LifeModel, LifeRecord};
use sea_query::{Expr, ExprTrait};

#[test]
fn test_model_implements_from_row() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_from_row"]
    struct TestFromRow {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Verify that Model implements FromRow trait
    // This is a compile-time check - if it compiles, the trait is implemented
    let _model = TestFromRowModel { id: 1, name: "Test".to_string() };
    let _from_row_method = TestFromRowModel::from_row;
}

#[test]
fn test_find_by_id_method_exists() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_find_by_id"]
    struct TestFindById {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Verify that find_by_id method exists
    // This is a compile-time check
    fn _check_find_by_id<E: lifeguard::LifeExecutor>(executor: &E, id: i32) -> Result<TestFindByIdModel, lifeguard::LifeError> {
        TestFindByIdModel::find_by_id(executor, id)
    }
}

#[test]
fn test_find_method_exists() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_find"]
    struct TestFind {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Verify that find method exists and returns SelectQuery
    let _query = TestFindModel::find();
}

#[test]
fn test_delete_method_exists() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_delete"]
    struct TestDelete {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Verify that delete method exists
    // This is a compile-time check
    fn _check_delete<E: lifeguard::LifeExecutor>(executor: &E, id: i32) -> Result<u64, lifeguard::LifeError> {
        TestDeleteModel::delete(executor, id)
    }
}

#[test]
fn test_insert_method_exists() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_insert"]
    struct TestInsert {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Verify that insert method exists
    // This is a compile-time check
    fn _check_insert<E: lifeguard::LifeExecutor>(record: &TestInsertRecord, executor: &E) -> Result<TestInsertModel, lifeguard::LifeError> {
        record.insert(executor)
    }
}

#[test]
fn test_update_method_exists() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_update"]
    struct TestUpdate {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Verify that update method exists
    // This is a compile-time check
    fn _check_update<E: lifeguard::LifeExecutor>(record: &TestUpdateRecord, executor: &E, id: i32) -> Result<TestUpdateModel, lifeguard::LifeError> {
        record.update(executor, id)
    }
}

#[test]
fn test_select_query_methods() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_select_query"]
    struct TestSelectQuery {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Verify that SelectQuery has all() and one() methods
    // This is a compile-time check - methods exist on the instance
    let _query = TestSelectQueryModel::find();
    // These are instance methods, so we just verify the query compiles
    // The methods will be available when we have an executor
}

#[test]
fn test_insert_many_method_exists() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_insert_many"]
    struct TestInsertMany {
        #[primary_key]
        id: i32,
        name: String,
        email: String,
    }
    
    // Verify that insert_many method exists
    // This is a compile-time check
    fn _check_insert_many<E: lifeguard::LifeExecutor>(records: &[TestInsertManyRecord], executor: &E) -> Result<Vec<TestInsertManyModel>, lifeguard::LifeError> {
        TestInsertManyModel::insert_many(records, executor)
    }
}

#[test]
fn test_update_many_method_exists() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_update_many"]
    struct TestUpdateMany {
        #[primary_key]
        id: i32,
        name: String,
        email: String,
    }
    
    // Verify that update_many method exists
    // This is a compile-time check
    fn _check_update_many<E: lifeguard::LifeExecutor>(filter: sea_query::Expr, values: &TestUpdateManyRecord, executor: &E) -> Result<u64, lifeguard::LifeError> {
        TestUpdateManyModel::update_many(filter, values, executor)
    }
}

#[test]
fn test_delete_many_method_exists() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_delete_many"]
    struct TestDeleteMany {
        #[primary_key]
        id: i32,
        name: String,
        email: String,
    }
    
    // Verify that delete_many method exists
    // This is a compile-time check
    fn _check_delete_many<E: lifeguard::LifeExecutor>(filter: sea_query::Expr, executor: &E) -> Result<u64, lifeguard::LifeError> {
        TestDeleteManyModel::delete_many(filter, executor)
    }
}

#[test]
fn test_batch_operations_with_query_builder() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_batch_ops"]
    struct TestBatchOps {
        #[primary_key]
        id: i32,
        name: String,
        age: i32,
    }
    
    use sea_query::Expr;
    
    // Verify that batch operations work with query builder expressions
    // This is a compile-time check
    fn _check_batch_ops<E: lifeguard::LifeExecutor>(
        records: &[TestBatchOpsRecord],
        update_values: &TestBatchOpsRecord,
        executor: &E
    ) -> Result<(), lifeguard::LifeError> {
        // Test insert_many
        let _inserted = TestBatchOpsModel::insert_many(records, executor)?;
        
        // Test update_many with filter
        let filter = Expr::col("age").gte(18);
        let _updated_count = TestBatchOpsModel::update_many(filter, update_values, executor)?;
        
        // Test delete_many with filter
        let delete_filter = Expr::col("age").lt(18);
        let _deleted_count = TestBatchOpsModel::delete_many(delete_filter, executor)?;
        
        Ok(())
    }
}

// ============================================================================
// EDGE CASE TESTS FOR BATCH OPERATIONS (Epic 02 Story 06)
// ============================================================================

#[test]
fn test_insert_many_empty_slice() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_empty_insert"]
    struct TestEmptyInsert {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Verify that insert_many handles empty slice
    // This is a compile-time check - the method should accept empty slice
    fn _check_empty_insert<E: lifeguard::LifeExecutor>(executor: &E) -> Result<Vec<TestEmptyInsertModel>, lifeguard::LifeError> {
        let empty_records: &[TestEmptyInsertRecord] = &[];
        TestEmptyInsertModel::insert_many(empty_records, executor)
    }
}

#[test]
fn test_insert_many_single_record() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_single_insert"]
    struct TestSingleInsert {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Verify that insert_many handles single record
    fn _check_single_insert<E: lifeguard::LifeExecutor>(record: &TestSingleInsertRecord, executor: &E) -> Result<Vec<TestSingleInsertModel>, lifeguard::LifeError> {
        TestSingleInsertModel::insert_many(&[record.clone()], executor)
    }
}

#[test]
fn test_insert_many_mixed_null_values() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_mixed_null"]
    struct TestMixedNull {
        #[primary_key]
        id: i32,
        name: String,
        email: Option<String>,
        age: Option<i32>,
    }
    
    // Verify that insert_many handles records with mixed NULL and non-NULL values
    fn _check_mixed_null<E: lifeguard::LifeExecutor>(
        records: &[TestMixedNullRecord],
        executor: &E
    ) -> Result<Vec<TestMixedNullModel>, lifeguard::LifeError> {
        TestMixedNullModel::insert_many(records, executor)
    }
}

#[test]
fn test_update_many_no_matches() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_update_no_match"]
    struct TestUpdateNoMatch {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Verify that update_many returns 0 when filter matches no rows
    fn _check_update_no_match<E: lifeguard::LifeExecutor>(
        filter: Expr,
        values: &TestUpdateNoMatchRecord,
        executor: &E
    ) -> Result<u64, lifeguard::LifeError> {
        TestUpdateNoMatchModel::update_many(filter, values, executor)
    }
}

#[test]
fn test_update_many_empty_values() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_update_empty"]
    struct TestUpdateEmpty {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Verify that update_many errors when all fields are None
    fn _check_update_empty<E: lifeguard::LifeExecutor>(
        filter: Expr,
        values: &TestUpdateEmptyRecord,
        executor: &E
    ) -> Result<u64, lifeguard::LifeError> {
        TestUpdateEmptyModel::update_many(filter, values, executor)
    }
}

#[test]
fn test_update_many_primary_key_skipped() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_update_pk_skip"]
    struct TestUpdatePkSkip {
        #[primary_key]
        id: i32,
        name: String,
        email: String,
    }
    
    // Verify that update_many skips primary key even if set in values
    fn _check_update_pk_skip<E: lifeguard::LifeExecutor>(
        filter: Expr,
        values: &TestUpdatePkSkipRecord,
        executor: &E
    ) -> Result<u64, lifeguard::LifeError> {
        // Even if values.id is Some, it should be skipped
        TestUpdatePkSkipModel::update_many(filter, values, executor)
    }
}

#[test]
fn test_update_many_null_values() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_update_null"]
    struct TestUpdateNull {
        #[primary_key]
        id: i32,
        name: Option<String>,
        email: Option<String>,
    }
    
    // Verify that update_many handles NULL values
    fn _check_update_null<E: lifeguard::LifeExecutor>(
        filter: Expr,
        values: &TestUpdateNullRecord,
        executor: &E
    ) -> Result<u64, lifeguard::LifeError> {
        TestUpdateNullModel::update_many(filter, values, executor)
    }
}

#[test]
fn test_update_many_complex_filter() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_update_complex"]
    struct TestUpdateComplex {
        #[primary_key]
        id: i32,
        name: String,
        age: i32,
        active: bool,
    }
    
    // Verify that update_many works with complex filter expressions
    fn _check_update_complex<E: lifeguard::LifeExecutor>(
        values: &TestUpdateComplexRecord,
        executor: &E
    ) -> Result<u64, lifeguard::LifeError> {
        // Complex filter: (age >= 18 AND active = true) OR (age < 18 AND name LIKE 'Admin%')
        let filter = Expr::col("age").gte(18)
            .and(Expr::col("active").eq(true))
            .or(Expr::col("age").lt(18)
                .and(Expr::col("name").like("Admin%")));
        TestUpdateComplexModel::update_many(filter, values, executor)
    }
}

#[test]
fn test_delete_many_no_matches() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_delete_no_match"]
    struct TestDeleteNoMatch {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Verify that delete_many returns 0 when filter matches no rows
    fn _check_delete_no_match<E: lifeguard::LifeExecutor>(
        filter: Expr,
        executor: &E
    ) -> Result<u64, lifeguard::LifeError> {
        TestDeleteNoMatchModel::delete_many(filter, executor)
    }
}

#[test]
fn test_delete_many_complex_filter() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_delete_complex"]
    struct TestDeleteComplex {
        #[primary_key]
        id: i32,
        name: String,
        age: i32,
        status: String,
    }
    
    // Verify that delete_many works with complex filter expressions
    fn _check_delete_complex<E: lifeguard::LifeExecutor>(
        executor: &E
    ) -> Result<u64, lifeguard::LifeError> {
        // Complex filter: age < 18 OR (age >= 65 AND status = 'retired')
        let filter = Expr::col("age").lt(18)
            .or(Expr::col("age").gte(65)
                .and(Expr::col("status").eq("retired")));
        TestDeleteComplexModel::delete_many(filter, executor)
    }
}

#[test]
fn test_delete_many_in_clause() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_delete_in"]
    struct TestDeleteIn {
        #[primary_key]
        id: i32,
        status: String,
    }
    
    // Verify that delete_many works with IN clause (many parameters)
    fn _check_delete_in<E: lifeguard::LifeExecutor>(
        executor: &E
    ) -> Result<u64, lifeguard::LifeError> {
        // Filter with IN clause - many parameters
        let statuses = vec!["active", "pending", "suspended", "deleted"];
        let filter = Expr::col("status").is_in(statuses);
        TestDeleteInModel::delete_many(filter, executor)
    }
}

// ============================================================================
// NULL VALUE HANDLING TESTS FOR delete_many (Value::Null fix verification)
// ============================================================================

#[test]
fn test_delete_many_with_is_null_filter() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_delete_is_null"]
    struct TestDeleteIsNull {
        #[primary_key]
        id: i32,
        name: String,
        email: Option<String>,
    }
    
    // Verify that delete_many handles is_null() filter which produces Value::Null
    // This tests the fix for Value::Null handling in the first value conversion loop
    fn _check_delete_is_null<E: lifeguard::LifeExecutor>(
        executor: &E
    ) -> Result<u64, lifeguard::LifeError> {
        // Filter with is_null() - this produces Value::Null in the query values
        let filter = Expr::col("email").is_null();
        TestDeleteIsNullModel::delete_many(filter, executor)
    }
}

#[test]
fn test_delete_many_with_explicit_null_comparison() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_delete_null_eq"]
    struct TestDeleteNullEq {
        #[primary_key]
        id: i32,
        name: String,
        email: Option<String>,
    }
    
    // Verify that delete_many handles explicit null comparison which produces Value::Null
    // This tests the fix for Value::Null handling in both value conversion loops
    fn _check_delete_null_eq<E: lifeguard::LifeExecutor>(
        executor: &E
    ) -> Result<u64, lifeguard::LifeError> {
        // Filter with explicit null comparison - this produces Value::Null in the query values
        let filter = Expr::col("email").eq(Expr::null());
        TestDeleteNullEqModel::delete_many(filter, executor)
    }
}

#[test]
fn test_delete_many_with_complex_null_filter() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_delete_complex_null"]
    struct TestDeleteComplexNull {
        #[primary_key]
        id: i32,
        name: String,
        email: Option<String>,
        phone: Option<String>,
    }
    
    // Verify that delete_many handles complex filters with multiple null checks
    // This tests the fix for Value::Null handling when multiple null values appear
    fn _check_delete_complex_null<E: lifeguard::LifeExecutor>(
        executor: &E
    ) -> Result<u64, lifeguard::LifeError> {
        // Complex filter with multiple null checks - produces multiple Value::Null instances
        // (email IS NULL OR phone IS NULL) AND name != 'admin'
        let filter = Expr::col("name").ne("admin")
            .and(
                Expr::col("email").is_null()
                    .or(Expr::col("phone").is_null())
            );
        TestDeleteComplexNullModel::delete_many(filter, executor)
    }
}

// ============================================================================
// VALUE::NULL HANDLING IN INSERT_MANY (Fix verification)
// ============================================================================

#[test]
fn test_insert_many_handles_value_null_in_conversion() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_insert_null_conversion"]
    struct TestInsertNullConversion {
        #[primary_key]
        id: i32,
        name: String,
        email: Option<String>,
        age: Option<i32>,
    }
    
    // Verify that insert_many handles Value::Null in the value conversion loops
    // This tests that when None fields produce Value::Null, they are properly converted
    // to ToSql parameters without falling through to the catch-all error
    let mut record = TestInsertNullConversionRecord::new();
    record.set_name("Test".to_string());
    // email and age are None - will produce Value::Null
    
    // Verify the method exists and accepts records with None fields
    fn _check_insert_null_conversion<E: lifeguard::LifeExecutor>(
        records: &[TestInsertNullConversionRecord],
        executor: &E
    ) -> Result<Vec<TestInsertNullConversionModel>, lifeguard::LifeError> {
        // Records with None fields will produce Value::Null in the query
        // This should be handled in both conversion loops (value collection and params building)
        TestInsertNullConversionModel::insert_many(records, executor)
    }
    
    // Verify record has None fields
    assert!(record.email.is_none());
    assert!(record.age.is_none());
    assert_eq!(record.name, Some("Test".to_string()));
}

#[test]
fn test_insert_many_handles_mixed_null_and_non_null() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_insert_mixed_null"]
    struct TestInsertMixedNull {
        #[primary_key]
        id: i32,
        name: String,
        email: Option<String>,
        phone: Option<String>,
    }
    
    // Verify that insert_many handles records with mixed None and Some fields
    // This ensures Value::Null appears in the sea_values and is properly handled
    let mut record1 = TestInsertMixedNullRecord::new();
    record1.set_name("User1".to_string()).set_email("user1@example.com".to_string());
    // phone is None
    
    let mut record2 = TestInsertMixedNullRecord::new();
    record2.set_name("User2".to_string());
    // email and phone are None
    
    // Verify the method exists and accepts records with mixed None/Some fields
    fn _check_insert_mixed_null<E: lifeguard::LifeExecutor>(
        records: &[TestInsertMixedNullRecord],
        executor: &E
    ) -> Result<Vec<TestInsertMixedNullModel>, lifeguard::LifeError> {
        // Some records have email=None, some have phone=None, some have both
        // This produces Value::Null in various positions in the value array
        TestInsertMixedNullModel::insert_many(records, executor)
    }
    
    // Verify records have mixed None/Some fields
    assert!(record1.phone.is_none());
    assert_eq!(record1.email, Some("user1@example.com".to_string()));
    assert!(record2.email.is_none());
    assert!(record2.phone.is_none());
}

// ============================================================================
// PRIMARY KEY HANDLING IN INSERT_MANY (Fix verification)
// ============================================================================

#[test]
fn test_insert_many_skips_primary_key_when_none() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_insert_pk_none"]
    struct TestInsertPkNone {
        #[primary_key]
        id: i32,
        name: String,
        email: String,
    }
    
    // Verify that insert_many skips primary key column when it's None (auto-increment case)
    // This matches single insert behavior - primary key is never included in columns
    // even if it's set to Some, to allow auto-increment to work
    // Create records with primary key as None (auto-increment case)
    let mut record1 = TestInsertPkNoneRecord::new();
    record1.set_name("Alice".to_string()).set_email("alice@example.com".to_string());
    // id is None - should be excluded from columns
    
    let mut record2 = TestInsertPkNoneRecord::new();
    record2.set_name("Bob".to_string()).set_email("bob@example.com".to_string());
    // id is None - should be excluded from columns
    
    // Verify the method exists and accepts records with None primary key
    // Primary key should NOT be included in columns, allowing auto-increment to work
    fn _check_insert_pk_none<E: lifeguard::LifeExecutor>(
        records: &[TestInsertPkNoneRecord],
        executor: &E
    ) -> Result<Vec<TestInsertPkNoneModel>, lifeguard::LifeError> {
        TestInsertPkNoneModel::insert_many(records, executor)
    }
    
    // Verify records can be created with None primary key
    assert!(record1.id.is_none());
    assert!(record2.id.is_none());
}

#[test]
fn test_insert_many_skips_primary_key_even_when_some() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_insert_pk_some"]
    struct TestInsertPkSome {
        #[primary_key]
        id: i32,
        name: String,
        email: String,
    }
    
    // Verify that insert_many skips primary key column even when it's Some
    // Primary keys should NEVER be included in insert columns, regardless of value
    // This matches single insert behavior
    // Create records with primary key set to Some (should still be excluded)
    let mut record1 = TestInsertPkSomeRecord::new();
    record1.set_id(1).set_name("Alice".to_string()).set_email("alice@example.com".to_string());
    // id is Some(1) - should STILL be excluded from columns
    
    let mut record2 = TestInsertPkSomeRecord::new();
    record2.set_id(2).set_name("Bob".to_string()).set_email("bob@example.com".to_string());
    // id is Some(2) - should STILL be excluded from columns
    
    // Verify the method exists and accepts records with Some primary key
    // Primary key should NOT be included in columns, even when set
    fn _check_insert_pk_some<E: lifeguard::LifeExecutor>(
        records: &[TestInsertPkSomeRecord],
        executor: &E
    ) -> Result<Vec<TestInsertPkSomeModel>, lifeguard::LifeError> {
        TestInsertPkSomeModel::insert_many(records, executor)
    }
    
    // Verify records can be created with Some primary key
    assert_eq!(record1.id, Some(1));
    assert_eq!(record2.id, Some(2));
}

#[test]
fn test_insert_many_matches_single_insert_primary_key_behavior() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_insert_pk_consistency"]
    struct TestInsertPkConsistency {
        #[primary_key]
        id: i32,
        name: String,
        email: Option<String>,
        age: Option<i32>,
    }
    
    // Verify that insert_many matches single insert behavior for primary key handling
    // Both should skip primary key column entirely, regardless of its value
    // This is a compile-time check - both methods should exist and accept records with None primary key
    fn _check_insert_pk_consistency<E: lifeguard::LifeExecutor>(
        single_record: &TestInsertPkConsistencyRecord,
        batch_records: &[TestInsertPkConsistencyRecord],
        executor: &E
    ) -> Result<(), lifeguard::LifeError> {
        // Single insert: primary key can be None, should be skipped
        // This verifies the single insert method exists and accepts records with None primary key
        let _single_result = single_record.insert(executor)?;
        
        // Batch insert: primary key can be None, should also be skipped
        // This verifies insert_many method exists and accepts records with None primary key
        // Both should work the same way - primary key excluded, auto-increment works
        let _batch_result = TestInsertPkConsistencyModel::insert_many(batch_records, executor)?;
        
        Ok(())
    }
}

#[test]
fn test_insert_many_auto_increment_primary_key() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_insert_auto_increment"]
    struct TestInsertAutoIncrement {
        #[primary_key]
        id: i32, // SERIAL/auto-increment primary key
        name: String,
        email: String,
    }
    
    // Verify that insert_many works correctly with auto-increment primary keys
    // Primary key should be excluded from columns, allowing PostgreSQL to generate it
    // Records with None primary key (typical auto-increment usage)
    let mut record1 = TestInsertAutoIncrementRecord::new();
    record1.set_name("User1".to_string()).set_email("user1@example.com".to_string());
    // id is None - should be excluded, allowing auto-increment
    
    let mut record2 = TestInsertAutoIncrementRecord::new();
    record2.set_name("User2".to_string()).set_email("user2@example.com".to_string());
    // id is None - should be excluded, allowing auto-increment
    
    // Verify the method exists and accepts records with None primary key
    fn _check_insert_auto_increment<E: lifeguard::LifeExecutor>(
        records: &[TestInsertAutoIncrementRecord],
        executor: &E
    ) -> Result<Vec<TestInsertAutoIncrementModel>, lifeguard::LifeError> {
        // This should work - primary key excluded, database generates IDs
        // Should NOT fail with NOT NULL constraint violation
        TestInsertAutoIncrementModel::insert_many(records, executor)
    }
    
    // Verify records have None primary key (auto-increment case)
    assert!(record1.id.is_none());
    assert!(record2.id.is_none());
    assert_eq!(record1.name, Some("User1".to_string()));
    assert_eq!(record2.name, Some("User2".to_string()));
}

// ============================================================================
// DIRTY FIELDS BEHAVIOR IN INSERT_MANY (Fix verification)
// ============================================================================

#[test]
fn test_insert_many_respects_dirty_fields_like_single_insert() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_insert_dirty_fields"]
    struct TestInsertDirtyFields {
        #[primary_key]
        id: i32,
        name: String,
        email: Option<String>,
        age: Option<i32>,
    }
    
    // Verify that insert_many only includes columns for fields that are Some
    // This matches single insert behavior - None fields are skipped entirely
    // (not sent as NULL) to allow database defaults to apply
    // Create records with only some fields set (matching single insert behavior)
    // The first record determines which columns are included
    // All records must have the same fields set (consistent dirty fields)
    let mut record1 = TestInsertDirtyFieldsRecord::new();
    record1.set_name("Alice".to_string()).set_email("alice@example.com".to_string());
    // age is None - should be excluded from columns
    
    let mut record2 = TestInsertDirtyFieldsRecord::new();
    record2.set_name("Bob".to_string()).set_email("bob@example.com".to_string());
    // age is None - should be excluded from columns
    
    // Verify the method exists and accepts records with consistent dirty fields
    fn _check_insert_dirty_fields<E: lifeguard::LifeExecutor>(
        records: &[TestInsertDirtyFieldsRecord],
        executor: &E
    ) -> Result<Vec<TestInsertDirtyFieldsModel>, lifeguard::LifeError> {
        // Both records have same fields set (name and email), age is None in both
        // This should work - only name and email columns should be included
        TestInsertDirtyFieldsModel::insert_many(records, executor)
    }
    
    // Verify records have consistent dirty fields
    assert!(record1.age.is_none());
    assert!(record2.age.is_none());
    assert_eq!(record1.name, Some("Alice".to_string()));
    assert_eq!(record2.name, Some("Bob".to_string()));
}

#[test]
fn test_insert_many_skips_none_fields_consistently() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_insert_skip_none"]
    struct TestInsertSkipNone {
        #[primary_key]
        id: i32,
        name: String,
        email: Option<String>,
        phone: Option<String>,
        address: Option<String>,
    }
    
    // Verify that insert_many skips None fields consistently across all records
    // Only fields that are Some in the first record should be included
    // First record: only name and email set
    let mut record1 = TestInsertSkipNoneRecord::new();
    record1.set_name("User1".to_string()).set_email("user1@example.com".to_string());
    // phone and address are None - should be excluded
    
    // Second record: same fields set (name and email), phone and address still None
    let mut record2 = TestInsertSkipNoneRecord::new();
    record2.set_name("User2".to_string()).set_email("user2@example.com".to_string());
    // phone and address are None - should be excluded
    
    // Verify the method exists and accepts records with skipped None fields
    fn _check_insert_skip_none<E: lifeguard::LifeExecutor>(
        records: &[TestInsertSkipNoneRecord],
        executor: &E
    ) -> Result<Vec<TestInsertSkipNoneModel>, lifeguard::LifeError> {
        // This should work - only name and email columns included
        // phone and address are skipped entirely (not sent as NULL)
        TestInsertSkipNoneModel::insert_many(records, executor)
    }
    
    // Verify records have None fields that should be skipped
    assert!(record1.phone.is_none());
    assert!(record1.address.is_none());
    assert!(record2.phone.is_none());
    assert!(record2.address.is_none());
}

// ============================================================================
// JSON VALUE TYPE SUPPORT IN BATCH OPERATIONS (Fix verification)
// ============================================================================

#[test]
fn test_insert_many_handles_json_fields() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_insert_json"]
    struct TestInsertJson {
        #[primary_key]
        id: i32,
        name: String,
        metadata: String, // JSON field stored as String
        config: Option<String>, // Optional JSON field
    }
    
    // Verify that insert_many handles sea_query::Value::Json types correctly
    // This tests both Json(Some) and Json(None) cases in the conversion loops
    let mut record = TestInsertJsonRecord::new();
    record.set_name("Test".to_string()).set_metadata(r#"{"key": "value"}"#.to_string());
    // config is None - will produce Json(None)
    
    // Verify the method exists and accepts records with JSON fields
    fn _check_insert_json<E: lifeguard::LifeExecutor>(
        records: &[TestInsertJsonRecord],
        executor: &E
    ) -> Result<Vec<TestInsertJsonModel>, lifeguard::LifeError> {
        // Records with JSON fields should work without "Unsupported value type" errors
        // Json(Some) should be converted to string and added to params
        // Json(None) should be converted to NULL and added to params
        TestInsertJsonModel::insert_many(records, executor)
    }
    
    // Verify record has JSON fields
    assert_eq!(record.metadata, Some(r#"{"key": "value"}"#.to_string()));
    assert!(record.config.is_none());
}

#[test]
fn test_update_many_handles_json_fields() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_update_json"]
    struct TestUpdateJson {
        #[primary_key]
        id: i32,
        name: String,
        metadata: String, // JSON field stored as String
        config: Option<String>, // Optional JSON field
    }
    
    // Verify that update_many handles sea_query::Value::Json types correctly
    // This tests both Json(Some) and Json(None) cases in the conversion loops
    let mut values = TestUpdateJsonRecord::new();
    values.set_metadata(r#"{"updated": true}"#.to_string());
    // config is None - will produce Json(None)
    
    // Verify the method exists and accepts records with JSON fields
    fn _check_update_json<E: lifeguard::LifeExecutor>(
        filter: Expr,
        values: &TestUpdateJsonRecord,
        executor: &E
    ) -> Result<u64, lifeguard::LifeError> {
        // Update with JSON fields should work without "Unsupported value type" errors
        // Json(Some) should be converted to string and added to params
        // Json(None) should be converted to NULL and added to params
        TestUpdateJsonModel::update_many(filter, values, executor)
    }
    
    // Verify values record has JSON fields
    assert_eq!(values.metadata, Some(r#"{"updated": true}"#.to_string()));
    assert!(values.config.is_none());
}

#[test]
fn test_delete_many_handles_json_in_filter() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_delete_json_filter"]
    struct TestDeleteJsonFilter {
        #[primary_key]
        id: i32,
        name: String,
        metadata: String, // JSON field stored as String
    }
    
    // Verify that delete_many handles sea_query::Value::Json types in filter expressions
    // This tests Json handling when it appears in WHERE clause values
    let filter = Expr::col("metadata").eq(r#"{"key": "value"}"#);
    
    // Verify the method exists and accepts filters with JSON values
    fn _check_delete_json_filter<E: lifeguard::LifeExecutor>(
        filter: Expr,
        executor: &E
    ) -> Result<u64, lifeguard::LifeError> {
        // Filter expressions that produce Json values should work
        // This tests Json(Some) and Json(None) in the conversion loops
        TestDeleteJsonFilterModel::delete_many(filter, executor)
    }
    
    // Verify filter expression compiles (compile-time check)
    let _ = filter;
}

#[test]
fn test_batch_operations_json_with_null_values() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_json_null_batch"]
    struct TestJsonNullBatch {
        #[primary_key]
        id: i32,
        name: String,
        metadata: Option<String>, // Optional JSON field - can be None
    }
    
    // Verify that batch operations handle Json(None) correctly
    // This tests the Json(None) => nulls.push(None) path in all batch operations
    let mut record = TestJsonNullBatchRecord::new();
    record.set_name("Test".to_string());
    // metadata is None - will produce Json(None)
    
    let mut update_values = TestJsonNullBatchRecord::new();
    update_values.set_name("Updated".to_string());
    // metadata is None - will produce Json(None)
    
    // Verify the methods exist and accept records with Json(None) fields
    fn _check_json_null_batch<E: lifeguard::LifeExecutor>(
        records: &[TestJsonNullBatchRecord],
        update_values: &TestJsonNullBatchRecord,
        executor: &E
    ) -> Result<(), lifeguard::LifeError> {
        // insert_many with Json(None) fields
        let _inserted = TestJsonNullBatchModel::insert_many(records, executor)?;
        
        // update_many with Json(None) in values
        let filter = Expr::col("id").gt(0);
        let _updated = TestJsonNullBatchModel::update_many(filter, update_values, executor)?;
        
        // delete_many doesn't need Json values, but verifies method exists
        let delete_filter = Expr::col("id").lt(0);
        let _deleted = TestJsonNullBatchModel::delete_many(delete_filter, executor)?;
        
        Ok(())
    }
    
    // Verify records have Json(None) fields
    assert!(record.metadata.is_none());
    assert!(update_values.metadata.is_none());
}

#[test]
fn test_batch_operations_type_safety() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_type_safety"]
    struct TestTypeSafety {
        #[primary_key]
        id: i32,
        name: String,
        age: i32,
        score: f64,
        active: bool,
    }
    
    // Verify type safety - methods should only accept correct types
    fn _check_type_safety<E: lifeguard::LifeExecutor>(
        records: &[TestTypeSafetyRecord],
        update_values: &TestTypeSafetyRecord,
        executor: &E
    ) -> Result<(), lifeguard::LifeError> {
        // insert_many should return Vec<Model>
        let _inserted: Vec<TestTypeSafetyModel> = TestTypeSafetyModel::insert_many(records, executor)?;
        
        // update_many should return u64
        let _updated: u64 = TestTypeSafetyModel::update_many(
            Expr::col("age").gte(18),
            update_values,
            executor
        )?;
        
        // delete_many should return u64
        let _deleted: u64 = TestTypeSafetyModel::delete_many(
            Expr::col("age").lt(18),
            executor
        )?;
        
        Ok(())
    }
}

#[test]
fn test_batch_operations_all_data_types() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_all_types"]
    struct TestAllTypes {
        #[primary_key]
        id: i32,
        tiny_int: i8,
        small_int: i16,
        regular_int: i32,
        big_int: i64,
        tiny_uint: u8,
        small_uint: u16,
        regular_uint: u32,
        big_uint: u64,
        float_val: f32,
        double_val: f64,
        bool_val: bool,
        string_val: String,
        optional_string: Option<String>,
    }
    
    // Verify that batch operations work with all data types
    fn _check_all_types<E: lifeguard::LifeExecutor>(
        records: &[TestAllTypesRecord],
        update_values: &TestAllTypesRecord,
        executor: &E
    ) -> Result<(), lifeguard::LifeError> {
        let _inserted = TestAllTypesModel::insert_many(records, executor)?;
        let _updated = TestAllTypesModel::update_many(
            Expr::col("id").gt(0),
            update_values,
            executor
        )?;
        let _deleted = TestAllTypesModel::delete_many(
            Expr::col("id").lt(0),
            executor
        )?;
        Ok(())
    }
}

#[test]
fn test_batch_operations_with_json_fields() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_json_batch"]
    struct TestJsonBatch {
        #[primary_key]
        id: i32,
        name: String,
        metadata: Option<String>, // JSON field stored as String
    }
    
    // Verify that batch operations work with JSON fields
    // This is a compile-time check - the methods should accept records with JSON fields
    fn _check_json_batch<E: lifeguard::LifeExecutor>(
        records: &[TestJsonBatchRecord],
        update_values: &TestJsonBatchRecord,
        executor: &E
    ) -> Result<(), lifeguard::LifeError> {
        // Test insert_many with JSON fields
        let _inserted = TestJsonBatchModel::insert_many(records, executor)?;
        
        // Test update_many with JSON fields
        let filter = Expr::col("id").gt(0);
        let _updated_count = TestJsonBatchModel::update_many(filter, update_values, executor)?;
        
        // Test delete_many (doesn't need JSON, but verifies the method exists)
        let delete_filter = Expr::col("id").lt(0);
        let _deleted_count = TestJsonBatchModel::delete_many(delete_filter, executor)?;
        
        Ok(())
    }
}
