//! Primary key operations for LifeModel - Epic 02 Story 06
//!
//! This module provides traits and implementations for type-safe primary key operations
//! that match SeaORM's API. Primary keys can be queried and manipulated with compile-time type checking.

/// Trait for primary key operations
///
/// This trait provides methods for working with primary keys at compile time.
/// It's similar to SeaORM's `PrimaryKeyTrait` and allows type-safe primary key operations.
///
/// # Example
///
/// ```no_run
/// use lifeguard::PrimaryKeyTrait;
///
/// // In a real application, the macro would generate this:
/// // impl PrimaryKeyTrait for UserPrimaryKey {
/// //     type ValueType = i32;
/// //     fn auto_increment(self) -> bool {
/// //         match self {
/// //             UserPrimaryKey::Id => true,
/// //         }
/// //     }
/// // }
/// ```
pub trait PrimaryKeyTrait: Copy + std::fmt::Debug {
    /// The value type of the primary key
    ///
    /// This associated type represents the Rust type that the primary key value
    /// should be converted to/from. For example, `i32` for integer primary keys,
    /// `String` for string primary keys, etc.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::PrimaryKeyTrait;
    ///
    /// // For a primary key with type i32:
    /// // type ValueType = i32;
    /// ```
    type ValueType;

    /// Check if the primary key is auto-increment
    ///
    /// Returns `true` if the primary key column has the `#[auto_increment]` attribute,
    /// `false` otherwise.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::PrimaryKeyTrait;
    ///
    /// // In a real application, the macro would generate this:
    /// // impl PrimaryKeyTrait for UserPrimaryKey {
    /// //     fn auto_increment(self) -> bool {
    /// //         match self {
    /// //             UserPrimaryKey::Id => true,  // Has #[auto_increment]
    /// //         }
    /// //     }
    /// // }
    /// ```
    fn auto_increment(self) -> bool;
}

/// Trait for mapping between PrimaryKey and Column
///
/// This trait provides a way to convert a PrimaryKey enum variant to its
/// corresponding Column enum variant. This is useful for queries that need
/// to reference the primary key column.
///
/// # Example
///
/// ```no_run
/// use lifeguard::PrimaryKeyToColumn;
///
/// // In a real application, the macro would generate this:
/// // impl PrimaryKeyToColumn for UserPrimaryKey {
/// //     type Column = UserColumn;
    /// //     fn to_column(self) -> Self::Column {
    /// //         match self {
    /// //             UserPrimaryKey::Id => UserColumn::Id,
    /// //         }
    /// //     }
    /// // }
/// ```
pub trait PrimaryKeyToColumn {
    /// The Column enum type for this entity
    type Column;

    /// Convert a PrimaryKey variant to its corresponding Column variant
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::PrimaryKeyToColumn;
    ///
    /// // let pk = UserPrimaryKey::Id;
    /// // let col = pk.to_column();  // Returns UserColumn::Id
    /// ```
    fn to_column(self) -> Self::Column;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test PrimaryKey enum for PrimaryKeyTrait tests
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    enum TestPrimaryKey {
        Id,
    }

    // Test Column enum for PrimaryKeyToColumn tests
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    enum TestColumn {
        Id,
    }

    // Manual implementation for testing
    impl PrimaryKeyTrait for TestPrimaryKey {
        type ValueType = i32;

        fn auto_increment(self) -> bool {
            match self {
                TestPrimaryKey::Id => true,
            }
        }
    }

    impl PrimaryKeyToColumn for TestPrimaryKey {
        type Column = TestColumn;

        fn to_column(self) -> Self::Column {
            match self {
                TestPrimaryKey::Id => TestColumn::Id,
            }
        }
    }

    #[test]
    fn test_primary_key_trait_auto_increment() {
        let pk = TestPrimaryKey::Id;
        assert_eq!(pk.auto_increment(), true);
    }

    #[test]
    fn test_primary_key_to_column() {
        let pk = TestPrimaryKey::Id;
        let col = pk.to_column();
        assert_eq!(col, TestColumn::Id);
    }

    #[test]
    fn test_primary_key_value_type() {
        // Test that ValueType is accessible
        let _value: <TestPrimaryKey as PrimaryKeyTrait>::ValueType = 42i32;
    }
}
