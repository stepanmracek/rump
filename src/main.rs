mod mpd;
mod templates;

use axum::{
    extract::{Path, Query},
    response::IntoResponse,
    routing::get,
    Router,
};
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
        .nest_service("/assets", ServeDir::new("assets"));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_library() -> Result<t::HtmlTemplate<t::LibraryTemplate>, String> {
    let artists = mpd::get_artists(None).await?;
    let template = t::LibraryTemplate { artists };
    Ok(t::HtmlTemplate(template))
}

async fn get_artists(
    Query(artists_search_query): Query<ArtistsSearchQuery>,
) -> Result<t::HtmlTemplate<t::ArtistsTemplate>, String> {
    let artists = mpd::get_artists(artists_search_query.q).await?;
    let template = t::ArtistsTemplate { artists };
    Ok(t::HtmlTemplate(template))
}

async fn get_albums(
    Path(artist): Path<String>,
) -> Result<t::HtmlTemplate<t::AlbumsTemplate>, String> {
    let albums = mpd::get_albums(&artist).await?;
    let template = t::AlbumsTemplate { albums };
    Ok(t::HtmlTemplate(template))
}

async fn get_index() -> impl IntoResponse {
    let template = t::IndexTemplate {};
    t::HtmlTemplate(template)
}
