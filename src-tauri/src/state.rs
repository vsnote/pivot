use crate::db::Database;
use crate::gateway::router::RoutingEngine;
use crate::gateway::server::GatewayStatsInternal;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub router: Arc<RoutingEngine>,
    pub http_client: reqwest::Client,
    pub stats: Arc<GatewayStatsInternal>,
    pub port: u16,
}
