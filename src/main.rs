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
use tokio::sync::Mutex;
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

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
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
        .with_state(album_art_cache)
        .nest_service("/assets", ServeDir::new("assets"))
        .layer(TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn get_library(headers: HeaderMap) -> Result<impl IntoResponse, AppError> {
    let tabs = t::TabsTemplate {
        library_active: true,
        ..Default::default()
    };

    if headers.contains_key("HX-Request") {
        Ok(t::LibraryTemplate { tabs: Some(tabs) }.into_response())
    } else {
        let index = render_index(t::Page::Library(t::LibraryTemplate { tabs: None }), tabs).await?;
        Ok(index.into_response())
    }
}

async fn get_database(headers: HeaderMap) -> Result<impl IntoResponse, AppError> {
    let stats = Mpd::connect().await?.stats().await?;
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

async fn update_db() -> Result<(), AppError> {
    Mpd::connect().await?.update_db().await?;
    Ok(())
}

async fn update_status() -> Result<impl IntoResponse, AppError> {
    let updating = Mpd::connect().await?.get_status().await?.ubdating_db;
    let template = t::DatabaseUpdateStatusTemplate { updating };
    Ok(template)
}

async fn get_artists(
    Query(artists_search_query): Query<GenericQuery>,
) -> Result<impl IntoResponse, AppError> {
    let artists = Mpd::connect()
        .await?
        .get_artists(artists_search_query.q)
        .await?;
    Ok(t::ArtistsTemplate::new(artists))
}

async fn get_albums(
    Query(query): Query<ArtistQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let albums = Mpd::connect().await?.get_albums(&query.artist).await?;
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
    Query(q): Query<ArtistAlbumQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let songs = Mpd::connect().await?.get_songs(&q.artist, &q.album).await?;
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

async fn render_index(page: t::Page, tabs: t::TabsTemplate) -> Result<impl IntoResponse, AppError> {
    let error = Mpd::connect().await.err().map(|e| e.to_string());
    Ok(t::IndexTemplate { error, page, tabs })
}

async fn get_index() -> Result<impl IntoResponse, AppError> {
    get_library(HeaderMap::default()).await
}

async fn get_status(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_ws_status)
}

async fn send_mpd_status(mpd: &mut Mpd, socket: &mut WebSocket) -> anyhow::Result<()> {
    let mpd_status = mpd.get_status().await?;
    let template = t::StatusTemplate { status: mpd_status }.render()?;
    socket.send(template.into()).await?;
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

async fn toggle_random() -> Result<(), AppError> {
    Mpd::connect().await?.toggle_random().await?;
    Ok(())
}

async fn toggle_repeat() -> Result<(), AppError> {
    Mpd::connect().await?.toggle_repeat().await?;
    Ok(())
}

async fn get_playlist_songs(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_ws_playlist)
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
            | ConnectionEvent::SubsystemChange(Subsystem::Queue)
            | ConnectionEvent::SubsystemChange(Subsystem::Options) => {
                if send_playlist(&mpd, &mut socket).await.is_err() {
                    return;
                }
            }
            ConnectionEvent::ConnectionClosed(_) => return,
            ConnectionEvent::SubsystemChange(_) => {}
        }
    }
}

async fn get_playlist(headers: HeaderMap) -> Result<impl IntoResponse, AppError> {
    let tabs = t::TabsTemplate {
        playlist_active: true,
        ..Default::default()
    };

    if headers.contains_key("HX-Request") {
        Ok(t::PlaylistTemplate { tabs: Some(tabs) }.into_response())
    } else {
        let index =
            render_index(t::Page::Playlist(t::PlaylistTemplate { tabs: None }), tabs).await?;
        Ok(index.into_response())
    }
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

async fn remove_song_by_id(Query(q): Query<SongIdQuery>) -> Result<(), AppError> {
    if let Some(song_id) = q.song_id {
        Mpd::connect().await?.remove_from_playlist(song_id).await?;
    }
    Ok(())
}

async fn get_cover(
    Query(q): Query<ArtistAlbumQuery>,
    State(cache): State<Arc<Mutex<AlbumArtCache>>>,
) -> Result<Vec<u8>, AppError> {
    let cache_key = (q.artist.clone(), q.album.clone());
    let mut cache = cache.lock().await;

    if let Some(cached) = cache.cache.get(&cache_key) {
        tracing::debug!(target: "album_art", "returning cached value for {}-{}", q.artist, q.album);
        return Ok(cached.clone());
    }

    let art = Mpd::connect().await?.album_art(&q.artist, &q.album).await?;

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

    Ok(art)
}

async fn get_now_playing(headers: HeaderMap) -> Result<impl IntoResponse, AppError> {
    let tabs = t::TabsTemplate {
        now_playing_active: true,
        ..Default::default()
    };

    if headers.contains_key("HX-Request") {
        Ok(t::NowPlayingTemplate { tabs: Some(tabs) }.into_response())
    } else {
        let index = render_index(
            t::Page::NowPlaying(t::NowPlayingTemplate { tabs: None }),
            tabs,
        )
        .await?;
        Ok(index.into_response())
    }
}

async fn get_now_playing_content(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_ws_now_playing)
}

async fn handle_ws_now_playing(mut socket: WebSocket) {
    let mpd = Mpd::connect().await;
    if mpd.is_err() {
        return;
    }
    let mpd = mpd.unwrap();

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
