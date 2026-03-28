use crate::models::ArtistAlbumQuery;
use crate::state::AppState;
use crate::{cache::get_set, error::AppError};
use axum::extract::{Query, State};
use bytes::Bytes;

pub async fn get_cover(
    State(state): State<AppState>,
    Query(q): Query<ArtistAlbumQuery>,
) -> Result<Bytes, AppError> {
    let cache_key = (q.artist.clone(), q.album.clone());
    get_set(cache_key, state.album_art_cache, &state.mpd).await
}
