use crate::mpd::{Album, Status, SongInQueue};
use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate;

#[derive(Template)]
#[template(path = "library.html")]
pub struct LibraryTemplate {
    pub artists: Vec<String>,
}

#[derive(Template)]
#[template(path = "artists.html")]
pub struct ArtistsTemplate {
    pub artists: Vec<String>,
}

#[derive(Template)]
#[template(path = "albums.html")]
pub struct AlbumsTemplate {
    pub albums: Vec<Album>,
}

#[derive(Template)]
#[template(path = "status.html")]
pub struct StatusTemplate{
    pub status: Status,
}

#[derive(Template)]
#[template(path = "playlist.html")]
pub struct PlaylistTemplate;

#[derive(Template)]
#[template(path = "playlist_songs.html")]
pub struct PlaylistSongsTemplate {
    pub songs: Vec<SongInQueue>,
}

pub struct HtmlTemplate<T>(pub T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {err}"),
            )
                .into_response(),
        }
    }
}
