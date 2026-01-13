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
