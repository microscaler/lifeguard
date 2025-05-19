/// Create a temp table, seed it with test data, and run a scoped block.
/// Format:
/// ```ignore
/// seed_test!(table, "(id INT, name TEXT)", [
///     { id: 1, name: "Alice" },
///     { id: 2, name: "Bob" }
/// ], db, {
///     // test logic here
/// });
/// ```
#[macro_export]
macro_rules! seed_test {
    ($table:ident, $schema:expr, [ $( { $( $key:ident : $val:tt ),* $(,)? } ),+ $(,)? ], $db:expr, $block:block) => {{
        use sea_orm::ConnectionTrait;

        let create_sql = format!("CREATE TEMP TABLE IF NOT EXISTS {} {}", stringify!($table), $schema);
        let drop_sql = format!("DROP TABLE IF EXISTS {}", stringify!($table));

        let result = async {
            $db.execute_unprepared(&create_sql).await?;
            $crate::insert_test_rows!($table, [ $( { $( $key : $val ),* } ),+ ], $db);
            let out = (|| async $block)().await;
            $db.execute_unprepared(&drop_sql).await?;
            out
        }
        .await;

        result
    }};
}
