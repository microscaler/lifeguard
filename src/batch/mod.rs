//! Batch operations: multi-row writes, batched reads by key, and a buffering writer.
//!
//! Single-row ORM calls cost one statement (one round trip) per row. These APIs write or read
//! many rows per statement while keeping the ORM's semantics: `before_insert` / `before_update`
//! hooks and validators run per record, `save_as` expressions apply, RLS context and replica
//! routing come from the executor, and results map through the entity's `FromRow`.
//!
//! | API | Statement |
//! |---|---|
//! | [`insert_many`] | `INSERT INTO t (..) VALUES (..), (..) [RETURNING *]`, or `SELECT * FROM unnest($1::int8[], ..)` for large batches |
//! | [`upsert_many`] | the same with `ON CONFLICT (..) DO UPDATE / DO NOTHING` |
//! | [`update_many`] | `UPDATE t SET c = v.c .. FROM unnest(..) AS v(..) WHERE t.pk = v.pk` |
//! | [`delete_many`] | `DELETE FROM t WHERE pk = ANY($1)` |
//! | [`find_by_ids`] | `SELECT .. FROM t WHERE pk = ANY($1)` |
//! | [`BatchWriter`] | a buffer that coroutines push rows into; a flusher writes them with [`insert_many`] / [`upsert_many`] |
//!
//! ## Chunking and statement reuse
//!
//! PostgreSQL accepts at most 65,535 parameters per statement. A `VALUES` batch uses one per
//! value, so rows are split into chunks of at most [`BatchOptions::max_rows`] rows and
//! [`BatchOptions::max_params`] parameters. Full chunks have identical SQL, so each chunk size is
//! prepared once per connection and then served from the statement cache. The `unnest` form uses
//! one array parameter per column whatever the row count, so its SQL is constant per column set:
//! it is chosen when a chunk has at least [`BatchOptions::unnest_min_rows`] rows and every value
//! is a plain value (no `save_as` expression).
//!
//! ## Errors
//!
//! Chunks run in order, each as its own statement. A failure returns [`BatchError`] naming the
//! chunk and its row range, with the rows written by earlier chunks. Run the call inside
//! [`crate::LifeguardPool::with_session_transaction`] (or a `Transaction`) to make the whole batch
//! atomic.

pub mod writer;

pub use writer::{BatchWriter, BatchWriterConfig, BatchWriterStats, WriteMode};

use crate::active_model::{ActiveModelBehavior, ActiveModelError, ActiveModelTrait};
use crate::executor::{LifeError, LifeExecutor};
use crate::pool::{OwnedArray, OwnedParam};
use crate::query::column::column_trait::ColumnDefHelper;
use crate::query::traits::{FromRow, LifeEntityName, LifeModelTrait};
use sea_query::{Expr, Iden, OnConflict, PostgresQueryBuilder, Query, Value};
use std::fmt;

/// PostgreSQL's limit on bind parameters in one statement.
pub const POSTGRES_MAX_PARAMS: usize = 65_535;

/// Tuning for the batch APIs.
#[derive(Debug, Clone)]
pub struct BatchOptions {
    /// Rows per statement (chunk), at most.
    pub max_rows: usize,
    /// Bind parameters per statement, at most (capped at [`POSTGRES_MAX_PARAMS`]).
    pub max_params: usize,
    /// Use `unnest(..)` arrays instead of `VALUES` when a chunk has at least this many rows
    /// (`usize::MAX` disables `unnest`).
    pub unnest_min_rows: usize,
    /// Add `RETURNING *` and map the written rows back to models.
    pub returning: bool,
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self {
            max_rows: 1000,
            max_params: 32_000,
            unnest_min_rows: 64,
            returning: false,
        }
    }
}

impl BatchOptions {
    /// Default options with `RETURNING *`.
    #[must_use]
    pub fn returning() -> Self {
        Self {
            returning: true,
            ..Self::default()
        }
    }
}

/// What a batch call wrote or read.
#[derive(Debug)]
pub struct BatchOutcome<M> {
    /// Rows the statements reported as affected (inserted, updated or deleted).
    pub rows_affected: u64,
    /// Rows returned (`RETURNING *`, or the rows found by [`find_by_ids`]).
    pub models: Vec<M>,
    /// Statements run.
    pub statements: usize,
}

