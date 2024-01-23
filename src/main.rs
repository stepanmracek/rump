mod mpd;
mod templates;

use askama::Template;
use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    extract::{Path, Query},
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::Deserialize;
use templates as t;
use tower_http::services::ServeDir;

#[derive(Deserialize)]
struct ArtistsSearchQuery {
    q: Option<String>,
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(get_index))
        .route("/library", get(get_library))
        .route("/artists", get(get_artists))
        .route("/albums/:artist", get(get_albums))
        .route("/status", get(get_status))
        .route("/control/play", get(control_play))
        .route("/control/unpause", get(control_unpause))
        .route("/control/pause", get(control_pause))
        .route("/control/prev", get(control_prev))
        .route("/control/next", get(control_next))
        .nest_service("/assets", ServeDir::new("assets"));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_library() -> Result<t::HtmlTemplate<t::LibraryTemplate>, String> {
    let artists = mpd::get_artists(None).await?;
    let template = t::LibraryTemplate { artists };
    Ok(t::HtmlTemplate(template))
}

async fn get_artists(
    Query(artists_search_query): Query<ArtistsSearchQuery>,
) -> Result<t::HtmlTemplate<t::ArtistsTemplate>, String> {
    let artists = mpd::get_artists(artists_search_query.q).await?;
    let template = t::ArtistsTemplate { artists };
    Ok(t::HtmlTemplate(template))
}

async fn get_albums(
    Path(artist): Path<String>,
) -> Result<t::HtmlTemplate<t::AlbumsTemplate>, String> {
    let albums = mpd::get_albums(&artist).await?;
    let template = t::AlbumsTemplate { albums };
    Ok(t::HtmlTemplate(template))
}

async fn get_index() -> impl IntoResponse {
    let template = t::IndexTemplate {};
    t::HtmlTemplate(template)
}

async fn get_status(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_ws_status)
}

async fn send_mpd_status_to_web_socket(
    mpd_client: &mpd_client::Client,
    socket: &mut WebSocket,
) -> Result<(), String> {
    let mpd_status = mpd::get_status(&mpd_client).await;
    match mpd_status {
        Ok(mpd_status) => {
            println!("{mpd_status:?}");
            let template = t::StatusTemplate { status: mpd_status }.render().map_err(|e| e.to_string());
            match template {
                Ok(template) => socket.send(template.into()).await.map_err(|e| e.to_string())?,
                Err(e) => println!("Error when rendering template: {e}")
            };
        }
        Err(_) => todo!(),
    };

    Ok(())
}

async fn handle_ws_status(mut socket: WebSocket) {
    let connection = mpd::connect().await;
    if connection.is_err() {
        return;
    }
    let (mpd_client, mut connection_events) = connection.unwrap();
    if send_mpd_status_to_web_socket(&mpd_client, &mut socket)
        .await
        .is_err()
    {
        return;
    }
    loop {
        let event = connection_events.next().await;
        if event.is_none() {
            return;
        }
        let event = event.unwrap();

        match event {
            mpd_client::client::ConnectionEvent::SubsystemChange(
                mpd_client::client::Subsystem::Player,
            )
            | mpd_client::client::ConnectionEvent::SubsystemChange(
                mpd_client::client::Subsystem::Queue,
            ) => {
                if send_mpd_status_to_web_socket(&mpd_client, &mut socket)
                    .await
                    .is_err()
                {
                    return;
                }
            }
            mpd_client::client::ConnectionEvent::ConnectionClosed(_) => {
                return;
            }
            mpd_client::client::ConnectionEvent::SubsystemChange(_) => {}
        }
    }
}

async fn control_play() {
    let _ = mpd::play().await;
}

async fn control_pause() {
    let _ = mpd::pause(true).await;
}

async fn control_unpause() {
    let _ = mpd::pause(false).await;
}

async fn control_prev() {
    let _ = mpd::prev().await;
}

async fn control_next() {
    let _ = mpd::next().await;
}
