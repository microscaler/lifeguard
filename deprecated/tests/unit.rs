use lifeguard::pool::config::DatabaseConfig;
use lifeguard::DbPoolManager;
use std::env;

// #[test]
// #[cfg_attr(test, ignore)]
// fn test_config_defaults() {
//     env::remove_var("LIFEGUARD__DATABASE__URL");
//     let config = DatabaseConfig::load();
//
//     assert_eq!(
//         config.url,
//         "postgres://postgres:postgres@localhost:5432/postgres"
//     );
//     assert_eq!(config.max_connections, 5);
// }
//
// #[test]
// #[cfg_attr(test, ignore)]
// fn test_config_env_override() {
//     env::set_var(
//         "LIFEGUARD__DATABASE__URL",
//         "postgres://override:pass@host/db",
//     );
//     env::set_var("LIFEGUARD__DATABASE__MAX_CONNECTIONS", "42");
//
//     let config = DatabaseConfig::load();
//
//     assert_eq!(config.url, "postgres://override:pass@host/db");
//     assert_eq!(config.max_connections, 42);
//
//     env::remove_var("LIFEGUARD__DATABASE__URL");
//     env::remove_var("LIFEGUARD__DATABASE__MAX_CONNECTIONS");
// }
//
// #[test]
// #[cfg_attr(test, ignore)]
// fn test_db_pool_manager_creation() {
//     let cfg = DatabaseConfig {
//         url: "postgres://postgres:postgres@localhost:5432/postgres".into(),
//         max_connections: 1,
//     };
//
//     let pool = DbPoolManager::from_config(&cfg);
//     assert!(pool.is_ok());
// }
//
// #[test]
// #[cfg_attr(test, ignore)]
// fn test_execute_returns_error_on_invalid_query() {
//     let cfg = DatabaseConfig::default();
//     let pool = DbPoolManager::from_config(&cfg).unwrap();
//
//     let result: Result<(), _> = pool.execute(|_db| {
//         Box::pin(async { Err::<(), _>(sea_orm::DbErr::Query("synthetic failure".into())) })
//     });
//
//     assert!(result.is_err());
//     let err_str = format!("{:?}", result.unwrap_err());
//     assert!(err_str.contains("synthetic failure"));
// }
//
// #[test]
// #[cfg_attr(test, ignore)]
// fn test_multiple_concurrent_executes() {
//     let cfg = DatabaseConfig::default();
//     let pool = DbPoolManager::from_config(&cfg).unwrap();
//
//     let mut handles = vec![];
//
//     for i in 0..10 {
//         let pool = pool.clone();
//         let handle = std::thread::spawn(move || {
//             let res: Result<String, _> = pool.execute(move |_db| {
//                 Box::pin(async move { Ok::<_, sea_orm::DbErr>(format!("result-{}", i)) })
//             });
//             assert_eq!(res.unwrap(), format!("result-{}", i));
//         });
//
//         handles.push(handle);
//     }
//
//     for h in handles {
//         h.join().unwrap();
//     }
// }
