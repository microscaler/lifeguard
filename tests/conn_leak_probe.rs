//! Connection-lifetime probe.
//!
//! Opens a connection, uses it, drops it, and asks the SERVER how many backends
//! remain. If dropping the handle closes the socket, the count returns to
//! baseline each iteration. If it climbs, the connection outlives the handle.
//!
//! Run with:
//!   TEST_DATABASE_URL=postgresql://... cargo test --test conn_leak_probe -- --nocapture

use lifeguard::{query_value, MayPostgresExecutor};

fn backends(observer: &MayPostgresExecutor, db: &str) -> i64 {
    let sql = format!(
        "select count(*) from pg_stat_activity where datname = '{db}'"
    );
    query_value::<i64, _>(observer, &sql, &[]).expect("count backends")
}

#[test]
fn dropping_a_connection_releases_its_backend() {
    let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("PROBE skipped: TEST_DATABASE_URL unset");
        return;
    };
    let db = url.rsplit('/').next().unwrap_or("lifeguard_test").to_string();

    // Held for the whole probe so we always have a way to ask the server.
    let observer =
        MayPostgresExecutor::new(may_postgres::connect(&url).expect("observer connect"));

    let base = backends(&observer, &db);
    println!("PROBE baseline={base}");

    for i in 1..=10 {
        {
            let c = may_postgres::connect(&url).expect("connect");
            let e = MayPostgresExecutor::new(c);
            let one = query_value::<i32, _>(&e, "select 1", &[]).expect("select 1");
            assert_eq!(one, 1);
            // e and c dropped at the end of this block.
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
        let now = backends(&observer, &db);
        println!("PROBE i={i} backends={now} delta={}", now - base);
    }

    let end = backends(&observer, &db);
    println!("PROBE final={end} baseline={base}");
    assert_eq!(
        end, base,
        "{} connection(s) survived being dropped",
        end - base
    );
}

/// Same probe with NO lifeguard wrapper at all - a bare may_postgres::Client,
/// used and dropped. Discriminates between lifeguard-s executor and the
/// underlying driver.
#[test]
fn raw_may_postgres_client_releases_its_backend() {
    let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("RAW skipped: TEST_DATABASE_URL unset");
        return;
    };
    let db = url.rsplit('/').next().unwrap_or("lifeguard_test").to_string();
    let observer =
        MayPostgresExecutor::new(may_postgres::connect(&url).expect("observer connect"));

    let base = backends(&observer, &db);
    println!("RAW baseline={base}");

    for i in 1..=5 {
        {
            let client = may_postgres::connect(&url).expect("connect");
            let rows = client.query("select 1", &[]).expect("query");
            assert_eq!(rows.len(), 1);
            drop(client);
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
        println!("RAW i={i} backends={}", backends(&observer, &db));
    }
    println!("RAW final={}", backends(&observer, &db));
}
