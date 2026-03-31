//! Named **scopes**: reusable, composable predicates on [`SelectQuery`].
//!
//! # Pattern
//!
//! Define associated functions (or constants) that return anything implementing
//! [`sea_query::IntoCondition`] (e.g. [`sea_query::Expr`] from [`crate::ColumnTrait`]),
//! then chain with [`SelectQuery::scope`] (or [`SelectQuery::filter`]). Prefer the
//! [`crate::scope`] attribute on `impl Entity` so `fn active()` becomes `scope_active()`
//! (see `lifeguard-derive` / PRD Phase C).
//!
//! ```ignore
//! User::find()
//!     .scope(UserEntity::scope_active())
//!     .scope(UserEntity::scope_published())
//! ```
//!
//! # Composition
//!
//! Each [`SelectQuery::scope`] call **AND**s its condition with the rest of the `WHERE`
//! clause (same as [`SelectQuery::filter`]). Use [`SelectQuery::scope_or`] or
//! [`SelectQuery::scope_any`] when you need **OR** between predicates (PRD SC-2).
//!
//! # Soft delete
//!
//! If the entity implements [`crate::LifeModelTrait::soft_delete_column`], execution
//! methods ([`crate::SelectQuery::all`], [`crate::SelectQuery::one`], …) append
//! `deleted_at IS NULL` (or the configured column) **unless** [`SelectQuery::with_trashed`]
//! was used. That predicate is **AND**ed with every `scope` / `filter` you added. Scopes do
//! not replace the global soft-delete filter.

use crate::query::select::SelectQuery;
use crate::query::traits::LifeModelTrait;
use sea_query::Condition;

/// Something that can be applied to a [`SelectQuery`] as a named scope.
///
/// Implemented for all [`sea_query::IntoCondition`] types (column expressions, [`sea_query::Condition`], etc.).
pub trait IntoScope<E: LifeModelTrait> {
    /// Apply this scope by ANDing its condition onto the query.
    fn apply_scope(self, query: SelectQuery<E>) -> SelectQuery<E>;
}

impl<E, C> IntoScope<E> for C
where
    E: LifeModelTrait,
    C: sea_query::IntoCondition,
{
    fn apply_scope(self, query: SelectQuery<E>) -> SelectQuery<E> {
        query.filter(self)
    }
}

