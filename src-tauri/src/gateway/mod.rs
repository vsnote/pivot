pub mod adapters;
pub mod router;
pub mod server;
pub mod types;

use server::{create_gateway_router, GatewayState};
use std::net::SocketAddr;
use std::time::Instant;

pub fn start_gateway(state: GatewayState, port: u16) {
    // Set start time
    if let Ok(mut lock) = state.stats.start_time.lock() {
        *lock = Some(Instant::now());
    }

    std::thread::Builder::new()
        .name("omniswitch-gateway".to_string())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime for Gateway");

            rt.block_on(async move {
                let app = create_gateway_router(state);
                let addr = SocketAddr::from(([127, 0, 0, 1], port));
                tracing::info!("OmniSwitch Gateway running on http://{}", addr);

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
            });
        })
        .expect("Failed to spawn gateway thread");
}
