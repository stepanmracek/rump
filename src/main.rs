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
use mpd::Mpd;
use mpd_client::client::{ConnectionEvent, Subsystem};
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
        .route("/artist/:artist/album/:album/songs", get(get_songs))
        .route("/status", get(get_status))
        .route("/control/play", get(control_play))
        .route("/control/play/:song_id", get(control_play_song))
        .route("/control/playlist/clear", get(clear_playlist))
        .route("/control/unpause", get(control_unpause))
        .route("/control/pause", get(control_pause))
        .route("/control/prev", get(control_prev))
        .route("/control/next", get(control_next))
        .route("/playlist", get(get_playlist))
        .route("/playlist/songs", get(get_playlist_songs))
        .nest_service("/assets", ServeDir::new("assets"));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_library() -> Result<t::HtmlTemplate<t::LibraryTemplate>, String> {
    let template = t::LibraryTemplate;
    Ok(t::HtmlTemplate(template))
}

async fn get_artists(
    Query(artists_search_query): Query<ArtistsSearchQuery>,
) -> Result<impl IntoResponse, String> {
    let artists = Mpd::connect()
        .await?
        .get_artists(artists_search_query.q)
        .await?;
    let template = t::ArtistsTemplate { artists };
    Ok(t::HtmlTemplate(template))
}

async fn get_albums(Path(artist): Path<String>) -> Result<impl IntoResponse, String> {
    let albums = Mpd::connect().await?.get_albums(&artist).await?;
    let template = t::AlbumsTemplate { artist, albums };
    Ok(t::HtmlTemplate(template))
}

async fn get_songs(
    Path((artist, album)): Path<(String, String)>,
) -> Result<impl IntoResponse, String> {
    let songs = Mpd::connect().await?.get_songs(&artist, &album).await?;
    let template = t::AlbumSongsTemplate {
        artist,
        album,
        songs,
    };
    Ok(t::HtmlTemplate(template))
}

async fn get_index() -> impl IntoResponse {
    let template = t::IndexTemplate;
    t::HtmlTemplate(template)
}

async fn get_status(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_ws_status)
}

async fn send_mpd_status(mpd: &mut Mpd, socket: &mut WebSocket) -> Result<(), String> {
    let mpd_status = mpd.get_status().await;
    match mpd_status {
        Ok(mpd_status) => {
            let template = t::StatusTemplate { status: mpd_status }
                .render()
                .map_err(|e| e.to_string());
            match template {
                Ok(template) => socket
                    .send(template.into())
                    .await
                    .map_err(|e| e.to_string())?,
                Err(e) => return Err(e),
            };
        }
        Err(e) => return Err(e),
    };

    Ok(())
}

async fn handle_ws_status(mut socket: WebSocket) {
    let mpd = Mpd::connect().await;
    if mpd.is_err() {
        return;
    }
    let mut mpd = mpd.unwrap();
    if send_mpd_status(&mut mpd, &mut socket).await.is_err() {
        return;
    }
    loop {
        let event = mpd.connection_events.next().await;
        if event.is_none() {
            return;
        }
        let event = event.unwrap();

        match event {
            ConnectionEvent::SubsystemChange(Subsystem::Player)
            | ConnectionEvent::SubsystemChange(Subsystem::Queue) => {
                if send_mpd_status(&mut mpd, &mut socket).await.is_err() {
                    return;
                }
            }
            ConnectionEvent::ConnectionClosed(_) => return,
            ConnectionEvent::SubsystemChange(_) => {}
        }
    }
}

async fn control_play() -> Result<(), String> {
    Mpd::connect().await?.play().await
}

async fn control_play_song(Path(song_id): Path<u64>) -> Result<(), String> {
    Mpd::connect().await?.play_song(song_id).await
}

async fn control_pause() -> Result<(), String> {
    Mpd::connect().await?.pause(true).await
}

async fn control_unpause() -> Result<(), String> {
    Mpd::connect().await?.pause(false).await
}

async fn control_prev() -> Result<(), String> {
    Mpd::connect().await?.prev().await
}

async fn control_next() -> Result<(), String> {
    Mpd::connect().await?.next().await
}

async fn clear_playlist() -> Result<(), String> {
    Mpd::connect().await?.clear_playlist().await
}

async fn get_playlist_songs(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_ws_playlist)
}

async fn send_playlist(mpd: &Mpd, socket: &mut WebSocket) -> Result<(), String> {
    let playlist = mpd.get_playlist().await;
    match playlist {
        Ok(playlist) => {
            let template = t::PlaylistSongsTemplate { songs: playlist }
                .render()
                .map_err(|e| e.to_string());
            match template {
                Ok(template) => socket
                    .send(template.into())
                    .await
                    .map_err(|e| e.to_string())?,
                Err(e) => return Err(e),
            };
        }
        Err(e) => return Err(e),
    };

    Ok(())
}

async fn handle_ws_playlist(mut socket: WebSocket) {
    let mpd = Mpd::connect().await;
    if mpd.is_err() {
        return;
    }
    let mut mpd = mpd.unwrap();

    if send_playlist(&mpd, &mut socket).await.is_err() {
        return;
    }
    loop {
        let event = mpd.connection_events.next().await;
        if event.is_none() {
            return;
        }
        let event = event.unwrap();

        match event {
            ConnectionEvent::SubsystemChange(Subsystem::Player)
            | ConnectionEvent::SubsystemChange(Subsystem::Queue) => {
                if send_playlist(&mpd, &mut socket).await.is_err() {
                    return;
                }
            }
            ConnectionEvent::ConnectionClosed(_) => return,
            ConnectionEvent::SubsystemChange(_) => {}
        }
    }
}

async fn get_playlist() -> impl IntoResponse {
    let template = t::PlaylistTemplate;
    t::HtmlTemplate(template)
}
