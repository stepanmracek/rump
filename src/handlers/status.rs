use crate::cache::{get_set, AlbumArtCache};
use crate::mpd::Mpd;
use crate::state::AppState;
use crate::templates as t;
use askama::Template;
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use mpd_client::client::Subsystem;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn get_status(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_status(state, socket))
}

async fn handle_ws_status(state: AppState, mut socket: WebSocket) {
    let mut mpd = state.mpd;
    let mut rx = state.event_tx.subscribe();

    let mut last_background: Option<(Option<(String, String)>, t::Background)> = None;

    if send_mpd_status(
        &mut mpd,
        state.album_art_cache.clone(),
        &mut socket,
        &mut last_background,
    )
    .await
    .is_err()
    {
        return;
    }

    loop {
        let event = rx.recv().await;
        if event.is_err() {
            return;
        }
        let event = event.unwrap();

        match event {
            Subsystem::Player | Subsystem::Queue => {
                if send_mpd_status(
                    &mut mpd,
                    state.album_art_cache.clone(),
                    &mut socket,
                    &mut last_background,
                )
                .await
                .is_err()
                {
                    return;
                }
            }
            _ => {}
        }
    }
}

async fn send_mpd_status(
    mpd: &mut Mpd,
    album_art_cache: Arc<Mutex<AlbumArtCache>>,
    socket: &mut WebSocket,
    last_background: &mut Option<(Option<(String, String)>, t::Background)>,
) -> anyhow::Result<()> {
    let mpd_status = mpd.get_status().await?;

    let album_art_key = mpd_status.artist.clone().zip(mpd_status.album.clone());

    let background = match last_background {
        Some((ref last_key, bg)) if last_key == &album_art_key => {
            // cache hit
            *bg
        }
        _ => {
            let bg = background_color(album_art_key.clone(), album_art_cache, mpd).await;
            *last_background = Some((album_art_key, bg));
            bg
        }
    };

    let template = t::StatusTemplate {
        status: mpd_status,
        background,
    }
    .render()?;
    socket.send(template.into()).await?;
    Ok(())
}

async fn background_color(
    album_art_key: Option<(String, String)>,
    album_art_cache: Arc<Mutex<AlbumArtCache>>,
    mpd: &Mpd,
) -> t::Background {
    match album_art_key {
        Some(key) => {
            tracing::info!("dominant color for {key:?} start");
            let cover_bytes = get_set(key, album_art_cache, mpd).await;

            match cover_bytes {
                Ok(cover_bytes) => {
                    let cover_img = image::load_from_memory(&cover_bytes).unwrap();
                    dominant_color_rs::dominant_color(
                        &cover_img,
                        &dominant_color_rs::Settings::default(),
                    )
                    .map(|f| t::Background::from_floats(&f))
                    .unwrap_or_default()
                }
                _ => t::Background { r: 255, g: 0, b: 0 },
            }
        }
        _ => t::Background::default(),
    }
}
