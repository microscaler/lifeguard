//! Identity enum for representing single and composite column references
//!
//! This module provides the `Identity` enum which can represent either a single column
//! or a composite key consisting of multiple columns. This is used by `RelationDef`
//! to handle both single and composite primary/foreign keys.

use sea_query::DynIden;

/// Represents a column identifier that can be single or composite
///
/// This enum is used to represent primary keys and foreign keys that may consist
/// of one or more columns. It supports:
/// - Single column keys (`Unary`)
/// - Two column composite keys (`Binary`)
/// - Three column composite keys (`Ternary`)
/// - Four or more column composite keys (`Many`)
///
/// # Example
///
/// ```no_run
/// use lifeguard::relation::identity::Identity;
/// use sea_query::DynIden;
///
/// // Single column
/// let id_col: DynIden = "id".into();
/// let identity = Identity::Unary(id_col);
///
/// // Composite key (2 columns)
/// let id_col: DynIden = "id".into();
/// let tenant_col: DynIden = "tenant_id".into();
/// let identity = Identity::Binary(id_col, tenant_col);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Identity {
    /// Single column identifier
    Unary(DynIden),
    /// Two column identifiers (composite key)
    Binary(DynIden, DynIden),
    /// Three column identifiers (composite key)
    Ternary(DynIden, DynIden, DynIden),
    /// Four or more column identifiers (composite key)
    Many(Vec<DynIden>),
}

impl Identity {
    /// Get the arity (number of columns) for this identity
    ///
    /// # Returns
    ///
    /// The number of columns in this identity (1 for Unary, 2 for Binary, etc.)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::relation::identity::Identity;
    ///
    /// let id_col: DynIden = "id".into();
    /// let identity = Identity::Unary(id_col);
    /// assert_eq!(identity.arity(), 1);
    ///
    /// let identity2 = Identity::Binary(id_col.clone(), id_col.clone());
    /// assert_eq!(identity2.arity(), 2);
    /// ```
    pub fn arity(&self) -> usize {
        match self {
            Self::Unary(_) => 1,
            Self::Binary(_, _) => 2,
            Self::Ternary(_, _, _) => 3,
            Self::Many(vec) => vec.len(),
        }
    }

    /// Iterate over column identifiers
    ///
    /// Returns an iterator that yields references to each `DynIden` in this identity.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::relation::identity::Identity;
    ///
    /// let id_col: DynIden = "id".into();
    /// let tenant_col: DynIden = "tenant_id".into();
    /// let identity = Identity::Binary(id_col.clone(), tenant_col.clone());
    ///
    /// let columns: Vec<&DynIden> = identity.iter().collect();
    /// assert_eq!(columns.len(), 2);
    /// ```
    pub fn iter(&self) -> BorrowedIdentityIter<'_> {
        BorrowedIdentityIter {
            identity: self,
            index: 0,
        }
    }

    /// Check if this identity contains a specific column
    ///
    /// # Arguments
    ///
    /// * `col` - The column identifier to check for
    ///
    /// # Returns
    ///
    /// `true` if the identity contains the column, `false` otherwise
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::relation::identity::Identity;
    ///
    /// let id_col: DynIden = "id".into();
    /// let tenant_col: DynIden = "tenant_id".into();
    /// let identity = Identity::Binary(id_col.clone(), tenant_col.clone());
    ///
    /// assert!(identity.contains(&id_col));
    /// assert!(identity.contains(&tenant_col));
    /// ```
    pub fn contains(&self, col: &DynIden) -> bool {
        self.iter().any(|c| c == col)
    }

    /// Check if this identity is a superset of another identity
    ///
    /// Returns `true` if all columns in `other` are present in `self`.
    ///
    /// # Arguments
    ///
    /// * `other` - The other identity to check against
    ///
    /// # Returns
    ///
    /// `true` if `self` contains all columns from `other`
    pub fn fully_contains(&self, other: &Identity) -> bool {
        for col in other.iter() {
            if !self.contains(col) {
                return false;
            }
        }
        true
    }
}

