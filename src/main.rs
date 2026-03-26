mod cache;
mod error;
mod handlers;
mod models;
mod mpd;
mod routes;
mod state;
mod templates;

use crate::cache::AlbumArtCache;
use crate::mpd::Mpd;
use crate::routes::create_router;
use crate::state::AppState;
use mpd_client::client::ConnectionEvent;
use mpd_client::client::Subsystem;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let album_art_cache: Arc<Mutex<AlbumArtCache>> = Mutex::new(AlbumArtCache::new()).into();

    let (mpd_client, mut connection_events) =
        Mpd::connect().await.expect("Failed to connect to MPD");
    let mpd = Mpd::new(mpd_client);
    let (event_tx, _) = broadcast::channel(16);

    // MPD reconnection loop
    let mpd_clone = mpd.clone();
    let event_tx_clone = event_tx.clone();
    tokio::spawn(async move {
        loop {
            while let Some(event) = connection_events.next().await {
                if let ConnectionEvent::SubsystemChange(subsystem) = event {
                    let _ = event_tx_clone.send(subsystem);
                }
            }

            tracing::warn!("MPD connection lost, retrying in 5 seconds...");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            match Mpd::connect().await {
                Ok((new_client, new_events)) => {
                    tracing::info!("Reconnected to MPD");
                    mpd_clone.update_client(new_client).await;
                    connection_events = new_events;
                    // Trigger a refresh after reconnection
                    let _ = event_tx_clone.send(Subsystem::Player);
                }
                Err(e) => {
                    tracing::error!("Failed to reconnect to MPD: {}", e);
                }
            }
        }
    });

    let state = AppState {
        mpd,
        album_art_cache,
        event_tx,
    };

    let app = create_router(state).layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
