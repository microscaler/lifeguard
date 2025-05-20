/// Automatically create and drop a temporary table around a test block.
///
/// Usage:
/// ```ignore
/// with_temp_table!("my_temp", "(id SERIAL)", db, {
///     // use the table inside this block
///     db.execute_unprepared("INSERT INTO my_temp DEFAULT VALUES").await?;
///     ...
/// });
/// ```
#[macro_export]
macro_rules! with_temp_table {
    ($name:expr, $schema:expr, $db:expr, $block:block) => {{
        use sea_orm::ConnectionTrait;

        let drop_sql = format!("DROP TABLE IF EXISTS {}", $name);
        let create_sql = format!("CREATE TEMP TABLE IF NOT EXISTS {} {}", $name, $schema);

        // Always drop, even if block fails
        let result = async {
            $db.execute_unprepared(&drop_sql).await?;
            $db.execute_unprepared(&create_sql).await?;
            let out = (|| async $block)().await;
            $db.execute_unprepared(&drop_sql).await?;
            out
        }
        .await;
        result
    }};
}
