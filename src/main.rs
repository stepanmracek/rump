mod mpd;
mod templates;
use askama::Template;
use axum::extract::State;
use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use mpd::Mpd;
use mpd_client::client::{ConnectionEvent, Subsystem};
use serde::Deserialize;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use templates as t;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

#[derive(Deserialize)]
struct GenericQuery {
    q: Option<String>,
}

#[derive(Deserialize)]
struct ArtistQuery {
    artist: String,
}

#[derive(Deserialize)]
struct UrlQuery {
    url: String,
}

#[derive(Deserialize)]
struct ArtistAlbumQuery {
    artist: String,
    album: String,
}

#[derive(Deserialize)]
struct SongIdQuery {
    song_id: Option<u64>,
}

struct AlbumArtCache {
    cache: HashMap<(String, String), Vec<u8>>,
    keys: VecDeque<(String, String)>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let album_art_cache: Arc<Mutex<AlbumArtCache>> = {
        let cache = HashMap::new();
        let keys = VecDeque::from([]);
        Mutex::new(AlbumArtCache { cache, keys }).into()
    };

    let app = Router::new()
        .route("/", get(get_index))
        .route("/library", get(get_library))
        .route("/artists", get(get_artists))
        .route("/albums", get(get_albums))
        .route("/songs", get(get_songs))
        .route("/status", get(get_status))
        .route("/control/play", get(control_play_song))
        .route("/control/unpause", get(control_unpause))
        .route("/control/pause", get(control_pause))
        .route("/control/prev", get(control_prev))
        .route("/control/next", get(control_next))
        .route("/playlist", get(get_playlist))
        .route("/playlist/clear", get(clear_playlist))
        .route("/playlist/songs", get(get_playlist_songs))
        .route("/playlist/append/album", get(append_album))
        .route("/playlist/play/album", get(play_album))
        .route("/playlist/play/song", get(play_song_by_url))
        .route("/playlist/append/song", get(append_song_by_url))
        .route("/cover", get(get_cover))
        .with_state(album_art_cache)
        .nest_service("/assets", ServeDir::new("assets"))
        .layer(TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn get_library() -> Result<impl IntoResponse, AppError> {
    let template = t::LibraryTemplate;
    Ok(t::HtmlTemplate(template))
}

async fn get_artists(
    Query(artists_search_query): Query<GenericQuery>,
) -> Result<impl IntoResponse, AppError> {
    let artists = Mpd::connect()
        .await?
        .get_artists(artists_search_query.q)
        .await?;
    let template = t::ArtistsTemplate { artists };
    Ok(t::HtmlTemplate(template))
}

async fn get_albums(Query(query): Query<ArtistQuery>) -> Result<impl IntoResponse, AppError> {
    let albums = Mpd::connect().await?.get_albums(&query.artist).await?;
    let template = t::AlbumsTemplate {
        artist: query.artist,
        albums,
    };
    Ok(t::HtmlTemplate(template))
}

async fn get_songs(Query(q): Query<ArtistAlbumQuery>) -> Result<impl IntoResponse, AppError> {
    let songs = Mpd::connect().await?.get_songs(&q.artist, &q.album).await?;
    let template = t::AlbumSongsTemplate {
        artist: q.artist,
        album: q.album,
        songs,
    };
    Ok(t::HtmlTemplate(template))
}

async fn get_index() -> impl IntoResponse {
    let error = Mpd::connect().await.err().map(|e| e.to_string());
    let template = t::IndexTemplate { error };
    t::HtmlTemplate(template)
}

async fn get_status(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_ws_status)
}

async fn send_mpd_status(mpd: &mut Mpd, socket: &mut WebSocket) -> Result<(), AppError> {
    let mpd_status = mpd.get_status().await;
    match mpd_status {
        Ok(mpd_status) => {
            let template = t::StatusTemplate { status: mpd_status }.render();
            match template {
                Ok(template) => socket.send(template.into()).await?,
                Err(e) => return Err(e.into()),
            };
        }
        Err(e) => return Err(e.into()),
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

async fn control_play_song(Query(q): Query<SongIdQuery>) -> Result<(), AppError> {
    if let Some(song_id) = q.song_id {
        Mpd::connect().await?.play_song(song_id).await?;
    } else {
        Mpd::connect().await?.play().await?;
    }
    Ok(())
}

async fn control_pause() -> Result<(), AppError> {
    Mpd::connect().await?.pause(true).await?;
    Ok(())
}

async fn control_unpause() -> Result<(), AppError> {
    Mpd::connect().await?.pause(false).await?;
    Ok(())
}

async fn control_prev() -> Result<(), AppError> {
    Mpd::connect().await?.prev().await?;
    Ok(())
}

async fn control_next() -> Result<(), AppError> {
    Mpd::connect().await?.next().await?;
    Ok(())
}

async fn clear_playlist() -> Result<(), AppError> {
    Mpd::connect().await?.clear_playlist().await?;
    Ok(())
}

async fn get_playlist_songs(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_ws_playlist)
}

async fn send_playlist(mpd: &Mpd, socket: &mut WebSocket) -> Result<(), AppError> {
    let playlist = mpd.get_playlist().await;
    match playlist {
        Ok(playlist) => {
            let template = t::PlaylistSongsTemplate { songs: playlist }.render();
            match template {
                Ok(template) => socket.send(template.into()).await?,
                Err(e) => return Err(e.into()),
            };
        }
        Err(e) => return Err(e.into()),
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

async fn append_album(Query(q): Query<ArtistAlbumQuery>) -> Result<(), AppError> {
    Mpd::connect()
        .await?
        .append_album_to_playlist(&q.artist, &q.album)
        .await?;
    Ok(())
}

async fn play_album(Query(q): Query<ArtistAlbumQuery>) -> Result<(), AppError> {
    Mpd::connect()
        .await?
        .play_album(&q.artist, &q.album)
        .await?;
    Ok(())
}

async fn play_song_by_url(Query(url_query): Query<UrlQuery>) -> Result<(), AppError> {
    Mpd::connect()
        .await?
        .play_song_by_url(&url_query.url)
        .await?;
    Ok(())
}

async fn append_song_by_url(Query(url_query): Query<UrlQuery>) -> Result<(), AppError> {
    Mpd::connect()
        .await?
        .append_song_by_url(&url_query.url)
        .await?;
    Ok(())
}

async fn get_cover(
    Query(q): Query<ArtistAlbumQuery>,
    State(cache): State<Arc<Mutex<AlbumArtCache>>>,
) -> Result<Vec<u8>, AppError> {
    let cache_key = (q.artist.clone(), q.album.clone());

    if let Ok(cache) = cache.lock() {
        if let Some(cached) = cache.cache.get(&cache_key) {
            tracing::debug!(target: "album_art", "returning cached value for {}-{}", q.artist, q.album);
            return Ok(cached.clone());
        }
    }

    let art = Mpd::connect().await?.album_art(&q.artist, &q.album).await?;

    if let Ok(mut cache) = cache.lock() {
        let old_val = cache.cache.insert(cache_key.clone(), art.clone());
        if old_val.is_none() {
            // new value was added
            tracing::debug!(target: "album_art", "caching new value {}-{}", cache_key.0, cache_key.1);
            cache.keys.push_back(cache_key);

            while cache.keys.len() > 100 {
                let to_delete = cache.keys.pop_front().unwrap();
                tracing::debug!(target: "album_art", "removing cached value {}-{}", to_delete.0, to_delete.1);
                cache.cache.remove(&to_delete);
            }
        }
    }

    Ok(art)
}

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
