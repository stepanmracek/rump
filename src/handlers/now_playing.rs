use crate::error::AppError;
use crate::handlers::library::render_index;
use crate::mpd::Mpd;
use crate::state::AppState;
use crate::templates as t;
use askama::Template;
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;

pub async fn get_now_playing(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let tabs = t::TabsTemplate {
        now_playing_active: true,
        ..Default::default()
    };

    if headers.contains_key("HX-Request") {
        Ok(t::NowPlayingTemplate { tabs: Some(tabs) }.into_response())
    } else {
        let index = render_index(
            &state.mpd,
            t::Page::NowPlaying(t::NowPlayingTemplate { tabs: None }),
            tabs,
        )
        .await?;
        Ok(index.into_response())
    }
}

pub async fn get_now_playing_content(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_now_playing(state, socket))
}

async fn handle_ws_now_playing(state: AppState, mut socket: WebSocket) {
    let mpd = state.mpd;

    loop {
        if send_now_playing_content(&mpd, &mut socket).await.is_err() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

async fn send_now_playing_content(mpd: &Mpd, socket: &mut WebSocket) -> anyhow::Result<()> {
    let status = mpd.get_status().await?;
    let template = t::NowPlayingContentTemplate { status }.render()?;
    socket.send(template.into()).await?;

    Ok(())
}
