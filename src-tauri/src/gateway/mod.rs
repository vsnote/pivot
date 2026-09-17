pub mod adapters;
pub mod router;
pub mod server;
pub mod types;

use server::{create_gateway_router, GatewayState};
use std::net::SocketAddr;
use std::sync::atomic::Ordering;

pub fn start_gateway(state: GatewayState, port: u16) {
    state.stats.is_running.store(false, Ordering::Relaxed);

    std::thread::Builder::new()
        .name("omniswitch-gateway".to_string())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime for Gateway");

            rt.block_on(async move {
                let stats = state.stats.clone();
                let app = create_gateway_router(state);
                let addr = SocketAddr::from(([127, 0, 0, 1], port));
                tracing::info!("Starting OmniSwitch Gateway on http://{}", addr);

                match tokio::net::TcpListener::bind(addr).await {
                    Ok(listener) => {
                        stats.is_running.store(true, Ordering::Relaxed);
                        if let Ok(mut lock) = stats.start_time.lock() {
                            *lock = Some(std::time::Instant::now());
                        }
                        tracing::info!("OmniSwitch Gateway running on http://{}", addr);
                        if let Err(e) = axum::serve(listener, app).await {
                            tracing::error!("OmniSwitch Gateway server error: {}", e);
                        }
                        stats.is_running.store(false, Ordering::Relaxed);
                    }
                    Err(e) => {
                        stats.is_running.store(false, Ordering::Relaxed);
                        tracing::error!(
                            "Failed to bind OmniSwitch Gateway on {}. Is another OmniSwitch instance or service using this port? {}",
                            addr,
                            e
                        );
                    }
                }
            });
        })
        .expect("Failed to spawn gateway thread");
}
