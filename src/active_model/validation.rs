//! Orchestrates [`super::traits::ActiveModelBehavior::validate_fields`] then
//! [`super::traits::ActiveModelBehavior::validate_model`] (fail-fast).

use super::error::ActiveModelError;
use super::traits::ActiveModelBehavior;
use super::validate_op::ValidateOp;

/// Run field-level then model-level validation (PRD ordering: field → model).
#[inline]
pub fn run_validators<R: ActiveModelBehavior>(record: &R, op: ValidateOp) -> Result<(), ActiveModelError> {
    record.validate_fields(op)?;
    record.validate_model(op)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::error::ActiveModelError;
    use super::super::traits::{ActiveModelBehavior, ActiveModelTrait};
    use super::super::validate_op::{ValidateOp, ValidationError};
    use super::run_validators;
    use crate::LifeModelTrait;
    use sea_query::{Iden, IdenStatic};

    #[derive(Copy, Clone, Debug)]
    enum TestColumn {
        Id,
    }

    impl Iden for TestColumn {
        fn unquoted(&self) -> &'static str {
            "id"
        }
    }

    impl IdenStatic for TestColumn {
        fn as_str(&self) -> &'static str {
            "id"
        }
    }

    crate::impl_column_def_helper_for_test!(TestColumn);

    #[derive(Copy, Clone, Debug, Default)]
    struct TestEntity;

    impl crate::LifeEntityName for TestEntity {
        fn table_name(&self) -> &'static str {
            "test_entities"
        }
    }

    #[derive(Clone, Debug)]
    struct TestModel;

    impl crate::ModelTrait for TestModel {
        type Entity = TestEntity;
        fn get(&self, _column: TestColumn) -> sea_query::Value {
            sea_query::Value::Int(Some(1))
        }
        fn set(
            &mut self,
            _column: TestColumn,
            _value: sea_query::Value,
        ) -> Result<(), crate::ModelError> {
            Ok(())
        }
        fn get_primary_key_value(&self) -> sea_query::Value {
            sea_query::Value::Int(Some(1))
        }
        fn get_primary_key_identity(&self) -> crate::Identity {
            use crate::relation::identity::Identity;
            use sea_query::IntoIden;
            Identity::Unary(TestColumn::Id.into_iden())
        }
        fn get_primary_key_values(&self) -> Vec<sea_query::Value> {
            vec![sea_query::Value::Int(Some(1))]
        }
    }

    impl LifeModelTrait for TestEntity {
        type Model = TestModel;
        type Column = TestColumn;
    }

    // Minimal stub: only `run_validators` is exercised.
    #[derive(Clone, Debug)]
    struct StubRecord {
        order: std::cell::RefCell<Vec<&'static str>>,
        fail_field: bool,
        fail_model: bool,
    }

    impl ActiveModelTrait for StubRecord {
        type Entity = TestEntity;
        type Model = TestModel;

        fn get(&self, _column: TestColumn) -> Option<sea_query::Value> {
            None
        }

        fn set(
            &mut self,
            _column: TestColumn,
            _value: sea_query::Value,
        ) -> Result<(), ActiveModelError> {
            Ok(())
        }

        fn take(&mut self, _column: TestColumn) -> Option<sea_query::Value> {
            None
        }

        fn reset(&mut self) {}

        fn insert(
            &self,
            _executor: &dyn crate::executor::LifeExecutor,
        ) -> Result<Self::Model, ActiveModelError> {
            Err(ActiveModelError::Other("stub".to_string()))
        }

        fn update(
            &self,
            _executor: &dyn crate::executor::LifeExecutor,
        ) -> Result<Self::Model, ActiveModelError> {
            Err(ActiveModelError::Other("stub".to_string()))
        }

        fn save(
            &self,
            _executor: &dyn crate::executor::LifeExecutor,
        ) -> Result<Self::Model, ActiveModelError> {
            Err(ActiveModelError::Other("stub".to_string()))
        }

        fn delete(
            &self,
            _executor: &dyn crate::executor::LifeExecutor,
        ) -> Result<(), ActiveModelError> {
            Err(ActiveModelError::Other("stub".to_string()))
        }

        fn from_json(_json: serde_json::Value) -> Result<Self, ActiveModelError> {
            Err(ActiveModelError::Other("stub".to_string()))
        }

        fn to_json(&self) -> Result<serde_json::Value, ActiveModelError> {
            Err(ActiveModelError::Other("stub".to_string()))
        }
    }

    impl ActiveModelBehavior for StubRecord {
        fn validate_fields(&self, _op: ValidateOp) -> Result<(), ActiveModelError> {
            self.order.borrow_mut().push("validate_fields");
            if self.fail_field {
                Err(ActiveModelError::Validation(vec![ValidationError::field(
                    "x",
                    "bad field",
                )]))
            } else {
                Ok(())
            }
        }

        fn validate_model(&self, _op: ValidateOp) -> Result<(), ActiveModelError> {
            self.order.borrow_mut().push("validate_model");
            if self.fail_model {
                Err(ActiveModelError::Validation(vec![ValidationError::model(
                    "bad model",
                )]))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn run_validators_order_field_then_model() {
        let r = StubRecord {
            order: std::cell::RefCell::new(Vec::new()),
            fail_field: false,
            fail_model: false,
        };
        assert!(matches!(
            run_validators(&r, ValidateOp::Insert),
            Ok(())
        ));
        assert_eq!(&*r.order.borrow(), &["validate_fields", "validate_model"]);
    }

    #[test]
    fn run_validators_stops_after_field_error() {
        let r = StubRecord {
            order: std::cell::RefCell::new(Vec::new()),
            fail_field: true,
            fail_model: false,
        };
        assert!(matches!(
            run_validators(&r, ValidateOp::Insert),
            Err(ActiveModelError::Validation(_))
        ));
        assert_eq!(&*r.order.borrow(), &["validate_fields"]);
    }
}
