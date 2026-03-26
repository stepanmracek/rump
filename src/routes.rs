use crate::handlers::{album_art, controls, database, library, now_playing, playlist, status};
use crate::state::AppState;
use axum::{routing::get, Router};
use tower_http::services::ServeDir;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(library::get_index))
        .route("/library", get(library::get_library))
        .route("/artists", get(library::get_artists))
        .route("/albums", get(library::get_albums))
        .route("/songs", get(library::get_songs))
        .route("/status", get(status::get_status))
        .route("/control/play", get(controls::control_play_song))
        .route("/control/unpause", get(controls::control_unpause))
        .route("/control/pause", get(controls::control_pause))
        .route("/control/prev", get(controls::control_prev))
        .route("/control/next", get(controls::control_next))
        .route("/control/toggle_repeat", get(controls::toggle_repeat))
        .route("/control/toggle_random", get(controls::toggle_random))
        .route("/playlist", get(playlist::get_playlist))
        .route("/playlist/clear", get(playlist::clear_playlist))
        .route("/playlist/songs", get(playlist::get_playlist_songs))
        .route("/playlist/append/album", get(playlist::append_album))
        .route("/playlist/play/album", get(playlist::play_album))
        .route("/playlist/play/song", get(playlist::play_song_by_url))
        .route("/playlist/append/song", get(playlist::append_song_by_url))
        .route("/playlist/remove/song", get(playlist::remove_song_by_id))
        .route("/cover", get(album_art::get_cover))
        .route("/database", get(database::get_database))
        .route("/database/update_db", get(database::update_db))
        .route("/database/update_status", get(database::update_status))
        .route("/now_playing", get(now_playing::get_now_playing))
        .route(
            "/now_playing/content",
            get(now_playing::get_now_playing_content),
        )
        .with_state(state)
        .nest_service("/assets", ServeDir::new("assets"))
}
