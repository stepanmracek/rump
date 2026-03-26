mod mpd;
mod templates;
use askama::Template;
use axum::extract::State;
use axum::http::HeaderMap;
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
use std::sync::Arc;
use templates as t;
use tokio::sync::{broadcast, Mutex};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

impl AlbumArtCache {
    pub fn new() -> Self {
        let cache = HashMap::new();
        let keys = VecDeque::from([]);
        Self { cache, keys }
    }

    pub fn get(&self, key: &(String, String)) -> Option<&Vec<u8>> {
        self.cache.get(key)
    }

    pub fn set(&mut self, key: (String, String), value: Vec<u8>) {
        let old_val = self.cache.insert(key.clone(), value);
        if old_val.is_none() {
            // new value was added
            tracing::debug!(target: "album_art", "caching new value {}-{}", key.0, key.1);
            self.keys.push_back(key);

            while self.keys.len() > 100 {
                let to_delete = self.keys.pop_front().unwrap();
                tracing::debug!(target: "album_art", "removing cached value {}-{}", to_delete.0, to_delete.1);
                self.cache.remove(&to_delete);
            }
        }
    }
}

#[derive(Clone)]
struct AppState {
    mpd: Mpd,
    album_art_cache: Arc<Mutex<AlbumArtCache>>,
    event_tx: broadcast::Sender<Subsystem>,
}

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
        .route("/control/toggle_repeat", get(toggle_repeat))
        .route("/control/toggle_random", get(toggle_random))
        .route("/playlist", get(get_playlist))
        .route("/playlist/clear", get(clear_playlist))
        .route("/playlist/songs", get(get_playlist_songs))
        .route("/playlist/append/album", get(append_album))
        .route("/playlist/play/album", get(play_album))
        .route("/playlist/play/song", get(play_song_by_url))
        .route("/playlist/append/song", get(append_song_by_url))
        .route("/playlist/remove/song", get(remove_song_by_id))
        .route("/cover", get(get_cover))
        .route("/database", get(get_database))
        .route("/database/update_db", get(update_db))
        .route("/database/update_status", get(update_status))
        .route("/now_playing", get(get_now_playing))
        .route("/now_playing/content", get(get_now_playing_content))
        .with_state(state)
        .nest_service("/assets", ServeDir::new("assets"))
        .layer(TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn get_library(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let tabs = t::TabsTemplate {
        library_active: true,
        ..Default::default()
    };

    if headers.contains_key("HX-Request") {
        Ok(t::LibraryTemplate { tabs: Some(tabs) }.into_response())
    } else {
        let index = render_index(
            &state.mpd,
            t::Page::Library(t::LibraryTemplate { tabs: None }),
            tabs,
        )
        .await?;
        Ok(index.into_response())
    }
}

async fn get_database(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let stats = state.mpd.stats().await?;
    let tabs = t::TabsTemplate {
        database_active: true,
        ..Default::default()
    };
    let mpd_addr = mpd::mpd_addr();

    if headers.contains_key("HX-Request") {
        Ok(t::DatabaseTemplate {
            tabs: Some(tabs),
            mpd_addr,
            stats,
        }
        .into_response())
    } else {
        let index = render_index(
            &state.mpd,
            t::Page::Database(t::DatabaseTemplate {
                tabs: None,
                mpd_addr,
                stats,
            }),
            tabs,
        )
        .await?;
        Ok(index.into_response())
    }
}

async fn update_db(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.update_db().await?;
    Ok(())
}

async fn update_status(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let updating = state.mpd.get_status().await?.ubdating_db;
    let template = t::DatabaseUpdateStatusTemplate { updating };
    Ok(template)
}

async fn get_artists(
    State(state): State<AppState>,
    Query(artists_search_query): Query<GenericQuery>,
) -> Result<impl IntoResponse, AppError> {
    let artists = state.mpd.get_artists(artists_search_query.q).await?;
    Ok(t::ArtistsTemplate::new(artists))
}

async fn get_albums(
    State(state): State<AppState>,
    Query(query): Query<ArtistQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let albums = state.mpd.get_albums(&query.artist).await?;
    let tabs = t::TabsTemplate {
        library_active: true,
        ..Default::default()
    };

    if headers.contains_key("HX-Request") {
        Ok(t::AlbumsTemplate {
            tabs: Some(tabs),
            artist: query.artist,
            albums,
        }
        .into_response())
    } else {
        let index = render_index(
            &state.mpd,
            t::Page::Albums(t::AlbumsTemplate {
                tabs: None,
                artist: query.artist,
                albums,
            }),
            tabs,
        )
        .await?;
        Ok(index.into_response())
    }
}

async fn get_songs(
    State(state): State<AppState>,
    Query(q): Query<ArtistAlbumQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let songs = state.mpd.get_songs(&q.artist, &q.album).await?;
    let tabs = t::TabsTemplate {
        library_active: true,
        ..Default::default()
    };

    if headers.contains_key("HX-Request") {
        Ok(t::AlbumSongsTemplate {
            tabs: Some(tabs),
            artist: q.artist,
            album: q.album,
            songs,
        }
        .into_response())
    } else {
        let index = render_index(
            &state.mpd,
            t::Page::Songs(t::AlbumSongsTemplate {
                tabs: None,
                artist: q.artist,
                album: q.album,
                songs,
            }),
            tabs,
        )
        .await?;
        Ok(index.into_response())
    }
}

async fn render_index(
    mpd: &Mpd,
    page: t::Page,
    tabs: t::TabsTemplate,
) -> Result<impl IntoResponse, AppError> {
    let error = mpd.stats().await.err().map(|e| e.to_string());
    Ok(t::IndexTemplate { error, page, tabs })
}

async fn get_index(
    state: State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    get_library(state, headers).await
}

async fn get_status(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
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

async fn control_play_song(
    State(state): State<AppState>,
    Query(q): Query<SongIdQuery>,
) -> Result<(), AppError> {
    if let Some(song_id) = q.song_id {
        state.mpd.play_song(song_id).await?;
    } else {
        state.mpd.play().await?;
    }
    Ok(())
}

async fn control_pause(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.pause(true).await?;
    Ok(())
}

async fn control_unpause(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.pause(false).await?;
    Ok(())
}

async fn control_prev(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.prev().await?;
    Ok(())
}

async fn control_next(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.next().await?;
    Ok(())
}

async fn clear_playlist(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.clear_playlist().await?;
    Ok(())
}

async fn toggle_random(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.toggle_random().await?;
    Ok(())
}

async fn toggle_repeat(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.toggle_repeat().await?;
    Ok(())
}

async fn get_playlist_songs(
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
            Subsystem::Player | Subsystem::Queue | Subsystem::Options => {
                if send_playlist(&mpd, &mut socket).await.is_err() {
                    return;
                }
            }
            _ => {}
        }
    }
}

