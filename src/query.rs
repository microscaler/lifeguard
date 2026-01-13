//! Query builder for LifeModel - Epic 02 Story 03
//!
//! Provides a query builder that works with SeaQuery to build SQL queries.

use crate::executor::{LifeExecutor, LifeError};
use may_postgres::Row;
use sea_query::{SelectStatement, PostgresQueryBuilder, Iden, Expr};
use std::marker::PhantomData;

/// Query builder for selecting records
///
/// This is returned by `LifeModel::find()` and can be chained with filters.
pub struct SelectQuery<M> {
    query: SelectStatement,
    _phantom: PhantomData<M>,
}

impl<M> SelectQuery<M>
where
    M: FromRow,
{
    /// Create a new select query
    pub fn new(table_name: &'static str) -> Self {
        struct TableName(&'static str);
        impl Iden for TableName {
            fn unquoted(&self) -> &str {
                self.0
            }
        }
        
        let mut query = SelectStatement::default();
        query.column(sea_query::Asterisk).from(TableName(table_name));
        Self {
            query,
            _phantom: PhantomData,
        }
    }
    
    /// Add a filter condition
    pub fn filter(mut self, condition: Expr) -> Self {
        self.query.and_where(condition);
        self
    }
    
    /// Execute the query and return all results
    pub fn all<E: LifeExecutor>(self, executor: &E) -> Result<Vec<M>, LifeError> {
        let (sql, _values) = self.query.build(PostgresQueryBuilder);
        let rows = executor.query_all(&sql, &[])?;
        
        let mut results = Vec::new();
        for row in rows {
            let model = M::from_row(&row)
                .map_err(|e| LifeError::ParseError(format!("Failed to parse row: {}", e)))?;
            results.push(model);
        }
        Ok(results)
    }
    
    /// Execute the query and return a single result
    pub fn one<E: LifeExecutor>(self, executor: &E) -> Result<M, LifeError> {
        let (sql, _values) = self.query.build(PostgresQueryBuilder);
        let row = executor.query_one(&sql, &[])?;
        M::from_row(&row).map_err(|e| LifeError::ParseError(format!("Failed to parse row: {}", e)))
    }
}

/// Trait for types that can be created from a database row
pub trait FromRow: Sized {
    fn from_row(row: &Row) -> Result<Self, may_postgres::Error>;
}
