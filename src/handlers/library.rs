use crate::error::AppError;
use crate::models::{ArtistAlbumQuery, ArtistQuery, GenericQuery};
use crate::mpd::Mpd;
use crate::state::AppState;
use crate::templates as t;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;

pub async fn get_library(
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

pub async fn get_artists(
    State(state): State<AppState>,
    Query(artists_search_query): Query<GenericQuery>,
) -> Result<impl IntoResponse, AppError> {
    let artists = state.mpd.get_artists(artists_search_query.q).await?;
    Ok(t::ArtistsTemplate::new(artists))
}

pub async fn get_albums(
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

pub async fn get_songs(
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

pub async fn render_index(
    mpd: &Mpd,
    page: t::Page,
    tabs: t::TabsTemplate,
) -> Result<impl IntoResponse, AppError> {
    let error = mpd.stats().await.err().map(|e| e.to_string());
    Ok(t::IndexTemplate { error, page, tabs })
}

pub async fn get_index(
    state: State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    get_library(state, headers).await
}
