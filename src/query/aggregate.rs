use crate::executor::LifeExecutor;
use crate::query::traits::LifeModelTrait;
use may_postgres::Row;
use sea_query::{PostgresQueryBuilder, SelectStatement};
use std::marker::PhantomData;

/// Trait for unpacking scalar integer/float results from `PostgreSQL` rows
pub trait LifeAggregate: Sized {
    /// Extract the aggregate value from the given database row
    fn from_aggregate_row(row: &Row) -> Result<Self, crate::LifeError>;
}

// Implement for standard primitive aggregate returns
impl LifeAggregate for i64 {
    fn from_aggregate_row(row: &Row) -> Result<Self, crate::LifeError> {
        let val: Option<i64> = row.try_get(0).map_err(crate::LifeError::PostgresError)?;
        Ok(val.unwrap_or(0))
    }
}

impl LifeAggregate for f64 {
    fn from_aggregate_row(row: &Row) -> Result<Self, crate::LifeError> {
        let val: Option<f64> = row.try_get(0).map_err(crate::LifeError::PostgresError)?;
        Ok(val.unwrap_or(0.0))
    }
}

/// Builder for execution of aggregation endpoints bypassing full entity instantiation
pub struct AggregateQuery<E, R>
where
    E: LifeModelTrait,
    R: LifeAggregate,
{
    pub(crate) query: SelectStatement,
    _phantom_model: PhantomData<E>,
    _phantom_return: PhantomData<R>,
}

impl<E, R> AggregateQuery<E, R>
where
    E: LifeModelTrait,
    R: LifeAggregate,
{
    #[must_use] pub fn new(query: SelectStatement) -> Self {
        Self {
            query,
            _phantom_model: PhantomData,
            _phantom_return: PhantomData,
        }
    }

    /// Execute the aggregate query returning a single scalar result
    pub fn one(self, executor: &dyn LifeExecutor) -> Result<R, crate::LifeError> {
        let (sql, values) = self.query.build(PostgresQueryBuilder);
        
        // Execute resolving exactly one row via scalar execution pattern
        crate::query::value_conversion::with_converted_params(&values, |params| {
            match executor.query_one(&sql, params) {
                Ok(row) => R::from_aggregate_row(&row),
                Err(e) => Err(e),
            }
        })
    }
}