impl<M> Default for BatchOutcome<M> {
    fn default() -> Self {
        Self {
            rows_affected: 0,
            models: Vec::new(),
            statements: 0,
        }
    }
}

/// A batch call failed in one chunk; earlier chunks were written.
#[derive(Debug)]
pub struct BatchError {
    /// Zero-based index of the failing chunk.
    pub chunk: usize,
    /// Input rows `rows.start..rows.end` made up the failing chunk.
    pub rows: std::ops::Range<usize>,
    /// Rows affected by the chunks before it.
    pub rows_written_before: u64,
    /// The underlying error.
    pub source: ActiveModelError,
}

/// For use inside closures that return [`LifeError`], e.g.
/// [`crate::LifeguardPool::with_session_transaction`]. Chunk details move into the message.
impl From<BatchError> for LifeError {
    fn from(e: BatchError) -> Self {
        Self::QueryError(e.to_string())
    }
}

impl fmt::Display for BatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "batch chunk {} (rows {}..{}) failed after {} rows were written: {}",
            self.chunk, self.rows.start, self.rows.end, self.rows_written_before, self.source
        )
    }
}

impl std::error::Error for BatchError {}

impl BatchError {
    fn before_any(source: ActiveModelError) -> Self {
        Self {
            chunk: 0,
            rows: 0..0,
            rows_written_before: 0,
            source,
        }
    }
}

fn db(e: &LifeError) -> ActiveModelError {
    ActiveModelError::DatabaseError(e.to_string())
}

// ---------------------------------------------------------------------------------------------
// Helpers

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn table_sql<E: LifeEntityName>(entity: &E) -> String {
    match entity.schema_name() {
        Some(schema) => format!(
            "{}.{}",
            quote_ident(schema),
            quote_ident(entity.table_name())
        ),
        None => quote_ident(entity.table_name()),
    }
}

fn column_name<C: Iden>(c: &C) -> String {
    c.to_string()
}

/// The entity's single primary-key column.
fn primary_key_column<E: LifeModelTrait>() -> Result<E::Column, ActiveModelError>
where
    E::Column: Copy,
{
    let pks: Vec<E::Column> = E::all_columns()
        .iter()
        .copied()
        .filter(|c| <E::Column as ColumnDefHelper>::column_def(*c).primary_key)
        .collect();
    match pks.as_slice() {
        [one] => Ok(*one),
        [] => Err(ActiveModelError::Other(
            "batch by key needs an entity with a #[primary_key] column".to_string(),
        )),
        _ => Err(ActiveModelError::Other(
            "batch by key supports single-column primary keys only".to_string(),
        )),
    }
}

fn is_json_column<E: LifeModelTrait>(c: E::Column) -> bool {
    <E::Column as ColumnDefHelper>::column_def(c)
        .column_type
        .is_some_and(|t| t.to_ascii_uppercase().contains("JSON"))
}