/// Iterator for borrowed references to `DynIden` in an `Identity`
///
/// This iterator allows you to iterate over the columns in an `Identity` without
/// consuming it.
#[derive(Debug)]
pub struct BorrowedIdentityIter<'a> {
    identity: &'a Identity,
    index: usize,
}

impl<'a> Iterator for BorrowedIdentityIter<'a> {
    type Item = &'a DynIden;

    fn next(&mut self) -> Option<Self::Item> {
        let result = match self.identity {
            Identity::Unary(iden1) => {
                if self.index == 0 {
                    Some(iden1)
                } else {
                    None
                }
            }
            Identity::Binary(iden1, iden2) => match self.index {
                0 => Some(iden1),
                1 => Some(iden2),
                _ => None,
            },
            Identity::Ternary(iden1, iden2, iden3) => match self.index {
                0 => Some(iden1),
                1 => Some(iden2),
                2 => Some(iden3),
                _ => None,
            },
            Identity::Many(vec) => vec.get(self.index),
        };
        if result.is_some() {
            self.index += 1;
        }
        result
    }
}

/// Trait for converting types to `Identity`
///
/// This trait allows types (like Column enums) to be converted into an `Identity`.
/// The macro will generate implementations for Column enums.
pub trait IntoIdentity {
    /// Convert this type into an `Identity`
    fn into_identity(self) -> Identity;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_arity() {
        let id_col: DynIden = "id".into();
        
        assert_eq!(Identity::Unary(id_col.clone()).arity(), 1);
        assert_eq!(Identity::Binary(id_col.clone(), id_col.clone()).arity(), 2);
        assert_eq!(
            Identity::Ternary(id_col.clone(), id_col.clone(), id_col.clone()).arity(),
            3
        );
        assert_eq!(
            Identity::Many(vec![id_col.clone(), id_col.clone(), id_col.clone(), id_col.clone()]).arity(),
            4
        );
    }

