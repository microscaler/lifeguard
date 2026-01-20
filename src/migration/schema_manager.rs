//! SchemaManager - Provides methods for schema operations in migrations

use crate::{LifeExecutor, LifeError, LifeModelTrait};
use crate::query::column::column_trait::ColumnDefHelper;
use sea_query::{Table, ColumnDef, TableName, SchemaName, IntoIden, TableCreateStatement, TableDropStatement, TableAlterStatement, IndexCreateStatement, IndexDropStatement, Iden};
use std::fmt::Display;

/// SchemaManager provides methods for performing schema operations in migrations
///
/// This struct wraps a `LifeExecutor` and provides convenient methods for
/// common schema operations like creating tables, adding columns, creating indexes, etc.
pub struct SchemaManager {
    executor: Box<dyn LifeExecutor>,
}

impl SchemaManager {
    /// Create a new SchemaManager with the given executor
    pub fn new(executor: Box<dyn LifeExecutor>) -> Self {
        Self { executor }
    }
    
    /// Create a table
    ///
    /// # Example
    /// ```rust,no_run
    /// use sea_query::{Table, ColumnDef};
    /// 
    /// let table = Table::create()
    ///     .table("users")
    ///     .col(ColumnDef::new("id").integer().not_null().auto_increment().primary_key())
    ///     .col(ColumnDef::new("email").string().not_null().unique())
    ///     .to_owned();
    /// 
    /// manager.create_table(table)?;
    /// ```
    pub fn create_table(&self, table: TableCreateStatement) -> Result<(), LifeError> {
        let builder = sea_query::PostgresQueryBuilder;
        let sql = table.build(builder);
        // DDL statements typically don't have parameters
        self.executor.execute(&sql, &[]).map(|_| ())
    }
    
    /// Drop a table
    ///
    /// # Example
    /// ```rust,no_run
    /// use sea_query::Table;
    /// 
    /// let table = Table::drop().table("users").to_owned();
    /// manager.drop_table(table)?;
    /// ```
    pub fn drop_table(&self, table: TableDropStatement) -> Result<(), LifeError> {
        let builder = sea_query::PostgresQueryBuilder;
        let sql = table.build(builder);
        // DDL statements typically don't have parameters
        self.executor.execute(&sql, &[]).map(|_| ())
    }
    
    /// Alter a table
    ///
    /// Uses `Table::alter()` to build ALTER TABLE statements.
    ///
    /// # Example
    /// ```rust,no_run
    /// use sea_query::{Table, ColumnDef};
    /// 
    /// let alter = Table::alter()
    ///     .table("users")
    ///     .add_column(ColumnDef::new("avatar_url").string().null())
    ///     .to_owned();
    /// 
    /// manager.alter_table(alter)?;
    /// ```
    pub fn alter_table(&self, alter: TableAlterStatement) -> Result<(), LifeError> {
        let builder = sea_query::PostgresQueryBuilder;
        let sql = alter.build(builder);
        // DDL statements typically don't have parameters
        self.executor.execute(&sql, &[]).map(|_| ())
    }
    
    /// Create an index
    ///
    /// # Example
    /// ```rust,no_run
    /// use sea_query::{Index, Expr};
    /// 
    /// let index = Index::create()
    ///     .name("idx_users_email")
    ///     .table("users")
    ///     .col(Expr::col("email"))
    ///     .to_owned();
    /// 
    /// manager.create_index(index)?;
    /// ```
    pub fn create_index(&self, index: IndexCreateStatement) -> Result<(), LifeError> {
        let builder = sea_query::PostgresQueryBuilder;
        let sql = index.build(builder);
        // DDL statements typically don't have parameters
        self.executor.execute(&sql, &[]).map(|_| ())
    }
    
    /// Drop an index
    ///
    /// # Example
    /// ```rust,no_run
    /// use sea_query::Index;
    /// 
    /// let index = Index::drop()
    ///     .name("idx_users_email")
    ///     .table("users")
    ///     .to_owned();
    /// 
    /// manager.drop_index(index)?;
    /// ```
    pub fn drop_index(&self, index: IndexDropStatement) -> Result<(), LifeError> {
        let builder = sea_query::PostgresQueryBuilder;
        let sql = index.build(builder);
        // DDL statements typically don't have parameters
        self.executor.execute(&sql, &[]).map(|_| ())
    }
    
