use lifeguard::{FromRow, LifeEntityName, LifeModelTrait, ModelTrait};
#[derive(Copy, Clone, Default, Debug)]
pub struct UnsignedOptionUser;
impl LifeEntityName for UnsignedOptionUser {
    fn table_name(&self) -> &'static str {
        "unsigned_option_users"
    }
}
impl sea_query::Iden for UnsignedOptionUser {
    fn unquoted(&self) -> &str {
        "unsigned_option_users"
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Column {
    Id,
    Name,
    ValueU8,
    ValueU16,
    ValueU32,
    ValueU64,
}
impl sea_query::Iden for Column {
    fn unquoted(&self) -> &str {
        match self {
            Column::Id => "id",
            Column::Name => "name",
            Column::ValueU8 => "value_u8",
            Column::ValueU16 => "value_u16",
            Column::ValueU32 => "value_u32",
            Column::ValueU64 => "value_u64",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimaryKey {
    Id,
}
#[derive(Debug, Clone)]
pub struct UnsignedOptionUserModel {
    pub id: i32,
    pub name: String,
    pub value_u8: Option<u8>,
    pub value_u16: Option<u16>,
    pub value_u32: Option<u32>,
    pub value_u64: Option<u64>,
}
impl FromRow for UnsignedOptionUserModel {
    fn from_row(row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
        Ok(Self {
            id: row.try_get::<&str, i32>("id")?,
            name: row.try_get::<&str, String>("name")?,
            // Note: may_postgres doesn't support u8/u16/u64 in FromSql, so we use i16/i32/i64 and cast
            // This is a workaround for testing - in practice, you'd use compatible database types
            value_u8: row.try_get::<&str, Option<i16>>("value_u8")?.map(|v| v as u8),
            value_u16: row.try_get::<&str, Option<i32>>("value_u16")?.map(|v| v as u16),
            value_u32: row.try_get::<&str, Option<u32>>("value_u32")?,
            value_u64: row.try_get::<&str, Option<i64>>("value_u64")?.map(|v| v as u64),
        })
    }
}
impl ModelTrait for UnsignedOptionUserModel {
    type Entity = UnsignedOptionUser;
    fn get(&self, column: <Self::Entity as LifeModelTrait>::Column) -> sea_query::Value {
        match column {
            Column::Id => sea_query::Value::Int(Some(self.id)),
            Column::Name => sea_query::Value::String(Some(self.name.clone())),
            Column::ValueU8 => self
                .value_u8
                .map(|v| sea_query::Value::SmallInt(Some(v as i16)))
                .unwrap_or(sea_query::Value::SmallInt(None)),
            Column::ValueU16 => self
                .value_u16
                .map(|v| sea_query::Value::Int(Some(v as i32)))
                .unwrap_or(sea_query::Value::Int(None)),
            Column::ValueU32 => self
                .value_u32
                .map(|v| sea_query::Value::BigInt(Some(v as i64)))
                .unwrap_or(sea_query::Value::BigInt(None)),
            Column::ValueU64 => self
                .value_u64
                .map(|v| sea_query::Value::BigInt(Some(v as i64)))
                .unwrap_or(sea_query::Value::BigInt(None)),
        }
    }
    fn get_primary_key_value(&self) -> sea_query::Value {
        sea_query::Value::Int(Some(self.id))
    }
}
impl LifeModelTrait for UnsignedOptionUser {
    type Model = UnsignedOptionUserModel;
    type Column = Column;
}
impl UnsignedOptionUser {
    pub const TABLE_NAME: &'static str = "unsigned_option_users";
}
