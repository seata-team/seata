use crate::settings;
use serde_yaml;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = settings::Settings::default();
        
        assert_eq!(settings.http_port, 36789);
        assert_eq!(settings.grpc_port, 36790);
        assert_eq!(settings.jsonrpc_port, 36791);
        assert_eq!(settings.log_level, "info");
        assert_eq!(settings.admin_base_path, "");
        assert_eq!(settings.request_timeout_secs, 3);
        assert_eq!(settings.retry_interval_secs, 10);
        assert_eq!(settings.timeout_to_fail_secs, 35);
        assert_eq!(settings.branch_parallelism, 8);
    }

    #[test]
    fn test_settings_serialization() {
        let settings = settings::Settings {
            http_port: 8080,
            grpc_port: 8081,
            jsonrpc_port: 8082,
            log_level: "debug".to_string(),
            admin_base_path: "/admin".to_string(),
            request_timeout_secs: 5,
            retry_interval_secs: 15,
            timeout_to_fail_secs: 60,
            branch_parallelism: 16,
            store: settings::Store::Boltdb,
        };
        
        let yaml = serde_yaml::to_string(&settings).unwrap();
        let deserialized: settings::Settings = serde_yaml::from_str(&yaml).unwrap();
        
        assert_eq!(settings.http_port, deserialized.http_port);
        assert_eq!(settings.grpc_port, deserialized.grpc_port);
        assert_eq!(settings.jsonrpc_port, deserialized.jsonrpc_port);
        assert_eq!(settings.log_level, deserialized.log_level);
        assert_eq!(settings.admin_base_path, deserialized.admin_base_path);
        assert_eq!(settings.request_timeout_secs, deserialized.request_timeout_secs);
        assert_eq!(settings.retry_interval_secs, deserialized.retry_interval_secs);
        assert_eq!(settings.timeout_to_fail_secs, deserialized.timeout_to_fail_secs);
        assert_eq!(settings.branch_parallelism, deserialized.branch_parallelism);
        assert_eq!(settings.store, deserialized.store);
    }

    #[test]
    fn test_redis_store_serialization() {
        let store = settings::Store::Redis {
            host: "localhost".to_string(),
            user: "user".to_string(),
            password: "password".to_string(),
            port: 6379,
            data_expire: Some(3600),
            finished_data_expire: Some(7200),
            redis_prefix: Some("seata:".to_string()),
        };
        
        let yaml = serde_yaml::to_string(&store).unwrap();
        let deserialized: settings::Store = serde_yaml::from_str(&yaml).unwrap();
        
        match (store, deserialized) {
            (settings::Store::Redis { host: h1, user: u1, password: p1, port: po1, data_expire: de1, finished_data_expire: fde1, redis_prefix: rp1 },
             settings::Store::Redis { host: h2, user: u2, password: p2, port: po2, data_expire: de2, finished_data_expire: fde2, redis_prefix: rp2 }) => {
                assert_eq!(h1, h2);
                assert_eq!(u1, u2);
                assert_eq!(p1, p2);
                assert_eq!(po1, po2);
                assert_eq!(de1, de2);
                assert_eq!(fde1, fde2);
                assert_eq!(rp1, rp2);
            }
            _ => panic!("Store types don't match"),
        }
    }

    #[test]
    fn test_mysql_store_serialization() {
        let store = settings::Store::Mysql {
            host: "localhost".to_string(),
            user: "root".to_string(),
            password: "password".to_string(),
            port: 3306,
            db: "seata".to_string(),
            max_open_conns: Some(100),
            max_idle_conns: Some(20),
            conn_max_life_time_minutes: Some(60),
        };
        
        let yaml = serde_yaml::to_string(&store).unwrap();
        let deserialized: settings::Store = serde_yaml::from_str(&yaml).unwrap();
        
        match (store, deserialized) {
            (settings::Store::Mysql { host: h1, user: u1, password: p1, port: po1, db: d1, max_open_conns: moc1, max_idle_conns: mic1, conn_max_life_time_minutes: cmlt1 },
             settings::Store::Mysql { host: h2, user: u2, password: p2, port: po2, db: d2, max_open_conns: moc2, max_idle_conns: mic2, conn_max_life_time_minutes: cmlt2 }) => {
                assert_eq!(h1, h2);
                assert_eq!(u1, u2);
                assert_eq!(p1, p2);
                assert_eq!(po1, po2);
                assert_eq!(d1, d2);
                assert_eq!(moc1, moc2);
                assert_eq!(mic1, mic2);
                assert_eq!(cmlt1, cmlt2);
            }
            _ => panic!("Store types don't match"),
        }
    }

    #[test]
    fn test_postgres_store_serialization() {
        let store = settings::Store::Postgres {
            host: "localhost".to_string(),
            user: "postgres".to_string(),
            password: "password".to_string(),
            port: 5432,
            db: "seata".to_string(),
            schema: Some("public".to_string()),
            max_open_conns: Some(100),
            max_idle_conns: Some(20),
            conn_max_life_time_minutes: Some(60),
        };
        
        let yaml = serde_yaml::to_string(&store).unwrap();
        let deserialized: settings::Store = serde_yaml::from_str(&yaml).unwrap();
        
        match (store, deserialized) {
            (settings::Store::Postgres { host: h1, user: u1, password: p1, port: po1, db: d1, schema: s1, max_open_conns: moc1, max_idle_conns: mic1, conn_max_life_time_minutes: cmlt1 },
             settings::Store::Postgres { host: h2, user: u2, password: p2, port: po2, db: d2, schema: s2, max_open_conns: moc2, max_idle_conns: mic2, conn_max_life_time_minutes: cmlt2 }) => {
                assert_eq!(h1, h2);
                assert_eq!(u1, u2);
                assert_eq!(p1, p2);
                assert_eq!(po1, po2);
                assert_eq!(d1, d2);
                assert_eq!(s1, s2);
                assert_eq!(moc1, moc2);
                assert_eq!(mic1, mic2);
                assert_eq!(cmlt1, cmlt2);
            }
            _ => panic!("Store types don't match"),
        }
    }

    #[test]
    fn test_boltdb_store_serialization() {
        let store = settings::Store::Boltdb;
        
        let yaml = serde_yaml::to_string(&store).unwrap();
        let deserialized: settings::Store = serde_yaml::from_str(&yaml).unwrap();
        
        assert_eq!(store, deserialized);
    }

    #[test]
    fn test_settings_from_yaml() {
        let yaml = r#"
http_port: 8080
grpc_port: 8081
jsonrpc_port: 8082
log_level: debug
admin_base_path: /admin
request_timeout_secs: 5
retry_interval_secs: 15
timeout_to_fail_secs: 60
branch_parallelism: 16
store:
  driver: redis
  host: localhost
  user: user
  password: password
  port: 6379
  data_expire: 3600
  finished_data_expire: 7200
  redis_prefix: "seata:"
"#;
        
        let settings: settings::Settings = serde_yaml::from_str(yaml).unwrap();
        
        assert_eq!(settings.http_port, 8080);
        assert_eq!(settings.grpc_port, 8081);
        assert_eq!(settings.jsonrpc_port, 8082);
        assert_eq!(settings.log_level, "debug");
        assert_eq!(settings.admin_base_path, "/admin");
        assert_eq!(settings.request_timeout_secs, 5);
        assert_eq!(settings.retry_interval_secs, 15);
        assert_eq!(settings.timeout_to_fail_secs, 60);
        assert_eq!(settings.branch_parallelism, 16);
        
        match settings.store {
            settings::Store::Redis { host, user, password, port, data_expire, finished_data_expire, redis_prefix } => {
                assert_eq!(host, "localhost");
                assert_eq!(user, "user");
                assert_eq!(password, "password");
                assert_eq!(port, 6379);
                assert_eq!(data_expire, Some(3600));
                assert_eq!(finished_data_expire, Some(7200));
                assert_eq!(redis_prefix, Some("seata:".to_string()));
            }
            _ => panic!("Expected Redis store"),
        }
    }

    #[test]
    fn test_settings_from_mysql_yaml() {
        let yaml = r#"
store:
  driver: mysql
  host: localhost
  user: root
  password: password
  port: 3306
  db: seata
  max_open_conns: 100
  max_idle_conns: 20
  conn_max_life_time_minutes: 60
"#;
        
        let settings: settings::Settings = serde_yaml::from_str(yaml).unwrap();
        
        match settings.store {
            settings::Store::Mysql { host, user, password, port, db, max_open_conns, max_idle_conns, conn_max_life_time_minutes } => {
                assert_eq!(host, "localhost");
                assert_eq!(user, "root");
                assert_eq!(password, "password");
                assert_eq!(port, 3306);
                assert_eq!(db, "seata");
                assert_eq!(max_open_conns, Some(100));
                assert_eq!(max_idle_conns, Some(20));
                assert_eq!(conn_max_life_time_minutes, Some(60));
            }
            _ => panic!("Expected MySQL store"),
        }
    }

    #[test]
    fn test_settings_from_postgres_yaml() {
        let yaml = r#"
store:
  driver: postgres
  host: localhost
  user: postgres
  password: password
  port: 5432
  db: seata
  schema: public
  max_open_conns: 100
  max_idle_conns: 20
  conn_max_life_time_minutes: 60
"#;
        
        let settings: settings::Settings = serde_yaml::from_str(yaml).unwrap();
        
        match settings.store {
            settings::Store::Postgres { host, user, password, port, db, schema, max_open_conns, max_idle_conns, conn_max_life_time_minutes } => {
                assert_eq!(host, "localhost");
                assert_eq!(user, "postgres");
                assert_eq!(password, "password");
                assert_eq!(port, 5432);
                assert_eq!(db, "seata");
                assert_eq!(schema, Some("public".to_string()));
                assert_eq!(max_open_conns, Some(100));
                assert_eq!(max_idle_conns, Some(20));
                assert_eq!(conn_max_life_time_minutes, Some(60));
            }
            _ => panic!("Expected PostgreSQL store"),
        }
    }

    #[test]
    fn test_settings_from_boltdb_yaml() {
        let yaml = r#"
store:
  driver: boltdb
"#;
        
        let settings: settings::Settings = serde_yaml::from_str(yaml).unwrap();
        
        match settings.store {
            settings::Store::Boltdb => {
                // Expected
            }
            _ => panic!("Expected Boltdb store"),
        }
    }

    #[test]
    fn test_settings_validation() {
        // Test valid settings
        let settings = settings::Settings {
            http_port: 8080,
            grpc_port: 8081,
            jsonrpc_port: 8082,
            log_level: "info".to_string(),
            admin_base_path: "".to_string(),
            request_timeout_secs: 5,
            retry_interval_secs: 10,
            timeout_to_fail_secs: 30,
            branch_parallelism: 16,
            store: settings::Store::Boltdb,
        };
        
        // Should not panic
        let _yaml = serde_yaml::to_string(&settings).unwrap();
    }

    #[test]
    fn test_settings_edge_cases() {
        // Test with minimal values
        let settings = settings::Settings {
            http_port: 1,
            grpc_port: 2,
            jsonrpc_port: 3,
            log_level: "".to_string(),
            admin_base_path: "".to_string(),
            request_timeout_secs: 1,
            retry_interval_secs: 1,
            timeout_to_fail_secs: 1,
            branch_parallelism: 1,
            store: settings::Store::Boltdb,
        };
        
        let yaml = serde_yaml::to_string(&settings).unwrap();
        let deserialized: settings::Settings = serde_yaml::from_str(&yaml).unwrap();
        
        assert_eq!(settings.http_port, deserialized.http_port);
        assert_eq!(settings.grpc_port, deserialized.grpc_port);
        assert_eq!(settings.jsonrpc_port, deserialized.jsonrpc_port);
        assert_eq!(settings.log_level, deserialized.log_level);
        assert_eq!(settings.admin_base_path, deserialized.admin_base_path);
        assert_eq!(settings.request_timeout_secs, deserialized.request_timeout_secs);
        assert_eq!(settings.retry_interval_secs, deserialized.retry_interval_secs);
        assert_eq!(settings.timeout_to_fail_secs, deserialized.timeout_to_fail_secs);
        assert_eq!(settings.branch_parallelism, deserialized.branch_parallelism);
        assert_eq!(settings.store, deserialized.store);
    }

    #[test]
    fn test_settings_with_empty_strings() {
        let yaml = r#"
log_level: ""
admin_base_path: ""
store:
  driver: redis
  host: ""
  user: ""
  password: ""
  port: 6379
  data_expire: 0
  finished_data_expire: 0
  redis_prefix: ""
"#;
        
        let settings: settings::Settings = serde_yaml::from_str(yaml).unwrap();
        
        assert_eq!(settings.log_level, "");
        assert_eq!(settings.admin_base_path, "");
        
        match settings.store {
            settings::Store::Redis { host, user, password, redis_prefix, .. } => {
                assert_eq!(host, "");
                assert_eq!(user, "");
                assert_eq!(password, "");
                assert_eq!(redis_prefix, Some("".to_string()));
            }
            _ => panic!("Expected Redis store"),
        }
    }

    #[test]
    fn test_settings_roundtrip() {
        let original = settings::Settings {
            http_port: 9999,
            grpc_port: 9998,
            jsonrpc_port: 9997,
            log_level: "trace".to_string(),
            admin_base_path: "/custom".to_string(),
            request_timeout_secs: 10,
            retry_interval_secs: 20,
            timeout_to_fail_secs: 120,
            branch_parallelism: 32,
            store: settings::Store::Redis {
                host: "redis.example.com".to_string(),
                user: "admin".to_string(),
                password: "secret".to_string(),
                port: 6380,
                data_expire: Some(7200),
                finished_data_expire: Some(14400),
                redis_prefix: Some("seata:prod:".to_string()),
            },
        };
        
        let yaml = serde_yaml::to_string(&original).unwrap();
        let deserialized: settings::Settings = serde_yaml::from_str(&yaml).unwrap();
        
        assert_eq!(original.http_port, deserialized.http_port);
        assert_eq!(original.grpc_port, deserialized.grpc_port);
        assert_eq!(original.jsonrpc_port, deserialized.jsonrpc_port);
        assert_eq!(original.log_level, deserialized.log_level);
        assert_eq!(original.admin_base_path, deserialized.admin_base_path);
        assert_eq!(original.request_timeout_secs, deserialized.request_timeout_secs);
        assert_eq!(original.retry_interval_secs, deserialized.retry_interval_secs);
        assert_eq!(original.timeout_to_fail_secs, deserialized.timeout_to_fail_secs);
        assert_eq!(original.branch_parallelism, deserialized.branch_parallelism);
        assert_eq!(original.store, deserialized.store);
    }
}