/// Typed arrays for `unnest`, one per column, or `None` when a column cannot be expressed as an
/// array (expression values, mixed or unsupported types, all-NULL columns).
fn columns_as_arrays(rows: &[Vec<Expr>], ncols: usize) -> Option<Vec<(OwnedArray, &'static str)>> {
    let mut out = Vec::with_capacity(ncols);
    for col in 0..ncols {
        let first = rows.iter().find_map(|r| match &r[col] {
            Expr::Value(v) if !value_is_null(v) => Some(v),
            _ => None,
        })?;
        let (mut arr, ty) = OwnedArray::for_value(first)?;
        for r in rows {
            match &r[col] {
                Expr::Value(v) => {
                    if !arr.push(v) {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        out.push((arr, ty));
    }
    Some(out)
}

fn value_is_null(v: &Value) -> bool {
    matches!(
        v,
        Value::Bool(None)
            | Value::TinyInt(None)
            | Value::SmallInt(None)
            | Value::Int(None)
            | Value::BigInt(None)
            | Value::TinyUnsigned(None)
            | Value::SmallUnsigned(None)
            | Value::Unsigned(None)
            | Value::BigUnsigned(None)
            | Value::Float(None)
            | Value::Double(None)
            | Value::String(None)
            | Value::Char(None)
            | Value::Bytes(None)
            | Value::Json(None)
            | Value::ChronoDate(None)
            | Value::ChronoTime(None)
            | Value::ChronoDateTime(None)
            | Value::ChronoDateTimeUtc(None)
            | Value::ChronoDateTimeLocal(None)
            | Value::Uuid(None)
            | Value::Decimal(None)
    )
}

fn values_to_owned(values: &sea_query::Values) -> Result<Vec<OwnedParam>, ActiveModelError> {
    values
        .0
        .iter()
        .map(OwnedParam::try_from)
        .collect::<Result<_, _>>()
        .map_err(|e| db(&e))
}

/// Split `n` rows of `ncols` values into chunk ranges under the row and parameter caps.
fn chunk_ranges(n: usize, ncols: usize, opts: &BatchOptions) -> Vec<std::ops::Range<usize>> {
    let max_params = opts.max_params.clamp(1, POSTGRES_MAX_PARAMS);
    let by_params = (max_params / ncols.max(1)).max(1);
    let size = opts.max_rows.max(1).min(by_params);
    (0..n).step_by(size).map(|s| s..(s + size).min(n)).collect()
}

// ---------------------------------------------------------------------------------------------
// insert_many / upsert_many

/// Insert many records, chunked, optionally returning the written rows.
///
/// Every record runs its `before_insert` hook and validators first, exactly as
/// [`ActiveModelTrait::insert`] does. Records in one call must set the same columns (the batch
/// statement has one column list); mixing column sets is an error.
///
/// # Errors
///
/// [`BatchError`] for the first failing chunk (or chunk 0 when a record fails its hooks).
pub fn insert_many<R>(
    executor: &dyn LifeExecutor,
    records: &[R],
    opts: &BatchOptions,
) -> Result<BatchOutcome<<R::Entity as LifeModelTrait>::Model>, BatchError>
where
    R: ActiveModelTrait + ActiveModelBehavior,
    R::Entity: LifeEntityName + Default + Iden + Clone + 'static,
    <R::Entity as LifeModelTrait>::Column: Copy + Iden + 'static,
    <R::Entity as LifeModelTrait>::Model: FromRow,
{
    write_many(executor, records, None, opts)
}

/// Insert many records with an `ON CONFLICT` clause (idempotent ingest).
///
/// Use `OnConflict::columns([..]).update_columns([..])` for last-write-wins upserts, or
/// `.do_nothing()` to skip existing keys. PostgreSQL rejects a statement that would update the
/// same row twice ("ON CONFLICT DO UPDATE command cannot affect row a second time"), so callers
/// that may repeat a key within one call should de-duplicate first ([`BatchWriter`] does).
///
/// # Errors
///
/// [`BatchError`] for the first failing chunk.
pub fn upsert_many<R>(
    executor: &dyn LifeExecutor,
    records: &[R],
    on_conflict: OnConflict,
    opts: &BatchOptions,
) -> Result<BatchOutcome<<R::Entity as LifeModelTrait>::Model>, BatchError>
where
    R: ActiveModelTrait + ActiveModelBehavior,
    R::Entity: LifeEntityName + Default + Iden + Clone + 'static,
    <R::Entity as LifeModelTrait>::Column: Copy + Iden + 'static,
    <R::Entity as LifeModelTrait>::Model: FromRow,
{
    write_many(executor, records, Some(on_conflict), opts)
}

#[allow(clippy::too_many_lines)]
fn write_many<R>(
    executor: &dyn LifeExecutor,
    records: &[R],
    on_conflict: Option<OnConflict>,
    opts: &BatchOptions,
) -> Result<BatchOutcome<<R::Entity as LifeModelTrait>::Model>, BatchError>
where
    R: ActiveModelTrait + ActiveModelBehavior,
    R::Entity: LifeEntityName + Default + Iden + Clone + 'static,
    <R::Entity as LifeModelTrait>::Column: Copy + Iden + 'static,
    <R::Entity as LifeModelTrait>::Model: FromRow,
{
    let mut out = BatchOutcome::default();
    if records.is_empty() {
        return Ok(out);
    }
    // Hooks, validators and column extraction for every record up front: a bad record fails the
    // call before anything is written.
    let mut columns: Option<Vec<<R::Entity as LifeModelTrait>::Column>> = None;
    let mut rows: Vec<Vec<Expr>> = Vec::with_capacity(records.len());
    for (i, r) in records.iter().enumerate() {
        let (cols, exprs) = r.__lg_insert_row().map_err(|source| BatchError {
            chunk: 0,
            rows: i..i + 1,
            rows_written_before: 0,
            source,
        })?;
        match &columns {
            None => columns = Some(cols),
            Some(first) => {
                let same = first.len() == cols.len()
                    && first
                        .iter()
                        .zip(&cols)
                        .all(|(a, b)| column_name(a) == column_name(b));
                if !same {
                    return Err(BatchError {
                        chunk: 0,
                        rows: i..i + 1,
                        rows_written_before: 0,
                        source: ActiveModelError::Other(format!(
                            "batch insert: record {i} sets different columns than record 0; one call writes one column set"
                        )),
                    });
                }
            }
        }
        rows.push(exprs);
    }
    let columns = columns.unwrap_or_default();
    let ncols = columns.len();
    let entity = R::Entity::default();
    let metrics = <R::Entity as LifeModelTrait>::__lg_entity_metrics();
    let op = if on_conflict.is_some() {
        crate::metrics::entity::Op::Upsert
    } else {
        crate::metrics::entity::Op::Insert
    };

    for (chunk_index, range) in chunk_ranges(rows.len(), ncols, opts)
        .into_iter()
        .enumerate()
    {
        let chunk = &rows[range.clone()];
        let fail = |source: ActiveModelError, written: u64| BatchError {
            chunk: chunk_index,
            rows: range.clone(),
            rows_written_before: written,
            source,
        };

        // Single-row-shaped statement built by SeaQuery: gives the quoted prefix, and the
        // ON CONFLICT / RETURNING suffix for the unnest form.
        let mut q = Query::insert();
        if let Some(schema) = entity.schema_name() {
            q.into_table((sea_query::Alias::new(schema), entity.clone()));
        } else {
            q.into_table(entity.clone());
        }
        q.columns(columns.iter().copied());

        let use_unnest = chunk.len() >= opts.unnest_min_rows;
        let arrays = if use_unnest {
            columns_as_arrays(chunk, ncols)
        } else {
            None
        };

        let (sql, params) = if let Some(arrays) = arrays {
            // Render the clauses around a one-row VALUES, then swap VALUES for unnest.
            q.values_panic(chunk[0].iter().cloned());
            if let Some(oc) = on_conflict.clone() {
                q.on_conflict(oc);
            }
            if opts.returning {
                q.returning_all();
            }
            let (one_row_sql, one_row_values) = q.build(PostgresQueryBuilder);
            match unnest_sql::<R::Entity>(
                &one_row_sql,
                one_row_values.0.len(),
                ncols,
                &arrays,
                &columns,
            ) {
                Some(sql) => (
                    sql,
                    arrays
                        .into_iter()
                        .map(|(a, _)| OwnedParam::Array(a))
                        .collect(),
                ),
                None => {
                    let (sql, values) = values_statement(
                        &entity,
                        &columns,
                        chunk,
                        on_conflict.clone(),
                        opts.returning,
                    );
                    (
                        sql,
                        values_to_owned(&values).map_err(|e| fail(e, out.rows_affected))?,
                    )
                }
            }
        } else {
            let (sql, values) = values_statement(
                &entity,
                &columns,
                chunk,
                on_conflict.clone(),
                opts.returning,
            );
            (
                sql,
                values_to_owned(&values).map_err(|e| fail(e, out.rows_affected))?,
            )
        };

        let timer = crate::metrics::entity::OpTimer::start(metrics, op);
        if opts.returning {
            let res = executor.__lg_query_all_owned(&sql, params);
            timer.finish(res.as_ref().map_or(0, |r| r.len() as u64), res.is_ok());
            let rows_back = res.map_err(|e| fail(db(&e), out.rows_affected))?;
            out.rows_affected += rows_back.len() as u64;
            for row in &rows_back {
                let m = <<R::Entity as LifeModelTrait>::Model as FromRow>::from_row(row).map_err(
                    |e| {
                        fail(
                            ActiveModelError::DatabaseError(e.to_string()),
                            out.rows_affected,
                        )
                    },
                )?;
                out.models.push(m);
            }
        } else {
            let res = executor.__lg_execute_owned(&sql, params);
            timer.finish(*res.as_ref().unwrap_or(&0), res.is_ok());
            out.rows_affected += res.map_err(|e| fail(db(&e), out.rows_affected))?;
        }
        out.statements += 1;
    }
    Ok(out)
}

fn values_statement<E, C>(
    entity: &E,
    columns: &[C],
    chunk: &[Vec<Expr>],
    on_conflict: Option<OnConflict>,
    returning: bool,
) -> (String, sea_query::Values)
where
    E: LifeEntityName + Iden + Clone + 'static,
    C: Iden + Copy + 'static,
{
    let mut q = Query::insert();
    if let Some(schema) = entity.schema_name() {
        q.into_table((sea_query::Alias::new(schema), entity.clone()));
    } else {
        q.into_table(entity.clone());
    }
    q.columns(columns.iter().copied());
    for row in chunk {
        q.values_panic(row.iter().cloned());
    }
    if let Some(oc) = on_conflict {
        q.on_conflict(oc);
    }
    if returning {
        q.returning_all();
    }
    q.build(PostgresQueryBuilder)
}

/// Turn a rendered one-row `INSERT .. VALUES ($1, .., $n) <suffix>` into
/// `INSERT .. SELECT .. FROM unnest($1::t[], ..) AS lg_u(..) <suffix>`. `None` when the suffix
/// carries parameters of its own (they would need renumbering) or the shape is unexpected.
fn unnest_sql<E: LifeModelTrait>(
    one_row_sql: &str,
    one_row_params: usize,
    ncols: usize,
    arrays: &[(OwnedArray, &'static str)],
    columns: &[E::Column],
) -> Option<String>
where
    E::Column: Copy,
{
    if one_row_params != ncols {
        return None;
    }
    let values_at = one_row_sql.find(" VALUES (")?;
    let close = one_row_sql[values_at..].find(')')? + values_at;
    let prefix = &one_row_sql[..values_at];
    let suffix = &one_row_sql[close + 1..];
    let mut sql = String::with_capacity(one_row_sql.len() + ncols * 24);
    sql.push_str(prefix);
    sql.push_str(" SELECT ");
    for (i, c) in columns.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("lg_u.c{i}"));
        // Text arrays feeding JSON columns need an explicit cast (no assignment cast exists).
        if matches!(arrays[i].0, OwnedArray::Text(_)) && is_json_column::<E>(*c) {
            sql.push_str("::jsonb");
        }
    }
    sql.push_str(" FROM unnest(");
    for (i, (_, ty)) in arrays.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("${}::{ty}[]", i + 1));
    }
    sql.push_str(") AS lg_u(");
    for i in 0..ncols {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("c{i}"));
    }
    sql.push(')');
    sql.push_str(suffix);
    Some(sql)
}

// ---------------------------------------------------------------------------------------------
// update_many / delete_many / find_by_ids

/// Update `columns` of many rows, each row keyed by its primary key, in one statement per chunk:
/// `UPDATE t SET c = lg_v.c .. FROM unnest(..) AS lg_v(..) WHERE t.pk = lg_v.pk`.
///
/// Each record runs `before_update` and its validators. Every record must have its primary key
/// and each of `columns` set.
///
/// # Errors
///
/// [`BatchError`] for the first failing chunk.
pub fn update_many<R>(
    executor: &dyn LifeExecutor,
    records: &[R],
    columns: &[<R::Entity as LifeModelTrait>::Column],
    opts: &BatchOptions,
) -> Result<BatchOutcome<<R::Entity as LifeModelTrait>::Model>, BatchError>
where
    R: ActiveModelTrait + ActiveModelBehavior,
    R::Entity: LifeEntityName + Default,
    <R::Entity as LifeModelTrait>::Column: Copy + Iden,
    <R::Entity as LifeModelTrait>::Model: FromRow,
{
    let mut out = BatchOutcome::default();
    if records.is_empty() || columns.is_empty() {
        return Ok(out);
    }
    let pk = primary_key_column::<R::Entity>().map_err(BatchError::before_any)?;
    let mut rows: Vec<Vec<Expr>> = Vec::with_capacity(records.len());
    for (i, r) in records.iter().enumerate() {
        let mut rec = r.clone();
        let bad = |source| BatchError {
            chunk: 0,
            rows: i..i + 1,
            rows_written_before: 0,
            source,
        };
        rec.before_update().map_err(bad)?;
        crate::active_model::run_validators(&rec, crate::active_model::ValidateOp::Update)
            .map_err(bad)?;
        let mut row = Vec::with_capacity(columns.len() + 1);
        let key = rec
            .get(pk)
            .ok_or_else(|| bad(ActiveModelError::PrimaryKeyRequired))?;
        row.push(Expr::Value(key));
        for c in columns {
            let v = rec.get(*c).ok_or_else(|| {
                bad(ActiveModelError::Other(format!(
                    "update_many: record {i} does not set column {}",
                    column_name(c)
                )))
            })?;
            row.push(Expr::Value(v));
        }
        rows.push(row);
    }
    let entity = R::Entity::default();
    let table = table_sql(&entity);
    let pk_name = quote_ident(&column_name(&pk));
    let metrics = <R::Entity as LifeModelTrait>::__lg_entity_metrics();
    let ncols = columns.len() + 1;
    for (chunk_index, range) in chunk_ranges(rows.len(), 1, opts).into_iter().enumerate() {
        let fail = |source: ActiveModelError, written: u64| BatchError {
            chunk: chunk_index,
            rows: range.clone(),
            rows_written_before: written,
            source,
        };
        let arrays = columns_as_arrays(&rows[range.clone()], ncols).ok_or_else(|| {
            fail(
                ActiveModelError::Other(
                    "update_many: a column has no array mapping (all NULL, mixed types or an expression)"
                        .to_string(),
                ),
                out.rows_affected,
            )
        })?;
        let mut sql = format!("UPDATE {table} AS lg_t SET ");
        for (i, c) in columns.iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }
            let cast = if matches!(arrays[i + 1].0, OwnedArray::Text(_))
                && is_json_column::<R::Entity>(*c)
            {
                "::jsonb"
            } else {
                ""
            };
            sql.push_str(&format!(
                "{} = lg_v.c{}{cast}",
                quote_ident(&column_name(c)),
                i + 1
            ));
        }
        sql.push_str(" FROM unnest(");
        for (i, (_, ty)) in arrays.iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }
            sql.push_str(&format!("${}::{ty}[]", i + 1));
        }
        sql.push_str(") AS lg_v(");
        for i in 0..ncols {
            if i > 0 {
                sql.push_str(", ");
            }
            sql.push_str(&format!("c{i}"));
        }
        sql.push_str(&format!(") WHERE lg_t.{pk_name} = lg_v.c0"));
        if opts.returning {
            sql.push_str(" RETURNING lg_t.*");
        }
        let params: Vec<OwnedParam> = arrays
            .into_iter()
            .map(|(a, _)| OwnedParam::Array(a))
            .collect();
        let timer =
            crate::metrics::entity::OpTimer::start(metrics, crate::metrics::entity::Op::Update);
        if opts.returning {
            let res = executor.__lg_query_all_owned(&sql, params);
            timer.finish(res.as_ref().map_or(0, |r| r.len() as u64), res.is_ok());
            let back = res.map_err(|e| fail(db(&e), out.rows_affected))?;
            out.rows_affected += back.len() as u64;
            for row in &back {
                out.models.push(
                    <<R::Entity as LifeModelTrait>::Model as FromRow>::from_row(row).map_err(
                        |e| {
                            fail(
                                ActiveModelError::DatabaseError(e.to_string()),
                                out.rows_affected,
                            )
                        },
                    )?,
                );
            }
        } else {
            let res = executor.__lg_execute_owned(&sql, params);
            timer.finish(*res.as_ref().unwrap_or(&0), res.is_ok());
            out.rows_affected += res.map_err(|e| fail(db(&e), out.rows_affected))?;
        }
        out.statements += 1;
    }
    Ok(out)
}

