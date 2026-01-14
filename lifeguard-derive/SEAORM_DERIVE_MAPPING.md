# SeaORM to Lifeguard Derive Macro Mapping

## SeaORM Derive Macros (21 total)

1. **DeriveEntity** - Generates Entity, EntityName, Iden, IdenStatic
2. **DeriveEntityModel** - Combines Entity + Model + ActiveModel (the "almighty" macro)
3. **DeriveModelEx** - Complex model with relational fields
4. **DeriveActiveModelEx** - Complex active model with relational fields
5. **DerivePrimaryKey** - PrimaryKey enum with PrimaryKeyTrait
6. **DeriveColumn** - Column enum with ColumnTrait
7. **DeriveModel** - Model struct with ModelTrait and FromQueryResult
8. **DeriveActiveModel** - ActiveModel struct with ActiveModelTrait
9. **DeriveIntoActiveModel** - Conversion from Model to ActiveModel
10. **DeriveActiveModelBehavior** - ActiveModelBehavior trait
11. **DeriveActiveEnum** - ActiveEnum trait for enums
12. **FromQueryResult** - FromQueryResult trait (separate from DeriveModel!)
13. **DeriveRelation** - Relation enum with RelationTrait
14. **DeriveRelatedEntity** - RelatedEntity enum
15. **DeriveMigrationName** - MigrationName trait
16. **FromJsonQueryResult** - FromJsonQueryResult trait
17. **DerivePartialModel** - PartialModelTrait for partial queries
18. **EnumIter** - From strum crate (enum iteration)
19. **DeriveValueType** - ValueType trait for wrapper types
20. **DeriveDisplay** - Display trait for ActiveEnum
21. **DeriveIden** - Iden trait implementation

## Current Lifeguard Derives

- **LifeModel** - Generates Entity, Model, Column, PrimaryKey, LifeModelTrait (combines multiple)
- **LifeRecord** - Generates Record (mutable change-set)
- **FromRow** - FromRow trait (just created, separate from LifeModel)

## Required Lifeguard Derives (to match SeaORM architecture)

### Core Entity/Model Derives (Priority 1)
1. **DeriveEntity** - Entity unit struct, EntityName, Iden, IdenStatic
2. **DeriveModel** - Model struct with ModelTrait (NOT FromRow - that's separate!)
3. **FromRow** - FromRow trait (already created ✅)
4. **DeriveRecord** - Record struct with RecordTrait (renamed from LifeRecord)
5. **DerivePrimaryKey** - PrimaryKey enum
6. **DeriveColumn** - Column enum

### Combined Macro (for convenience)
7. **DeriveLifeModel** - Combines Entity + Model + Record (like DeriveEntityModel)

### Future/Advanced (Priority 2)
8. **DeriveRelation** - Relations (future)
9. **DeriveActiveModelBehavior** - Record behavior customization (future)
10. **DeriveIden** - Iden trait helper (future)

## Key Insight from SeaORM

**Critical:** `FromQueryResult` (our `FromRow`) is a **separate derive** from `DeriveModel`!

- `DeriveModel` generates: Model struct + ModelTrait implementation
- `FromQueryResult` generates: FromQueryResult trait implementation
- They are applied separately: `#[derive(DeriveModel, FromQueryResult)]`

This separation allows the compiler to resolve trait bounds properly during macro expansion.

## Action Plan

1. ✅ Split `FromRow` from `LifeModel` (DONE)
2. Split `DeriveEntity` from `LifeModel` (Entity, EntityName, Iden, IdenStatic)
3. Split `DeriveModel` from `LifeModel` (Model struct + ModelTrait)
4. Split `DeriveColumn` from `LifeModel` (Column enum)
5. Split `DerivePrimaryKey` from `LifeModel` (PrimaryKey enum)
6. Keep `DeriveLifeModel` as convenience macro that combines all of the above
7. Rename `LifeRecord` to `DeriveRecord` for consistency
