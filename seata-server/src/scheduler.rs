use crate::repo::TxnRepo;
use std::sync::Arc;
use tokio::time::{self, Duration};
use tracing::info;

pub async fn run<R: TxnRepo + 'static>(_repo: Arc<R>) {
    let mut ticker = time::interval(Duration::from_secs(10));
    loop {
        ticker.tick().await;
        // TODO: scan unfinished transactions and retry/timeout
        info!("scheduler tick");
    }
}

