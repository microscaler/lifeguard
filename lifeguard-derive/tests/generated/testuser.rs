use lifeguard::{FromRow, LifeEntityName, LifeModelTrait, ModelTrait};
#[derive(Copy, Clone, Default, Debug)]
pub struct TestUser;
impl LifeEntityName for TestUser {
    fn table_name(&self) -> &'static str {
        "test_users"
    }
}
impl sea_query::Iden for TestUser {
    fn unquoted(&self) -> &str {
        "test_users"
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Column {
    Id,
    Name,
    Email,
    Age,
    Active,
}
impl sea_query::Iden for Column {
    fn unquoted(&self) -> &str {
        match self {
            Column::Id => "id",
            Column::Name => "name",
            Column::Email => "email",
            Column::Age => "age",
            Column::Active => "active",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimaryKey {
    Id,
}
#[derive(Debug, Clone)]
pub struct TestUserModel {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub age: Option<i32>,
    pub active: bool,
}
impl FromRow for TestUserModel {
    fn from_row(row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
        Ok(Self {
            id: row.try_get::<&str, i32>("id")?,
            name: row.try_get::<&str, String>("name")?,
            email: row.try_get::<&str, String>("email")?,
            age: row.try_get::<&str, Option<i32>>("age")?,
            active: row.try_get::<&str, bool>("active")?,
        })
    }
}
impl ModelTrait for TestUserModel {
    type Entity = TestUser;
    fn get(&self, column: <Self::Entity as LifeModelTrait>::Column) -> sea_query::Value {
        match column {
            Column::Id => sea_query::Value::Int(Some(self.id)),
            Column::Name => sea_query::Value::String(Some(self.name.clone())),
            Column::Email => sea_query::Value::String(Some(self.email.clone())),
            Column::Age => self
                .age
                .map(|v| sea_query::Value::Int(Some(v)))
                .unwrap_or(sea_query::Value::Int(None)),
            Column::Active => sea_query::Value::Bool(Some(self.active)),
        }
    }
    fn get_primary_key_value(&self) -> sea_query::Value {
        sea_query::Value::Int(Some(self.id))
    }
}
impl LifeModelTrait for TestUser {
    type Model = TestUserModel;
    type Column = Column;
}
impl TestUser {
    pub const TABLE_NAME: &'static str = "test_users";
}
