use crate::error::AppError;
use crate::models::SongIdQuery;
use crate::state::AppState;
use axum::extract::{Query, State};

pub async fn control_play_song(
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

pub async fn control_pause(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.pause(true).await?;
    Ok(())
}

pub async fn control_unpause(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.pause(false).await?;
    Ok(())
}

pub async fn control_prev(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.prev().await?;
    Ok(())
}

pub async fn control_next(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.next().await?;
    Ok(())
}

pub async fn toggle_random(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.toggle_random().await?;
    Ok(())
}

pub async fn toggle_repeat(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.toggle_repeat().await?;
    Ok(())
}
