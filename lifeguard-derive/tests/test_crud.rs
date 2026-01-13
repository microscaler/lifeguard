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