    /// Add a column to an existing table
    ///
    /// # Example
    /// ```rust,no_run
    /// use sea_query::ColumnDef;
    /// 
    /// let column = ColumnDef::new("avatar_url").string().null();
    /// manager.add_column("users", column)?;
    /// ```
    pub fn add_column<T: Display>(&self, table: T, column: ColumnDef) -> Result<(), LifeError> {
        let alter = Table::alter()
            .table(table.to_string())
            .add_column(column)
            .to_owned();
        self.alter_table(alter)
    }
    
    /// Drop a column from an existing table
    ///
    /// # Example
    /// ```rust,no_run
    /// manager.drop_column("users", "avatar_url")?;
    /// ```
    pub fn drop_column<T: Display>(&self, table: T, column: &str) -> Result<(), LifeError> {
        let alter = Table::alter()
            .table(table.to_string())
            .drop_column(column.to_string())
            .to_owned();
        self.alter_table(alter)
    }
    
    /// Rename a column in an existing table
    ///
    /// # Example
    /// ```rust,no_run
    /// manager.rename_column("users", "old_name", "new_name")?;
    /// ```
    pub fn rename_column<T: Display>(&self, table: T, old_name: &str, new_name: &str) -> Result<(), LifeError> {
        let alter = Table::alter()
            .table(table.to_string())
            .rename_column(old_name.to_string(), new_name.to_string())
            .to_owned();
        self.alter_table(alter)
    }
    
    /// Execute raw SQL
    ///
    /// # Example
    /// ```rust,no_run
    /// manager.execute("CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\"", &[])?;
    /// ```
    pub fn execute(&self, sql: &str, params: &[&dyn may_postgres::types::ToSql]) -> Result<(), LifeError> {
        self.executor.execute(sql, params).map(|_| ())
    }
    
    /// Get a reference to the underlying executor
    pub fn executor(&self) -> &dyn LifeExecutor {
        self.executor.as_ref()
    }
    
    /// Create a table from a LifeModel entity
    ///
    /// This helper method generates a CREATE TABLE statement from an entity definition,
    /// using all columns, their types, constraints, and default expressions.
    ///
    /// **Note:** Primary key constraints are automatically added if columns have `.primary_key()`
    /// set in their ColumnDef. For composite primary keys, you may need to add the constraint
    /// manually after calling this method.
    ///
    /// # Example
    /// ```rust,no_run
    /// use lifeguard::{LifeModelTrait, migration::SchemaManager};
    ///
    /// # struct User;
    /// # impl lifeguard::LifeModelTrait for User {
    /// #     type Model = ();
    /// #     type Column = ();
    /// # }
    /// # let manager: &SchemaManager = todo!();
    /// manager.create_table_from_entity::<User>()?;
    /// ```
    pub fn create_table_from_entity<E>(&self) -> Result<(), LifeError>
    where
        E: LifeModelTrait,
        E::Column: ColumnDefHelper + Iden,
    {
        let entity = E::default();
        let table_name = entity.table_name();
        let schema_name = entity.schema_name();
        
        // Build table reference with schema if present
        let table_ref = if let Some(schema) = schema_name {
            TableName(Some(SchemaName::from(schema)), table_name.into_iden())
        } else {
            TableName(None, table_name.into_iden())
        };
        
        let mut table = Table::create();
        table.table(table_ref);
        
        // Get all columns from the entity
        let columns = E::all_columns();
        
        // Add each column to the table
        for col in columns {
            let col_def = <E::Column as ColumnDefHelper>::column_def(*col);
            let mut sea_def = col_def.to_column_def(*col);
            
            // Apply default expression if present
            col_def.apply_default_expr(&mut sea_def);
            
            // Add column to table
            // Note: If the column has .primary_key() set in its ColumnDef,
            // SeaQuery will automatically handle the primary key constraint
            table.col(&mut sea_def);
        }
        
        // Note: For composite primary keys, you may need to manually add
        // the primary key constraint after creating the table using:
        // table.primary_key([col1, col2, ...])
        // This can be done by extending the table builder before calling create_table()
        
        // Execute the CREATE TABLE statement
        // table.to_owned() converts to TableCreateStatement which has build()
        let table_stmt = table.to_owned();
        self.create_table(table_stmt)
    }
}