    #[test]
    fn test_identity_iter() {
        let id_col: DynIden = "id".into();
        let tenant_col: DynIden = "tenant_id".into();
        let region_col: DynIden = "region_id".into();

        // Test Unary
        let identity = Identity::Unary(id_col.clone());
        let columns: Vec<&DynIden> = identity.iter().collect();
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0], &id_col);

        // Test Binary
        let identity = Identity::Binary(id_col.clone(), tenant_col.clone());
        let columns: Vec<&DynIden> = identity.iter().collect();
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0], &id_col);
        assert_eq!(columns[1], &tenant_col);

        // Test Ternary
        let identity = Identity::Ternary(id_col.clone(), tenant_col.clone(), region_col.clone());
        let columns: Vec<&DynIden> = identity.iter().collect();
        assert_eq!(columns.len(), 3);
        assert_eq!(columns[0], &id_col);
        assert_eq!(columns[1], &tenant_col);
        assert_eq!(columns[2], &region_col);

        // Test Many
        let identity = Identity::Many(vec![id_col.clone(), tenant_col.clone(), region_col.clone()]);
        let columns: Vec<&DynIden> = identity.iter().collect();
        assert_eq!(columns.len(), 3);
        assert_eq!(columns[0], &id_col);
        assert_eq!(columns[1], &tenant_col);
        assert_eq!(columns[2], &region_col);
    }

    #[test]
    fn test_identity_contains() {
        let id_col: DynIden = "id".into();
        let tenant_col: DynIden = "tenant_id".into();
        let other_col: DynIden = "other".into();

        let identity = Identity::Binary(id_col.clone(), tenant_col.clone());

        assert!(identity.contains(&id_col));
        assert!(identity.contains(&tenant_col));
        assert!(!identity.contains(&other_col));
    }

    #[test]
    fn test_identity_fully_contains() {
        let id_col: DynIden = "id".into();
        let tenant_col: DynIden = "tenant_id".into();
        let region_col: DynIden = "region_id".into();

        let identity1 = Identity::Binary(id_col.clone(), tenant_col.clone());
        let identity2 = Identity::Unary(id_col.clone());
        let identity3 = Identity::Ternary(id_col.clone(), tenant_col.clone(), region_col.clone());

        // identity1 contains identity2 (id is in Binary)
        assert!(identity1.fully_contains(&identity2));
        
        // identity1 does not fully contain identity3 (missing region_col)
        assert!(!identity1.fully_contains(&identity3));
        
        // identity3 fully contains identity1 (has both id and tenant_id)
        assert!(identity3.fully_contains(&identity1));
    }

    #[test]
    fn test_identity_many_large() {
        // Edge case: Many variant with 5+ columns
        let cols: Vec<DynIden> = (0..6).map(|i| format!("col_{}", i).into()).collect();
        let identity = Identity::Many(cols.clone());
        
        assert_eq!(identity.arity(), 6);
        let collected: Vec<&DynIden> = identity.iter().collect();
        assert_eq!(collected.len(), 6);
    }

    #[test]
    fn test_identity_many_empty() {
        // Edge case: Empty Many variant (shouldn't happen in practice, but test for safety)
        let identity = Identity::Many(vec![]);
        assert_eq!(identity.arity(), 0);
        let collected: Vec<&DynIden> = identity.iter().collect();
        assert_eq!(collected.len(), 0);
    }

    #[test]
    fn test_identity_many_duplicate_columns() {
        // Edge case: Many variant with duplicate columns
        let id_col: DynIden = "id".into();
        let identity = Identity::Many(vec![id_col.clone(), id_col.clone(), id_col.clone()]);
        
        assert_eq!(identity.arity(), 3);
        assert!(identity.contains(&id_col));
        // All three should be the same column
        let collected: Vec<&DynIden> = identity.iter().collect();
        assert_eq!(collected.len(), 3);
        assert_eq!(collected[0], &id_col);
        assert_eq!(collected[1], &id_col);
        assert_eq!(collected[2], &id_col);
    }

    #[test]
    fn test_identity_iter_multiple_iterations() {
        // Edge case: Iterator can be used multiple times
        let id_col: DynIden = "id".into();
        let tenant_col: DynIden = "tenant_id".into();
        let identity = Identity::Binary(id_col.clone(), tenant_col.clone());
        
        let iter1: Vec<&DynIden> = identity.iter().collect();
        let iter2: Vec<&DynIden> = identity.iter().collect();
        
        assert_eq!(iter1.len(), 2);
        assert_eq!(iter2.len(), 2);
        assert_eq!(iter1, iter2);
    }

    #[test]
    fn test_identity_contains_unary() {
        // Edge case: Contains check on Unary
        let id_col: DynIden = "id".into();
        let other_col: DynIden = "other".into();
        let identity = Identity::Unary(id_col.clone());
        
        assert!(identity.contains(&id_col));
        assert!(!identity.contains(&other_col));
    }

    #[test]
    fn test_identity_fully_contains_self() {
        // Edge case: Identity fully contains itself
        let id_col: DynIden = "id".into();
        let tenant_col: DynIden = "tenant_id".into();
        let identity = Identity::Binary(id_col.clone(), tenant_col.clone());
        
        assert!(identity.fully_contains(&identity));
    }

    #[test]
    fn test_identity_fully_contains_many() {
        // Edge case: Many variant fully contains smaller identities
        let id_col: DynIden = "id".into();
        let tenant_col: DynIden = "tenant_id".into();
        let region_col: DynIden = "region_id".into();
        let extra_col: DynIden = "extra".into();
        
        let many_identity = Identity::Many(vec![
            id_col.clone(),
            tenant_col.clone(),
            region_col.clone(),
            extra_col.clone(),
        ]);
        let binary_identity = Identity::Binary(id_col.clone(), tenant_col.clone());
        let unary_identity = Identity::Unary(id_col.clone());
        
        assert!(many_identity.fully_contains(&binary_identity));
        assert!(many_identity.fully_contains(&unary_identity));
        assert!(!binary_identity.fully_contains(&many_identity));
    }
}
