pub mod adapters;
pub mod router;
pub mod server;
pub mod types;

use server::{create_gateway_router, GatewayState};
use std::net::SocketAddr;
use std::time::Instant;

pub fn start_gateway(state: GatewayState, port: u16) -> tokio::task::JoinHandle<()> {
    // Set start time
    if let Ok(mut lock) = state.stats.start_time.lock() {
        *lock = Some(Instant::now());
    }

    tokio::spawn(async move {
        let app = create_gateway_router(state);
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        tracing::info!("OmniSwitch Gateway starting on http://{}", addr);

        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                if let Err(e) = axum::serve(listener, app).await {
                    tracing::error!("OmniSwitch Gateway server error: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to bind OmniSwitch Gateway on {}: {}", addr, e);
            }
        }
    })
}
