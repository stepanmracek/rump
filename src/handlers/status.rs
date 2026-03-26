use crate::mpd::Mpd;
use crate::state::AppState;
use crate::templates as t;
use askama::Template;
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use mpd_client::client::Subsystem;

pub async fn get_status(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_status(state, socket))
}

async fn send_mpd_status(mpd: &mut Mpd, socket: &mut WebSocket) -> anyhow::Result<()> {
    let mpd_status = mpd.get_status().await?;
    let template = t::StatusTemplate { status: mpd_status }.render()?;
    socket.send(template.into()).await?;
    Ok(())
}

async fn handle_ws_status(state: AppState, mut socket: WebSocket) {
    let mut mpd = state.mpd;
    let mut rx = state.event_tx.subscribe();

    if send_mpd_status(&mut mpd, &mut socket).await.is_err() {
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
                if send_mpd_status(&mut mpd, &mut socket).await.is_err() {
                    return;
                }
            }
            _ => {}
        }
    }
}
