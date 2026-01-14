//! Tests for type-safe column operations (Epic 02 Story 05)
//!
//! These tests verify that the Column enum works with ColumnTrait
//! and provides type-safe query building.

use lifeguard_derive::LifeModel;
use lifeguard::{SelectQuery, FromRow};
use sea_query::{Expr, ExprTrait};

#[test]
fn test_column_enum_exists() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    struct User {
        #[primary_key]
        id: i32,
        name: String,
        email: String,
        age: i32,
    }
    
    // Verify Column enum exists and has variants
    let _id_col = Column::Id;
    let _name_col = Column::Name;
    let _email_col = Column::Email;
    let _age_col = Column::Age;
}

#[test]
fn test_column_into_column_ref() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    struct User {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Verify Column implements IntoColumnRef
    use sea_query::IntoColumnRef;
    let col_ref = Column::Id.into_column_ref();
    // Should compile - verifies IntoColumnRef is implemented
    let _ = col_ref;
}

#[test]
fn test_column_trait_methods() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    struct User {
        #[primary_key]
        id: i32,
        name: String,
        age: i32,
    }
    
    // Test that Column enum works with sea_query::Expr methods
    // These should compile, demonstrating type safety
    
    // Equality
    let _filter1 = Expr::col(Column::Id).eq(1);
    let _filter2 = Expr::col(Column::Name).eq("test".to_string());
    let _filter3 = Expr::col(Column::Age).eq(25);
    
    // Comparison
    let _filter4 = Expr::col(Column::Age).gt(18);
    let _filter5 = Expr::col(Column::Age).lte(65);
    
    // Pattern matching
    let _filter6 = Expr::col(Column::Name).like("John%");
    
    // Null checks
    let _filter7 = Expr::col(Column::Name).is_null();
    let _filter8 = Expr::col(Column::Email).is_not_null();
    
    // IN clause
    let _filter9 = Expr::col(Column::Id).is_in(vec![1, 2, 3]);
    
    // BETWEEN
    let _filter10 = Expr::col(Column::Age).between(18, 65);
}

#[test]
fn test_type_safe_query_building() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    struct User {
        #[primary_key]
        id: i32,
        name: String,
        email: String,
    }
    
    // Test that type-safe columns work with query builder
    // This demonstrates the API - actual execution requires an executor
    
    // With Entity pattern, find() is on Entity (generated unit struct), not Model
    let _query = Entity::find()
        .filter(Expr::col(Column::Id).eq(1))
        .filter(Expr::col(Column::Name).like("John%"))
        .filter(Expr::col(Column::Email).eq("test@example.com".to_string()));
    
    // Should compile - verifies type-safe query building works
}

#[test]
fn test_column_with_order_by() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    struct User {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    use sea_query::Order;
    
    // Test that Column enum works with order_by
    // With Entity pattern, find() is on Entity (generated unit struct), not Model
    let _query = Entity::find()
        .filter(Expr::col(Column::Id).gt(0))
        .order_by(Column::Id, Order::Asc)
        .order_by(Column::Name, Order::Desc);
    
    // Should compile - verifies Column works with order_by
}

#[test]
fn test_multiple_column_filters() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    struct User {
        #[primary_key]
        id: i32,
        name: String,
        email: String,
        age: i32,
        active: bool,
    }
    
    // Test chaining multiple type-safe column filters
    // With Entity pattern, find() is on Entity (generated unit struct), not Model
    let _query = Entity::find()
        .filter(Expr::col(Column::Id).gte(1))
        .filter(Expr::col(Column::Age).between(18, 65))
        .filter(Expr::col(Column::Name).like("J%"))
        .filter(Expr::col(Column::Email).is_not_null())
        .filter(Expr::col(Column::Active).eq(true));
    
    // Should compile - demonstrates comprehensive type-safe filtering
}

#[test]
fn test_column_in_clause() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    struct User {
        #[primary_key]
        id: i32,
        status: String,
    }
    
    // Test IN clause with type-safe columns
    // With Entity pattern, find() is on Entity (generated unit struct), not Model
    let _query = Entity::find()
        .filter(Expr::col(Column::Id).is_in(vec![1, 2, 3, 4, 5]))
        .filter(Expr::col(Column::Status).is_in(vec!["active".to_string(), "pending".to_string()]));
    
    // Should compile - verifies IN clause works with type-safe columns
}

#[test]
fn test_column_not_in_clause() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    struct User {
        #[primary_key]
        id: i32,
    }
    
    // Test NOT IN clause with type-safe columns
    // With Entity pattern, find() is on Entity (generated unit struct), not Model
    let _query = Entity::find()
        .filter(Expr::col(Column::Id).is_not_in(vec![999, 1000]));
    
    // Should compile - verifies NOT IN clause works
}

#[test]
fn test_column_between_clause() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    struct User {
        #[primary_key]
        id: i32,
        age: i32,
        score: f64,
    }
    
    // Test BETWEEN clause with different types
    // With Entity pattern, find() is on Entity (generated unit struct), not Model
    let _query = Entity::find()
        .filter(Expr::col(Column::Age).between(18, 65))
        .filter(Expr::col(Column::Score).between(0.0, 100.0));
    
    // Should compile - verifies BETWEEN works with different numeric types
}

#[test]
fn test_column_null_checks() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    struct User {
        #[primary_key]
        id: i32,
        name: String,
        email: String,
        deleted_at: Option<String>,
    }
    
    // Test null checks with type-safe columns
    // With Entity pattern, find() is on Entity (generated unit struct), not Model
    let _query = Entity::find()
        .filter(Expr::col(Column::DeletedAt).is_null())
        .filter(Expr::col(Column::Email).is_not_null());
    
    // Should compile - verifies null checks work
}

#[test]
fn test_column_with_custom_names() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    struct User {
        #[primary_key]
        #[column_name = "user_id"]
        id: i32,
        #[column_name = "full_name"]
        name: String,
    }
    
    // Test that Column enum works even with custom column names
    // With Entity pattern, find() is on Entity (generated unit struct), not Model
    let _query = Entity::find()
        .filter(Expr::col(Column::Id).eq(1))
        .filter(Expr::col(Column::Name).like("John%"));
    
    // Should compile - verifies custom column names work with type-safe columns
}

#[test]
fn test_column_comparison_operators() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    struct User {
        #[primary_key]
        id: i32,
        age: i32,
        score: f64,
    }
    
    // Test all comparison operators with type-safe columns
    // With Entity pattern, find() is on Entity (generated unit struct), not Model
    let _query1 = Entity::find().filter(Expr::col(Column::Age).eq(25));
    let _query2 = Entity::find().filter(Expr::col(Column::Age).ne(25));
    let _query3 = Entity::find().filter(Expr::col(Column::Age).gt(18));
    let _query4 = Entity::find().filter(Expr::col(Column::Age).gte(18));
    let _query5 = Entity::find().filter(Expr::col(Column::Age).lt(65));
    let _query6 = Entity::find().filter(Expr::col(Column::Age).lte(65));
    
    // Should compile - verifies all comparison operators work
}

#[test]
fn test_column_pattern_matching() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    struct User {
        #[primary_key]
        id: i32,
        name: String,
        email: String,
    }
    
    // Test pattern matching with type-safe columns
    // With Entity pattern, find() is on Entity (generated unit struct), not Model
    let _query = Entity::find()
        .filter(Expr::col(Column::Name).like("John%"))
        .filter(Expr::col(Column::Email).like("%@example.com"));
    
    // Should compile - verifies LIKE works with type-safe columns
}
