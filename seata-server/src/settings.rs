use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub http_port: u16,
    pub grpc_port: u16,
    pub jsonrpc_port: u16,
    pub log_level: String,
    pub admin_base_path: String,
    pub request_timeout_secs: u64,
    pub retry_interval_secs: u64,
    pub timeout_to_fail_secs: u64,
    pub branch_parallelism: usize,

    // DB pool configuration (optional)
    pub max_open_conns: Option<u32>,
    pub max_idle_conns: Option<u32>,
    pub conn_max_life_time_minutes: Option<u64>,

    pub store: Store,
    pub registry: Option<Registry>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            http_port: 36789,
            grpc_port: 36790,
            jsonrpc_port: 36791,
            log_level: "info".into(),
            admin_base_path: "".into(),
            request_timeout_secs: 3,
            retry_interval_secs: 10,
            timeout_to_fail_secs: 35,
            branch_parallelism: 8,
            max_open_conns: None,
            max_idle_conns: None,
            conn_max_life_time_minutes: None,
            store: Store::default(),
            registry: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "driver", rename_all = "lowercase")]
pub enum Store {
    Boltdb,
    Redis {
        host: String,
        user: String,
        password: String,
        port: u16,
        data_expire: Option<u64>,
        finished_data_expire: Option<u64>,
        redis_prefix: Option<String>,
    },
    Mysql {
        host: String,
        user: String,
        password: String,
        port: u16,
        db: String,
        max_open_conns: Option<u32>,
        max_idle_conns: Option<u32>,
        conn_max_life_time_minutes: Option<u64>,
    },
    Postgres {
        host: String,
        user: String,
        password: String,
        port: u16,
        db: String,
        schema: Option<String>,
        max_open_conns: Option<u32>,
        max_idle_conns: Option<u32>,
        conn_max_life_time_minutes: Option<u64>,
    },
}

impl Default for Store {
    fn default() -> Self {
        Store::Boltdb
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub driver: String,                 // "etcd" or empty
    pub endpoints: Vec<String>,         // e.g. ["http://127.0.0.1:2379"]
    pub namespace: Option<String>,      // default "/seata"
    pub instance_id: Option<String>,    // default random UUID
}

pub fn load() -> anyhow::Result<Settings> {
    use config::{Config, Environment, File};

    let builder = Config::builder()
        .add_source(File::with_name("conf").required(false))
        .add_source(File::with_name("conf.yml").required(false))
        .add_source(File::with_name("conf.yaml").required(false))
        .add_source(Environment::with_prefix("SEATA").try_parsing(true).separator("_"));

    let cfg = builder.build()?;
    let mut s: Settings = cfg.try_deserialize()?;

    if let Ok(port) = std::env::var("SEATA_HTTP_PORT").and_then(|v| v.parse().map_err(|_| std::env::VarError::NotPresent)) {
        s.http_port = port;
    }

    Ok(s)
}