fn key_array(ids: &[Value]) -> Result<(OwnedArray, &'static str), ActiveModelError> {
    let first = ids
        .iter()
        .find(|v| !value_is_null(v))
        .ok_or_else(|| ActiveModelError::Other("batch by key: no non-NULL keys".to_string()))?;
    let (mut arr, ty) = OwnedArray::for_value(first).ok_or_else(|| {
        ActiveModelError::Other(format!("batch by key: unsupported key type {first:?}"))
    })?;
    for v in ids {
        if !arr.push(v) {
            return Err(ActiveModelError::Other(format!(
                "batch by key: key {v:?} does not match the type of the first key"
            )));
        }
    }
    Ok((arr, ty))
}

/// Delete rows by primary key: `DELETE FROM t WHERE pk = ANY($1)`, one statement per chunk.
///
/// This is a direct delete: per-record `before_delete` hooks do not run (there are no records).
///
/// # Errors
///
/// [`BatchError`] for the first failing chunk.
pub fn delete_many<E>(
    executor: &dyn LifeExecutor,
    ids: &[Value],
    opts: &BatchOptions,
) -> Result<BatchOutcome<E::Model>, BatchError>
where
    E: LifeModelTrait + LifeEntityName + Default,
    E::Column: Copy + Iden,
    E::Model: FromRow,
{
    let mut out = BatchOutcome::default();
    if ids.is_empty() {
        return Ok(out);
    }
    let pk = primary_key_column::<E>().map_err(BatchError::before_any)?;
    let table = table_sql(&E::default());
    let pk_name = quote_ident(&column_name(&pk));
    let metrics = E::__lg_entity_metrics();
    for (chunk_index, range) in chunk_ranges(ids.len(), 1, opts).into_iter().enumerate() {
        let fail = |source: ActiveModelError, written: u64| BatchError {
            chunk: chunk_index,
            rows: range.clone(),
            rows_written_before: written,
            source,
        };
        let (arr, ty) = key_array(&ids[range.clone()]).map_err(|e| fail(e, out.rows_affected))?;
        let mut sql = format!("DELETE FROM {table} WHERE {pk_name} = ANY($1::{ty}[])");
        if opts.returning {
            sql.push_str(" RETURNING *");
        }
        let timer =
            crate::metrics::entity::OpTimer::start(metrics, crate::metrics::entity::Op::Delete);
        if opts.returning {
            let res = executor.__lg_query_all_owned(&sql, vec![OwnedParam::Array(arr)]);
            timer.finish(res.as_ref().map_or(0, |r| r.len() as u64), res.is_ok());
            let back = res.map_err(|e| fail(db(&e), out.rows_affected))?;
            out.rows_affected += back.len() as u64;
            for row in &back {
                out.models.push(E::Model::from_row(row).map_err(|e| {
                    fail(
                        ActiveModelError::DatabaseError(e.to_string()),
                        out.rows_affected,
                    )
                })?);
            }
        } else {
            let res = executor.__lg_execute_owned(&sql, vec![OwnedParam::Array(arr)]);
            timer.finish(*res.as_ref().unwrap_or(&0), res.is_ok());
            out.rows_affected += res.map_err(|e| fail(db(&e), out.rows_affected))?;
        }
        out.statements += 1;
    }
    Ok(out)
}

