use sea_orm::{ConnectionTrait, DbErr};

#[allow(dead_code)]
pub async fn create_temp_table(
    db: &impl ConnectionTrait,
    name: &str,
    schema: &str,
) -> Result<(), DbErr> {
    let sql = format!("CREATE TEMP TABLE IF NOT EXISTS {} {}", name, schema);
    db.execute_unprepared(&sql).await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn drop_temp_table(db: &impl ConnectionTrait, name: &str) -> Result<(), DbErr> {
    let sql = format!("DROP TABLE IF EXISTS {}", name);
    db.execute_unprepared(&sql).await?;
    Ok(())
}
