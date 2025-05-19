/// Convert a Rust value to a SQL-safe string
#[macro_export]
macro_rules! sql_val {
    (null) => {
        "NULL".to_string()
    };
    ($val:expr) => {{
        let s = $val.to_string();
        if s == "true" || s == "false" || s.parse::<f64>().is_ok() {
            s
        } else {
            format!("'{}'", s.replace("'", "''")) // escape single quotes
        }
    }};
}

/// Insert multiple rows of test data into a table.
/// Format: `insert_test_rows!(table, [ { field: val, field2: val2 }, ... ], db)`
#[macro_export]
macro_rules! insert_test_rows {
    ($table:ident, [ $( { $( $key:ident : $val:tt ),* $(,)? } ),+ $(,)? ], $db:expr) => {{
        use sea_orm::ConnectionTrait;

        let columns: Vec<&str> = vec![ $( $( stringify!($key) ),* ),+ ]
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let values = vec![
            $(
                format!("({})", vec![
                    $( $crate::sql_val!($val) ),*
                ].join(", "))
            ),*
        ].join(", ");

        let sql = format!(
            "INSERT INTO {} ({}) VALUES {}",
            stringify!($table),
            columns.join(", "),
            values
        );

        $db.execute_unprepared(&sql).await?;
    }};
}

/// Update rows with field set expressions and WHERE condition
///
/// Usage:
/// ```ignore
/// update_test_rows!(table, { field: val, ... }, "id = 1", db);
/// ```
#[macro_export]
macro_rules! update_test_rows {
    ($table:ident, { $( $key:ident : $val:tt ),* $(,)? }, $where:expr, $db:expr) => {{
        use sea_orm::ConnectionTrait;

        let assignments = vec![
            $( format!("{} = {}", stringify!($key), $crate::sql_val!($val)) ),*
        ].join(", ");

        let sql = format!(
            "UPDATE {} SET {} WHERE {}",
            stringify!($table),
            assignments,
            $where
        );

        $db.execute_unprepared(&sql).await?;
    }};
}
