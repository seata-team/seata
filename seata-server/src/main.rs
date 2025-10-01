use std::{net::SocketAddr};

use anyhow::Result;
use axum::{routing::{get, post}, Router, Json};
use tokio::signal;
use tracing::{info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use seata_server::{settings, storage::{sled_kv::SledKv, redis_kv::RedisKv, sea_kv::SeaKv}, repo::{DynTxnRepo}, saga::{SagaManager, ExecConfig}, barrier::{Barrier, BarrierOps}, domain::{BarrierPhase, GlobalStatus}};
use prometheus::{Encoder, TextEncoder, Registry, Counter, Histogram, HistogramOpts, opts};
use axum::{extract::State, response::IntoResponse};
// static serving can be added later; currently proxying remote admin
use std::sync::Arc;
use tonic::{transport::Server};
use seata_proto::txn::transaction_service_server::{TransactionService, TransactionServiceServer};
use seata_proto::txn::{StartGlobalRequest, StartGlobalResponse, SubmitRequest, SubmitResponse, AbortRequest, AbortResponse, AddBranchRequest, AddBranchResponse, BranchTryRequest, BranchTryResponse, BranchStateRequest, BranchStateResponse, GetRequest, GetResponse, ListRequest, ListResponse};
use etcd_client as etcd;

#[derive(Clone)]
struct AppState { metrics: Registry, saga: Arc<SagaManager<DynTxnRepo>>, barrier: Arc<dyn BarrierOps> }

struct TxnSvcImpl { saga: Arc<SagaManager<DynTxnRepo>>, barrier: Arc<dyn BarrierOps> }

#[tonic::async_trait]
impl TransactionService for TxnSvcImpl {
    async fn start_global(&self, req: tonic::Request<StartGlobalRequest>) -> Result<tonic::Response<StartGlobalResponse>, tonic::Status> {
        let r = req.into_inner();
        let gid = self.saga.start(r.gid, r.mode, r.payload).await.map_err(|e| tonic::Status::internal(e.to_string()))?;
        Ok(tonic::Response::new(StartGlobalResponse { gid }))
    }
    async fn submit(&self, req: tonic::Request<SubmitRequest>) -> Result<tonic::Response<SubmitResponse>, tonic::Status> {
        let gid = req.get_ref().gid.clone();
        self.saga.submit(gid).await.map_err(|e| tonic::Status::internal(e.to_string()))?;
        Ok(tonic::Response::new(SubmitResponse {}))
    }
    async fn abort(&self, req: tonic::Request<AbortRequest>) -> Result<tonic::Response<AbortResponse>, tonic::Status> {
        let gid = req.get_ref().gid.clone();
        self.saga.abort(gid).await.map_err(|e| tonic::Status::internal(e.to_string()))?;
        Ok(tonic::Response::new(AbortResponse {}))
    }
    async fn add_branch(&self, req: tonic::Request<AddBranchRequest>) -> Result<tonic::Response<AddBranchResponse>, tonic::Status> {
        let r = req.into_inner();
        self.saga.add_branch(r.gid, r.branch_id, r.action).await.map_err(|e| tonic::Status::internal(e.to_string()))?;
        Ok(tonic::Response::new(AddBranchResponse {}))
    }
    async fn branch_try(&self, req: tonic::Request<BranchTryRequest>) -> Result<tonic::Response<BranchTryResponse>, tonic::Status> {
        let r = req.into_inner();
        match self.barrier.insert(&r.gid, &r.branch_id, BarrierPhase::Try).await {
            Ok(false) => return Ok(tonic::Response::new(BranchTryResponse {})),
            Err(e) => return Err(tonic::Status::internal(e.to_string())),
            Ok(true) => {}
        }
        self.saga.add_branch(r.gid, r.branch_id, r.action).await.map_err(|e| tonic::Status::internal(e.to_string()))?;
        Ok(tonic::Response::new(BranchTryResponse {}))
    }
    async fn branch_succeed(&self, req: tonic::Request<BranchStateRequest>) -> Result<tonic::Response<BranchStateResponse>, tonic::Status> {
        let r = req.into_inner();
        match self.barrier.insert(&r.gid, &r.branch_id, BarrierPhase::Confirm).await {
            Ok(false) => return Ok(tonic::Response::new(BranchStateResponse {})),
            Err(e) => return Err(tonic::Status::internal(e.to_string())),
            Ok(true) => {}
        }
        self.saga.branch_succeed(r.gid, r.branch_id).await.map_err(|e| tonic::Status::internal(e.to_string()))?;
        Ok(tonic::Response::new(BranchStateResponse {}))
    }
    async fn branch_fail(&self, req: tonic::Request<BranchStateRequest>) -> Result<tonic::Response<BranchStateResponse>, tonic::Status> {
        let r = req.into_inner();
        match self.barrier.insert(&r.gid, &r.branch_id, BarrierPhase::Cancel).await {
            Ok(false) => return Ok(tonic::Response::new(BranchStateResponse {})),
            Err(e) => return Err(tonic::Status::internal(e.to_string())),
            Ok(true) => {}
        }
        self.saga.branch_fail(r.gid, r.branch_id).await.map_err(|e| tonic::Status::internal(e.to_string()))?;
        Ok(tonic::Response::new(BranchStateResponse {}))
    }
    async fn get(&self, req: tonic::Request<GetRequest>) -> Result<tonic::Response<GetResponse>, tonic::Status> {
        let gid = req.get_ref().gid.clone();
        match self.saga.get(&gid).await.map_err(|e| tonic::Status::internal(e.to_string()))? {
            Some(tx) => Ok(tonic::Response::new(GetResponse { txn_json: serde_json::to_vec(&tx).unwrap_or_default() })),
            None => Err(tonic::Status::not_found("not found")),
        }
    }
    async fn list(&self, req: tonic::Request<ListRequest>) -> Result<tonic::Response<ListResponse>, tonic::Status> {
        let limit = req.get_ref().limit as usize;
        let offset = req.get_ref().offset as usize;
        let status_str = req.get_ref().status.clone();
        let status = match status_str.as_str() {
            "SUBMITTED" => Some(GlobalStatus::Submitted),
            "ABORTING" => Some(GlobalStatus::Aborting),
            "ABORTED" => Some(GlobalStatus::Aborted),
            "COMMITTING" => Some(GlobalStatus::Committing),
            "COMMITTED" => Some(GlobalStatus::Committed),
            _ => None,
        };
        let list = self.saga.list(limit, offset, status).await.map_err(|e| tonic::Status::internal(e.to_string()))?;
        let bytes = list.into_iter().map(|t| serde_json::to_vec(&t).unwrap_or_default()).collect();
        Ok(tonic::Response::new(ListResponse { txn_json: bytes }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // logging
    let fmt_layer = fmt::layer().with_target(false).compact();
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,hyper=warn,tower=warn"));
    tracing_subscriber::registry().with(filter).with(fmt_layer).init();

    // config
    let cfg = settings::load().unwrap_or_default();

    // storage select: prefer env SEATA_STORE; otherwise use cfg.store
    let env_store = std::env::var("SEATA_STORE").ok();
    let dyn_repo = if let Some(store) = env_store.as_deref() {
        if store == "redis" {
            let url = std::env::var("SEATA_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
            let r = RedisKv::new(&url)?;
            Arc::new(DynTxnRepo::new(Box::new(r)))
        } else if store == "mysql" {
            let dsn = std::env::var("SEATA_MYSQL_DSN").unwrap_or_else(|_| "mysql://root:password@127.0.0.1:3306/dtm".to_string());
            let kv = SeaKv::mysql(&dsn).await?;
            Arc::new(DynTxnRepo::new(Box::new(kv)))
        } else if store == "postgres" {
            let dsn = std::env::var("SEATA_POSTGRES_DSN").unwrap_or_else(|_| "postgres://postgres:mysecretpassword@127.0.0.1:5432/postgres".to_string());
            let kv = SeaKv::postgres(&dsn).await?;
            Arc::new(DynTxnRepo::new(Box::new(kv)))
        } else {
            let kv = SledKv::open("./data/seata-sled")?;
            Arc::new(DynTxnRepo::new(Box::new(kv)))
        }
    } else {
        match cfg.store {
            settings::Store::Redis { ref host, ref user, ref password, port, .. } => {
                let auth = if !user.is_empty() || !password.is_empty() { format!("{}:{}@", user, password) } else { String::new() };
                let url = format!("redis://{}{}:{}/", auth, host, port);
                Arc::new(DynTxnRepo::new(Box::new(RedisKv::new(&url)?)))
            }
            settings::Store::Mysql { ref host, ref user, ref password, port, ref db, .. } => {
                let dsn = format!("mysql://{}:{}@{}:{}/{}", user, password, host, port, db);
                let kv = SeaKv::mysql(&dsn).await?;
                Arc::new(DynTxnRepo::new(Box::new(kv)))
            }
            settings::Store::Postgres { ref host, ref user, ref password, port, ref db, .. } => {
                let dsn = format!("postgres://{}:{}@{}:{}/{}", user, password, host, port, db);
                let kv = SeaKv::postgres(&dsn).await?;
                Arc::new(DynTxnRepo::new(Box::new(kv)))
            }
            settings::Store::Boltdb => {
                let kv = SledKv::open("./data/seata-sled")?;
                Arc::new(DynTxnRepo::new(Box::new(kv)))
            }
        }
    };
    let exec_cfg = ExecConfig { request_timeout_secs: cfg.request_timeout_secs, retry_interval_secs: cfg.retry_interval_secs, timeout_to_fail_secs: cfg.timeout_to_fail_secs, branch_parallelism: cfg.branch_parallelism };
    // metrics for saga branch execution
    let branch_success_total = Counter::with_opts(opts!("seata_branch_success_total", "Number of successful branch executions"))?;
    let branch_failure_total = Counter::with_opts(opts!("seata_branch_failure_total", "Number of failed branch executions"))?;
    let branch_latency_seconds = Histogram::with_opts(HistogramOpts::new("seata_branch_latency_seconds", "Latency of branch execution"))?;
    let saga_metrics = Arc::new(seata_server::saga::SagaMetrics { branch_success_total: branch_success_total.clone(), branch_failure_total: branch_failure_total.clone(), branch_latency_seconds: branch_latency_seconds.clone() });
    let saga = Arc::new(SagaManager::new(dyn_repo).with_exec_config(exec_cfg).with_metrics(saga_metrics));

    // metrics registry
    let registry = Registry::new();
    registry.register(Box::new(branch_success_total))?;
    registry.register(Box::new(branch_failure_total))?;
    registry.register(Box::new(branch_latency_seconds))?;
    // barrier backend reuse the same KV as selected repo
    let barrier: Arc<dyn BarrierOps> = if let Some(store) = env_store.as_deref() {
        if store == "redis" {
            let url = std::env::var("SEATA_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
            Arc::new(Barrier::new(RedisKv::new(&url)?))
        } else if store == "mysql" {
            let dsn = std::env::var("SEATA_MYSQL_DSN").unwrap_or_else(|_| "mysql://root:password@127.0.0.1:3306/dtm".to_string());
            Arc::new(Barrier::new(SeaKv::mysql(&dsn).await?))
        } else if store == "postgres" {
            let dsn = std::env::var("SEATA_POSTGRES_DSN").unwrap_or_else(|_| "postgres://postgres:mysecretpassword@127.0.0.1:5432/postgres".to_string());
            Arc::new(Barrier::new(SeaKv::postgres(&dsn).await?))
        } else {
            Arc::new(Barrier::new(SledKv::open("./data/seata-sled")?))
        }
    } else {
        match cfg.store {
            settings::Store::Redis { ref host, ref user, ref password, port, .. } => {
                let auth = if !user.is_empty() || !password.is_empty() { format!("{}:{}@", user, password) } else { String::new() };
                let url = format!("redis://{}{}:{}/", auth, host, port);
                Arc::new(Barrier::new(RedisKv::new(&url)?))
            }
            settings::Store::Mysql { ref host, ref user, ref password, port, ref db, .. } => {
                let dsn = format!("mysql://{}:{}@{}:{}/{}", user, password, host, port, db);
                Arc::new(Barrier::new(SeaKv::mysql(&dsn).await?))
            }
            settings::Store::Postgres { ref host, ref user, ref password, port, ref db, .. } => {
                let dsn = format!("postgres://{}:{}@{}:{}/{}", user, password, host, port, db);
                Arc::new(Barrier::new(SeaKv::postgres(&dsn).await?))
            }
            settings::Store::Boltdb => {
                Arc::new(Barrier::new(SledKv::open("./data/seata-sled")?))
            }
        }
    };
    let state = Arc::new(AppState { metrics: registry, saga: saga.clone(), barrier });

    // http router
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/metrics", get(metrics))
        .route("/api/start", post(http_start))
        .route("/api/submit", post(http_submit))
        .route("/api/abort", post(http_abort))
        .route("/api/branch/add", post(http_branch_add))
        .route("/api/branch/try", post(http_branch_try))
        .route("/api/branch/succeed", post(http_branch_succeed))
        .route("/api/branch/fail", post(http_branch_fail))
        .route("/api/tx/{gid}", get(http_get_tx))
        .route("/api/tx", get(http_list_tx))
        .route("/jsonrpc", post(jsonrpc_dispatch))
        .route("/admin/{*path}", get(admin_proxy))
        .route("/", get(admin_proxy))
        .with_state(state.clone());

    // addr from env or default
    let port: u16 = cfg.http_port;
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let http_listener = tokio::net::TcpListener::bind(addr).await?;
    info!("seata-server listening on http://{}", http_listener.local_addr()?);

    // gRPC
    let grpc_addr = SocketAddr::from(([0, 0, 0, 0], cfg.grpc_port));
    let grpc = Server::builder()
        .add_service(TransactionServiceServer::new(TxnSvcImpl { saga: saga.clone(), barrier: state.barrier.clone() }))
        .serve(grpc_addr);
    info!("seata-server gRPC on {}", grpc_addr);

    // optional: service registration to etcd if configured via env
    // SEATA_ETCD_ENDPOINTS: comma-separated endpoints, e.g. "http://127.0.0.1:2379"
    // SEATA_ETCD_NS: namespace prefix (default: /seata)
    // optional etcd registration (env takes precedence, otherwise conf.registry)
    let mut reg_shutdown: Option<tokio::sync::oneshot::Sender<()>> = None;
    let http_url = format!("http://{}", http_listener.local_addr()?.to_string());
    let grpc_url = format!("grpc://{}", grpc_addr.to_string());
    let env_endpoints = std::env::var("SEATA_ETCD_ENDPOINTS").ok();
    let env_ns = std::env::var("SEATA_ETCD_NS").ok();
    let env_instance = std::env::var("SEATA_INSTANCE_ID").ok();
    let (endpoints_opt, ns, instance_id) = if let Some(csv) = env_endpoints {
        let eps: Vec<String> = csv.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        (Some(eps), env_ns.unwrap_or_else(|| "/seata".to_string()), env_instance.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()))
    } else if let Some(reg) = cfg.registry.clone() {
        if reg.driver.to_lowercase() == "etcd" && !reg.endpoints.is_empty() {
            (Some(reg.endpoints), reg.namespace.unwrap_or_else(|| "/seata".to_string()), reg.instance_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()))
        } else { (None, String::new(), String::new()) }
    } else { (None, String::new(), String::new()) };

    if let Some(endpoints) = endpoints_opt {
        let ns_http = format!("{}/endpoints/http/{}", ns, instance_id);
        let ns_grpc = format!("{}/endpoints/grpc/{}", ns, instance_id);
        let (tx, rx) = tokio::sync::oneshot::channel();
        reg_shutdown = Some(tx);
        tokio::spawn(async move {
            if let Err(e) = register_to_etcd(endpoints, ns_http, ns_grpc, http_url, grpc_url, rx).await {
                warn!("etcd registration failed: {}", e);
            }
        });
    }

    let http = axum::serve(http_listener, app.into_make_service());
    let sched_repo = saga.clone(); // using saga to reach repo in future; placeholder
    // spawn scheduler placeholder (no-op scanning)
    tokio::spawn(async move {
        // In a full impl, pass the underlying repo
        // scheduler::run(repo).await;
        let _ = sched_repo; // silence unused for now
    });

    tokio::select! {
        res = http => { res?; }
        _ = shutdown_signal() => {
            info!("shutdown signal received");
            if let Some(tx) = reg_shutdown.take() { let _ = tx.send(()); }
        }
        _ = grpc => {}
    }

    Ok(())
}

async fn register_to_etcd(endpoints: Vec<String>, key_http: String, key_grpc: String, http_url: String, grpc_url: String, mut shutdown_rx: tokio::sync::oneshot::Receiver<()>) -> anyhow::Result<()> {
    let mut client = etcd::Client::connect(endpoints, None).await?;
    // create a TTL lease and keepalive
    let lease_ttl = 15_i64;
    let lease = client.lease_grant(lease_ttl, None).await?.id();
    // put keys with lease
    client.put(key_http.clone(), http_url, Some(etcd::PutOptions::new().with_lease(lease))).await?;
    client.put(key_grpc.clone(), grpc_url, Some(etcd::PutOptions::new().with_lease(lease))).await?;
    // keepalive task
    let (_keeper, mut stream) = client.lease_keep_alive(lease).await?;
    let mut client_for_cleanup = client.clone();
   
    tokio::spawn(async move {
        tokio::select! {
            _ = async {
                loop {
                    match stream.message().await {
                        Ok(Some(_ka)) => {}
                        Ok(None) => break,
                        Err(_e) => break,
                    }
                }
            } => {}
            _ = &mut shutdown_rx => {}
        }
        // best-effort cleanup: delete keys and revoke lease
        let _ = client_for_cleanup.delete(key_http, None).await;
        let _ = client_for_cleanup.delete(key_grpc, None).await;
        let _ = client_for_cleanup.lease_revoke(lease).await;
    });
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).expect("failed to install signal handler");
        term.recv().await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    let metric_families = state.metrics.gather();
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }
    let body = String::from_utf8(buffer).unwrap_or_default();
    (axum::http::StatusCode::OK, body)
}

#[derive(serde::Deserialize)]
struct StartBody { gid: Option<String>, mode: Option<String>, payload: Option<Vec<u8>> }

async fn http_start(State(state): State<Arc<AppState>>, Json(body): Json<StartBody>) -> impl IntoResponse {
    match state.saga.start(body.gid.unwrap_or_default(), body.mode.unwrap_or_default(), body.payload.unwrap_or_default()).await {
        Ok(gid) => (axum::http::StatusCode::OK, Json(serde_json::json!({"gid": gid}))).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct GidBody { gid: String }

async fn http_submit(State(state): State<Arc<AppState>>, Json(body): Json<GidBody>) -> impl IntoResponse {
    match state.saga.submit(body.gid).await {
        Ok(()) => (axum::http::StatusCode::OK, "").into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn http_abort(State(state): State<Arc<AppState>>, Json(body): Json<GidBody>) -> impl IntoResponse {
    match state.saga.abort(body.gid).await {
        Ok(()) => (axum::http::StatusCode::OK, "").into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct BranchBody { gid: String, branch_id: String, action: String }

async fn http_branch_add(State(state): State<Arc<AppState>>, Json(body): Json<BranchBody>) -> impl IntoResponse {
    match state.saga.add_branch(body.gid, body.branch_id, body.action).await {
        Ok(()) => (axum::http::StatusCode::OK, "").into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct BranchStateBody { gid: String, branch_id: String }

async fn http_branch_succeed(State(state): State<Arc<AppState>>, Json(body): Json<BranchStateBody>) -> impl IntoResponse {
    // barrier confirm idempotency
    match state.barrier.insert(&body.gid, &body.branch_id, BarrierPhase::Confirm).await {
        Ok(false) => return (axum::http::StatusCode::OK, "").into_response(),
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        Ok(true) => {}
    }
    match state.saga.branch_succeed(body.gid, body.branch_id).await {
        Ok(()) => (axum::http::StatusCode::OK, "").into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
#[derive(serde::Deserialize)]
struct BranchTryBody { gid: String, branch_id: String, action: String }

async fn http_branch_try(State(state): State<Arc<AppState>>, Json(body): Json<BranchTryBody>) -> impl IntoResponse {
    match state.barrier.insert(&body.gid, &body.branch_id, BarrierPhase::Try).await {
        Ok(false) => return (axum::http::StatusCode::OK, "").into_response(),
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        Ok(true) => {}
    }
    match state.saga.add_branch(body.gid, body.branch_id, body.action).await {
        Ok(()) => (axum::http::StatusCode::OK, "").into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}


async fn http_branch_fail(State(state): State<Arc<AppState>>, Json(body): Json<BranchStateBody>) -> impl IntoResponse {
    // barrier cancel idempotency
    match state.barrier.insert(&body.gid, &body.branch_id, BarrierPhase::Cancel).await {
        Ok(false) => return (axum::http::StatusCode::OK, "").into_response(),
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        Ok(true) => {}
    }
    match state.saga.branch_fail(body.gid, body.branch_id).await {
        Ok(()) => (axum::http::StatusCode::OK, "").into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum JsonRpcParams {
    Start { gid: Option<String>, mode: Option<String>, payload: Option<Vec<u8>> },
    Gid { gid: String },
    None,
}

#[derive(serde::Deserialize)]
struct JsonRpcReq {
    #[serde(default)]
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Option<JsonRpcParams>,
    id: serde_json::Value,
}

#[derive(serde::Serialize)]
struct JsonRpcRes<T> {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")] result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")] error: Option<serde_json::Value>,
    id: serde_json::Value,
}

async fn jsonrpc_dispatch(State(state): State<Arc<AppState>>, Json(req): Json<JsonRpcReq>) -> impl IntoResponse {
    let id = req.id.clone();
    let resp = match req.method.as_str() {
        "start" => {
            let (gid, mode, payload) = match req.params {
                Some(JsonRpcParams::Start { gid, mode, payload }) => (gid.unwrap_or_default(), mode.unwrap_or_default(), payload.unwrap_or_default()),
                _ => (String::new(), String::new(), Vec::new()),
            };
            match state.saga.start(gid, mode, payload).await {
                Ok(gid) => JsonRpcRes { jsonrpc: "2.0", result: Some(serde_json::json!({"gid": gid})), error: None, id },
                Err(e) => JsonRpcRes { jsonrpc: "2.0", result: None, error: Some(serde_json::json!({"code": -32000, "message": e.to_string()})), id },
            }
        }
        "submit" => {
            let gid = match req.params { Some(JsonRpcParams::Gid { gid }) => gid, _ => String::new() };
            match state.saga.submit(gid).await {
                Ok(()) => JsonRpcRes { jsonrpc: "2.0", result: Some(serde_json::json!({})), error: None, id },
                Err(e) => JsonRpcRes { jsonrpc: "2.0", result: None, error: Some(serde_json::json!({"code": -32000, "message": e.to_string()})), id },
            }
        }
        "abort" => {
            let gid = match req.params { Some(JsonRpcParams::Gid { gid }) => gid, _ => String::new() };
            match state.saga.abort(gid).await {
                Ok(()) => JsonRpcRes { jsonrpc: "2.0", result: Some(serde_json::json!({})), error: None, id },
                Err(e) => JsonRpcRes { jsonrpc: "2.0", result: None, error: Some(serde_json::json!({"code": -32000, "message": e.to_string()})), id },
            }
        }
        _ => JsonRpcRes { jsonrpc: "2.0", result: None, error: Some(serde_json::json!({"code": -32601, "message": "Method not found"})), id },
    };
    (axum::http::StatusCode::OK, Json(resp))
}

async fn http_get_tx(State(state): State<Arc<AppState>>, axum::extract::Path(gid): axum::extract::Path<String>) -> impl IntoResponse {
    match state.saga.get(&gid).await {
        Ok(Some(tx)) => (axum::http::StatusCode::OK, Json(tx)).into_response(),
        Ok(None) => (axum::http::StatusCode::NOT_FOUND, "").into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn http_list_tx(State(state): State<Arc<AppState>>, axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    let limit = q.get("limit").and_then(|s| s.parse::<usize>().ok()).unwrap_or(50);
    let offset = q.get("offset").and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    let status = match q.get("status").map(|s| s.as_str()) {
        Some("SUBMITTED") => Some(GlobalStatus::Submitted),
        Some("ABORTING") => Some(GlobalStatus::Aborting),
        Some("ABORTED") => Some(GlobalStatus::Aborted),
        Some("COMMITTING") => Some(GlobalStatus::Committing),
        Some("COMMITTED") => Some(GlobalStatus::Committed),
        _ => None,
    };
    match state.saga.list(limit, offset, status).await {
        Ok(list) => (axum::http::StatusCode::OK, Json(list)).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn admin_proxy(State(_state): State<Arc<AppState>>, uri: axum::http::Uri) -> impl IntoResponse {
    let lang = std::env::var("LANG").unwrap_or_default();
    let host = if lang.starts_with("zh_CN") { "cn-admin.dtm.pub" } else { "admin.dtm.pub" };
    let path = uri.path().to_string();
    // strip optional admin base path if configured (not yet used elsewhere)
    // keep path as-is; remote expects same path
    let url = format!("http://{}{}", host, path);
    let client = reqwest::Client::new();
    match client.get(&url).send().await {
        Ok(resp) => {
            let status = axum::http::StatusCode::from_u16(resp.status().as_u16()).unwrap_or(axum::http::StatusCode::OK);
            let headers = resp.headers().clone();
            let bytes = resp.bytes().await.unwrap_or_default();
            let mut builder = axum::response::Response::builder().status(status);
            // copy minimal headers
            if let Some(ct) = headers.get(axum::http::header::CONTENT_TYPE) { builder = builder.header(axum::http::header::CONTENT_TYPE, ct); }
            builder.body(axum::body::Body::from(bytes)).unwrap()
        }
        Err(e) => (axum::http::StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}
