use crate::mpd::{Album, Song, SongInQueue, Status};
use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub error: Option<String>,
}

#[derive(Template, Default)]
#[template(path = "tabs.html")]
pub struct TabsTemplate {
    pub library_active: bool,
    pub playlist_active: bool,
    pub settings_active: bool,
}

#[derive(Template)]
#[template(path = "library.html")]
pub struct LibraryTemplate {
    pub tabs: TabsTemplate,
}

#[derive(Template)]
#[template(path = "artists.html")]
pub struct ArtistsTemplate {
    pub artists: Vec<String>,
}

#[derive(Template)]
#[template(path = "albums.html")]
pub struct AlbumsTemplate {
    pub tabs: TabsTemplate,
    pub artist: String,
    pub albums: Vec<Album>,
}

#[derive(Template)]
#[template(path = "status.html")]
pub struct StatusTemplate {
    pub status: Status,
}

#[derive(Template)]
#[template(path = "playlist.html")]
pub struct PlaylistTemplate {
    pub tabs: TabsTemplate,
}

#[derive(Template)]
#[template(path = "playlist_songs.html")]
pub struct PlaylistSongsTemplate {
    pub songs: Vec<SongInQueue>,
    pub status: Status,
}

#[derive(Template)]
#[template(path = "album_songs.html")]
pub struct AlbumSongsTemplate {
    pub tabs: TabsTemplate,
    pub artist: String,
    pub album: String,
    pub songs: Vec<Song>,
}

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub tabs: TabsTemplate,
    pub stats: mpd_client::responses::Stats,
}

mod filters {
    use chrono::DateTime;
    use std::fmt::Error;

    pub fn duration(total_seconds: u64) -> ::askama::Result<String> {
        let seconds = total_seconds % 60;
        let minutes = (total_seconds / 60) % 60;
        let hours = (total_seconds / 60) / 60;
        Ok(format!("{:0>2}:{:0>2}:{:0>2}", hours, minutes, seconds))
    }

    pub fn datetime(unix_timestamp: &u64) -> ::askama::Result<String> {
        let secs = *unix_timestamp as i64;
        let datetime = DateTime::from_timestamp(secs, 0).ok_or(::askama::Error::Fmt(Error))?;
        Ok(format!("{}", datetime.format("%Y-%m-%d %H:%M:%S+00:00")))
    }
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
