use crate::error::AppError;
use crate::models::ArtistAlbumQuery;
use crate::state::AppState;
use axum::extract::{Query, State};
use bytes::Bytes;

pub async fn get_cover(
    State(state): State<AppState>,
    Query(q): Query<ArtistAlbumQuery>,
) -> Result<Bytes, AppError> {
    let cache_key = (q.artist.clone(), q.album.clone());

    {
        let cache = state.album_art_cache.lock().await;
        if let Some(cached) = cache.get(&cache_key) {
            tracing::debug!(target: "album_art", "returning cached value for {}-{}", q.artist, q.album);
            return Ok(cached);
        }
    }

    let art = state.mpd.album_art(&q.artist, &q.album).await?;

    {
        let mut cache = state.album_art_cache.lock().await;
        cache.set(cache_key, art.clone());
    }

    Ok(art)
}