impl<E> SelectQuery<E>
where
    E: LifeModelTrait,
{
    /// Apply a **named scope**: same as [`SelectQuery::filter`], but documents intent
    /// (Rails/Django-style reusable predicates).
    #[must_use]
    pub fn scope<S: IntoScope<E>>(self, s: S) -> Self {
        s.apply_scope(self)
    }

    /// OR two scope conditions: `(a) OR (b)` (PRD SC-2).
    #[must_use]
    pub fn scope_or<A, B>(self, a: A, b: B) -> Self
    where
        A: sea_query::IntoCondition,
        B: sea_query::IntoCondition,
    {
        let c = Condition::any()
            .add(a.into_condition())
            .add(b.into_condition());
        self.filter(c)
    }

    /// OR an iterator of conditions. Empty iterator returns `self` unchanged.
    #[must_use]
    pub fn scope_any<I, C>(self, iter: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: sea_query::IntoCondition,
    {
        let mut it = iter.into_iter();
        let Some(first) = it.next() else {
            return self;
        };
        let mut c = Condition::any().add(first.into_condition());
        for cond in it {
            c = c.add(cond.into_condition());
        }
        self.filter(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::column::column_trait::ColumnDefHelper;
    use crate::query::column::definition::ColumnDefinition;
    use crate::query::traits::{LifeEntityName, LifeModelTrait};
    use crate::ColumnTrait;
    use sea_query::PostgresQueryBuilder;

    #[derive(Copy, Clone, Default, Debug)]
    struct ScopeTestEntity;

    impl LifeEntityName for ScopeTestEntity {
        fn table_name(&self) -> &'static str {
            "scope_test"
        }
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    enum ScopeTestColumn {
        Id,
        Status,
    }

    impl sea_query::Iden for ScopeTestColumn {
        fn unquoted(&self) -> &'static str {
            match self {
                ScopeTestColumn::Id => "id",
                ScopeTestColumn::Status => "status",
            }
        }
    }

    impl sea_query::IdenStatic for ScopeTestColumn {
        fn as_str(&self) -> &'static str {
            match self {
                ScopeTestColumn::Id => "id",
                ScopeTestColumn::Status => "status",
            }
        }
    }

    impl ScopeTestColumn {
        fn all_columns() -> &'static [ScopeTestColumn] {
            static COLS: &[ScopeTestColumn] = &[ScopeTestColumn::Id, ScopeTestColumn::Status];
            COLS
        }
    }

    impl ColumnDefHelper for ScopeTestColumn {
        fn column_def(self) -> ColumnDefinition {
            match self {
                ScopeTestColumn::Id => ColumnDefinition {
                    column_type: Some("Integer".to_string()),
                    nullable: false,
                    ..Default::default()
                },
                ScopeTestColumn::Status => ColumnDefinition {
                    column_type: Some("String".to_string()),
                    nullable: false,
                    ..Default::default()
                },
            }
        }
    }

    struct ScopeTestModel;

    impl LifeModelTrait for ScopeTestEntity {
        type Model = ScopeTestModel;
        type Column = ScopeTestColumn;

        fn all_columns() -> &'static [Self::Column] {
            ScopeTestColumn::all_columns()
        }
    }

    impl ScopeTestEntity {
        fn scope_status_eq(value: i32) -> sea_query::SimpleExpr {
            ScopeTestColumn::Status.eq(value)
        }
    }

    #[test]
    fn scope_chains_with_and_like_filter() {
        let q = SelectQuery::<ScopeTestEntity>::new()
            .scope(ScopeTestEntity::scope_status_eq(1))
            .scope(ScopeTestColumn::Id.gt(10i32));

        let (sql, _) = q.query.build(PostgresQueryBuilder);
        let s = sql.to_uppercase();
        assert!(
            s.contains("STATUS") && s.contains("ID"),
            "expected both columns in WHERE: {sql}"
        );
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    enum SoftCol {
        Id,
        DeletedAt,
    }

    impl sea_query::Iden for SoftCol {
        fn unquoted(&self) -> &'static str {
            match self {
                SoftCol::Id => "id",
                SoftCol::DeletedAt => "deleted_at",
            }
        }
    }

    impl sea_query::IdenStatic for SoftCol {
        fn as_str(&self) -> &'static str {
            match self {
                SoftCol::Id => "id",
                SoftCol::DeletedAt => "deleted_at",
            }
        }
    }

    impl SoftCol {
        fn all_columns() -> &'static [SoftCol] {
            static COLS: &[SoftCol] = &[SoftCol::Id, SoftCol::DeletedAt];
            COLS
        }
    }

    impl ColumnDefHelper for SoftCol {
        fn column_def(self) -> ColumnDefinition {
            match self {
                SoftCol::Id => ColumnDefinition {
                    column_type: Some("Integer".to_string()),
                    nullable: false,
                    ..Default::default()
                },
                SoftCol::DeletedAt => ColumnDefinition {
                    column_type: Some("DateTime".to_string()),
                    nullable: true,
                    ..Default::default()
                },
            }
        }
    }

    #[derive(Copy, Clone, Default, Debug)]
    struct SoftEntity;

    impl LifeEntityName for SoftEntity {
        fn table_name(&self) -> &'static str {
            "soft_scope"
        }
    }

    struct SoftModel;

    impl LifeModelTrait for SoftEntity {
        type Model = SoftModel;
        type Column = SoftCol;

        fn all_columns() -> &'static [Self::Column] {
            SoftCol::all_columns()
        }

        fn soft_delete_column() -> Option<Self::Column> {
            Some(SoftCol::DeletedAt)
        }
    }

    #[test]
    fn scope_and_soft_delete_both_anded_at_execution() {
        let q = SelectQuery::<SoftEntity>::new().scope(SoftCol::Id.eq(5i32));
        let stmt = q.apply_soft_delete();
        let (sql, _) = stmt.build(PostgresQueryBuilder);
        let upper = sql.to_uppercase();
        assert!(
            upper.contains("DELETED_AT") && upper.contains("IS NULL"),
            "soft delete: {sql}"
        );
        assert!(
            upper.contains("\"ID\"") || upper.contains("ID"),
            "user scope id: {sql}"
        );
    }

    #[test]
    fn scope_or_produces_or_in_sql() {
        let q = SelectQuery::<ScopeTestEntity>::new().scope_or(
            ScopeTestColumn::Status.eq(1i32),
            ScopeTestColumn::Status.eq(2i32),
        );
        let (sql, _) = q.query.build(PostgresQueryBuilder);
        let upper = sql.to_uppercase();
        assert!(upper.contains(" OR "), "expected OR in WHERE: {sql}");
    }

    #[test]
    fn scope_any_empty_is_noop() {
        let q = SelectQuery::<ScopeTestEntity>::new();
        let q2 = q.clone().scope_any(std::iter::empty::<sea_query::SimpleExpr>());
        let (s1, _) = q.query.build(PostgresQueryBuilder);
        let (s2, _) = q2.query.build(PostgresQueryBuilder);
        assert_eq!(s1, s2);
    }

    #[test]
    fn scope_any_three_branches() {
        let q = SelectQuery::<ScopeTestEntity>::new().scope_any([
            ScopeTestColumn::Status.eq(1i32),
            ScopeTestColumn::Status.eq(2i32),
            ScopeTestColumn::Id.eq(99i32),
        ]);
        let (sql, _) = q.query.build(PostgresQueryBuilder);
        let upper = sql.to_uppercase();
        assert!(upper.matches(" OR ").count() >= 2, "expected multiple OR: {sql}");
    }
}