async fn get_playlist(
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

async fn append_album(
    State(state): State<AppState>,
    Query(q): Query<ArtistAlbumQuery>,
) -> Result<(), AppError> {
    state
        .mpd
        .append_album_to_playlist(&q.artist, &q.album)
        .await?;
    Ok(())
}

async fn play_album(
    State(state): State<AppState>,
    Query(q): Query<ArtistAlbumQuery>,
) -> Result<(), AppError> {
    state.mpd.play_album(&q.artist, &q.album).await?;
    Ok(())
}

async fn play_song_by_url(
    State(state): State<AppState>,
    Query(url_query): Query<UrlQuery>,
) -> Result<(), AppError> {
    state.mpd.play_song_by_url(&url_query.url).await?;
    Ok(())
}

async fn append_song_by_url(
    State(state): State<AppState>,
    Query(url_query): Query<UrlQuery>,
) -> Result<(), AppError> {
    state.mpd.append_song_by_url(&url_query.url).await?;
    Ok(())
}

async fn remove_song_by_id(
    State(state): State<AppState>,
    Query(q): Query<SongIdQuery>,
) -> Result<(), AppError> {
    if let Some(song_id) = q.song_id {
        state.mpd.remove_from_playlist(song_id).await?;
    }
    Ok(())
}

async fn get_cover(
    State(state): State<AppState>,
    Query(q): Query<ArtistAlbumQuery>,
) -> Result<Vec<u8>, AppError> {
    let cache_key = (q.artist.clone(), q.album.clone());
    let mut cache = state.album_art_cache.lock().await;

    if let Some(cached) = cache.get(&cache_key) {
        tracing::debug!(target: "album_art", "returning cached value for {}-{}", q.artist, q.album);
        return Ok(cached.clone());
    }

    let art = state.mpd.album_art(&q.artist, &q.album).await?;
    cache.set(cache_key, art.clone());
    Ok(art)
}

async fn get_now_playing(
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

async fn get_now_playing_content(
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
