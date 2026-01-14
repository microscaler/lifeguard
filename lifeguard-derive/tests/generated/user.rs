use lifeguard::{FromRow, LifeEntityName, LifeModelTrait, ModelTrait};
#[derive(Copy, Clone, Default, Debug)]
pub struct User;
impl LifeEntityName for User {
    fn table_name(&self) -> &'static str {
        "users"
    }
}
impl sea_query::Iden for User {
    fn unquoted(&self) -> &str {
        "users"
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Column {
    Id,
    Email,
    Name,
}
impl sea_query::Iden for Column {
    fn unquoted(&self) -> &str {
        match self {
            Column::Id => "id",
            Column::Email => "email",
            Column::Name => "name",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimaryKey {
    Id,
}
#[derive(Debug, Clone)]
pub struct UserModel {
    pub id: i32,
    pub email: String,
    pub name: Option<String>,
}
impl FromRow for UserModel {
    fn from_row(row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
        Ok(Self {
            id: row.get::<&str, i32>("id"),
            email: row.get::<&str, String>("email"),
            name: row.try_get::<&str, Option<String>>("name")?,
        })
    }
}
impl ModelTrait for UserModel {
    type Entity = User;
    fn get(&self, column: <Self::Entity as LifeModelTrait>::Column) -> sea_query::Value {
        match column {
            Column::Id => sea_query::Value::Int(Some(self.id)),
            Column::Email => sea_query::Value::String(Some(self.email.clone())),
            Column::Name => self
                .name
                .as_ref()
                .map(|v| sea_query::Value::String(Some(v.clone())))
                .unwrap_or(sea_query::Value::String(None)),
        }
    }
    fn get_primary_key_value(&self) -> sea_query::Value {
        sea_query::Value::Int(Some(self.id))
    }
}
impl LifeModelTrait for User {
    type Model = UserModel;
    type Column = Column;
}
impl User {
    pub const TABLE_NAME: &'static str = "users";
}