/// Load the rows whose primary key is in `ids`: `SELECT .. FROM t WHERE pk = ANY($1)` (the
/// entity's normal projection and soft-delete filter apply). One statement per chunk of
/// [`BatchOptions::max_rows`] keys, whatever their number, instead of one per key.
///
/// Rows come back in database order, not in `ids` order; missing keys are simply absent.
/// Reads run on the executor's primary tier.
///
/// # Errors
///
/// [`BatchError`] for the first failing chunk.
pub fn find_by_ids<E>(
    executor: &dyn LifeExecutor,
    ids: &[Value],
    opts: &BatchOptions,
) -> Result<Vec<E::Model>, BatchError>
where
    E: LifeModelTrait + LifeEntityName + Default,
    E::Column: Copy + Iden,
    E::Model: FromRow,
{
    let mut models = Vec::with_capacity(ids.len());
    if ids.is_empty() {
        return Ok(models);
    }
    let pk = primary_key_column::<E>().map_err(BatchError::before_any)?;
    let pk_name = quote_ident(&column_name(&pk));
    let metrics = E::__lg_entity_metrics();
    const MARK: &str = "lg_batch_ids_marker";
    for (chunk_index, range) in chunk_ranges(ids.len(), 1, opts).into_iter().enumerate() {
        let fail = |source: ActiveModelError| BatchError {
            chunk: chunk_index,
            rows: range.clone(),
            rows_written_before: 0,
            source,
        };
        let (arr, ty) = key_array(&ids[range.clone()]).map_err(fail)?;
        // The entity's own SELECT (projection, soft delete) with a marker condition that is then
        // replaced by the array predicate, numbered after SeaQuery's own parameters.
        let select = E::find().filter(Expr::cust(MARK));
        let (sql, values) = select.query.build(PostgresQueryBuilder);
        let mut params = values_to_owned(&values).map_err(fail)?;
        params.push(OwnedParam::Array(arr));
        let sql = sql.replacen(
            MARK,
            &format!("{pk_name} = ANY(${}::{ty}[])", params.len()),
            1,
        );
        let timer =
            crate::metrics::entity::OpTimer::start(metrics, crate::metrics::entity::Op::Select);
        let res = executor.__lg_query_all_owned(&sql, params);
        timer.finish(res.as_ref().map_or(0, |r| r.len() as u64), res.is_ok());
        let rows = res.map_err(|e| fail(db(&e)))?;
        for row in &rows {
            models.push(
                E::Model::from_row(row)
                    .map_err(|e| fail(ActiveModelError::DatabaseError(e.to_string())))?,
            );
        }
    }
    Ok(models)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn chunks_respect_row_and_param_caps() {
        let opts = BatchOptions {
            max_rows: 1000,
            max_params: 100,
            ..BatchOptions::default()
        };
        // 10 columns: at most 10 rows per chunk by the parameter cap.
        let r = chunk_ranges(25, 10, &opts);
        assert_eq!(r, vec![0..10, 10..20, 20..25]);
        let r = chunk_ranges(5, 1, &opts);
        assert_eq!(r, vec![0..5]);
        // Never above PostgreSQL's limit even if asked.
        let big = BatchOptions {
            max_rows: usize::MAX,
            max_params: usize::MAX,
            ..BatchOptions::default()
        };
        let r = chunk_ranges(100_000, 3, &big);
        assert!(r.iter().all(|c| c.len() * 3 <= POSTGRES_MAX_PARAMS));
    }

    #[test]
    fn arrays_from_plain_values_only() {
        let rows = vec![
            vec![
                Expr::Value(Value::BigInt(Some(1))),
                Expr::Value(Value::String(Some("a".into()))),
            ],
            vec![
                Expr::Value(Value::BigInt(Some(2))),
                Expr::Value(Value::String(None)),
            ],
        ];
        let arrays = columns_as_arrays(&rows, 2).expect("plain values map to arrays");
        assert_eq!(arrays[0].1, "int8");
        assert_eq!(arrays[1].1, "text");
        assert_eq!(arrays[1].0.len(), 2);
        // An all-NULL column has no type to cast to: no unnest.
        let nulls = vec![vec![Expr::Value(Value::Int(None))]];
        assert!(columns_as_arrays(&nulls, 1).is_none());
        // Expressions (save_as) are not values: no unnest.
        let exprs = vec![vec![Expr::cust("now()")]];
        assert!(columns_as_arrays(&exprs, 1).is_none());
    }

    #[test]
    fn identifiers_are_quoted() {
        assert_eq!(quote_ident("we\"ird"), "\"we\"\"ird\"");
    }

    #[test]
    fn batch_error_names_its_chunk() {
        let e = BatchError {
            chunk: 2,
            rows: 2000..3000,
            rows_written_before: 2000,
            source: ActiveModelError::Other("boom".into()),
        };
        let s = e.to_string();
        assert!(
            s.contains("chunk 2") && s.contains("2000..3000") && s.contains("boom"),
            "{s}"
        );
    }
}
