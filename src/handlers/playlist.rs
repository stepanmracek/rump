use crate::error::AppError;
use crate::handlers::library::render_index;
use crate::models::{ArtistAlbumQuery, SongIdQuery, UrlQuery};
use crate::mpd::Mpd;
use crate::state::AppState;
use crate::templates as t;
use askama::Template;
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use mpd_client::client::Subsystem;

pub async fn get_playlist_songs(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_playlist(state, socket))
}

async fn send_playlist(mpd: &Mpd, socket: &mut WebSocket) -> anyhow::Result<()> {
    let status = mpd.get_status().await?;
    let playlist = mpd.get_playlist().await?;
    let template = t::PlaylistSongsTemplate {
        songs: playlist,
        status,
    }
    .render()?;
    socket.send(template.into()).await?;

    Ok(())
}

async fn handle_ws_playlist(state: AppState, mut socket: WebSocket) {
    let mpd = state.mpd;
    let mut rx = state.event_tx.subscribe();

    if send_playlist(&mpd, &mut socket).await.is_err() {
        return;
    }
    loop {
        let event = rx.recv().await;
        if event.is_err() {
            return;
        }
        let event = event.unwrap();

        match event {
            Subsystem::Player | Subsystem::Queue | Subsystem::Options
                if send_playlist(&mpd, &mut socket).await.is_err() =>
            {
                return;
            }
            _ => {}
        }
    }
}

pub async fn get_playlist(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let tabs = t::TabsTemplate {
        playlist_active: true,
        ..Default::default()
    };

    if headers.contains_key("HX-Request") {
        Ok(t::PlaylistTemplate { tabs: Some(tabs) }.into_response())
    } else {
        let index = render_index(
            &state.mpd,
            t::Page::Playlist(t::PlaylistTemplate { tabs: None }),
            tabs,
        )
        .await?;
        Ok(index.into_response())
    }
}

pub async fn clear_playlist(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.clear_playlist().await?;
    Ok(())
}

pub async fn append_album(
    State(state): State<AppState>,
    Query(q): Query<ArtistAlbumQuery>,
) -> Result<(), AppError> {
    state
        .mpd
        .append_album_to_playlist(&q.artist, &q.album)
        .await?;
    Ok(())
}

pub async fn play_album(
    State(state): State<AppState>,
    Query(q): Query<ArtistAlbumQuery>,
) -> Result<(), AppError> {
    state.mpd.play_album(&q.artist, &q.album).await?;
    Ok(())
}

pub async fn play_song_by_url(
    State(state): State<AppState>,
    Query(url_query): Query<UrlQuery>,
) -> Result<(), AppError> {
    state.mpd.play_song_by_url(&url_query.url).await?;
    Ok(())
}

pub async fn append_song_by_url(
    State(state): State<AppState>,
    Query(url_query): Query<UrlQuery>,
) -> Result<(), AppError> {
    state.mpd.append_song_by_url(&url_query.url).await?;
    Ok(())
}

pub async fn remove_song_by_id(
    State(state): State<AppState>,
    Query(q): Query<SongIdQuery>,
) -> Result<(), AppError> {
    if let Some(song_id) = q.song_id {
        state.mpd.remove_from_playlist(song_id).await?;
    }
    Ok(())
}
