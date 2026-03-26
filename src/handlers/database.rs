use crate::error::AppError;
use crate::handlers::library::render_index;
use crate::mpd;
use crate::state::AppState;
use crate::templates as t;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;

pub async fn get_database(
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

pub async fn update_db(State(state): State<AppState>) -> Result<(), AppError> {
    state.mpd.update_db().await?;
    Ok(())
}

pub async fn update_status(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let updating = state.mpd.get_status().await?.ubdating_db;
    let template = t::DatabaseUpdateStatusTemplate { updating };
    Ok(template)
}
