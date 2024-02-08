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
    pub database_active: bool,
    pub now_playing_active: bool,
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
#[template(path = "database.html")]
pub struct DatabaseTemplate {
    pub tabs: TabsTemplate,
    pub mpd_addr: String,
    pub stats: mpd_client::responses::Stats,
}

#[derive(Template)]
#[template(path = "database_update_status.html")]
pub struct DatabaseUpdateStatusTemplate {
    pub updating: bool,
}

#[derive(Template)]
#[template(path = "now_playing.html")]
pub struct NowPlayingTemplate {
    pub tabs: TabsTemplate,
}

#[derive(Template)]
#[template(path = "now_playing_content.html")]
pub struct NowPlayingContentTemplate {
    pub status: Status,
}

impl NowPlayingContentTemplate {
    pub fn progress(&self) -> Option<f64> {
        if let Some(elapsed) = &self.status.elapsed {
            if let Some(duration) = &self.status.duration {
                return Some((*elapsed as f64) * 100.0 / (*duration as f64));
            }
        }
        None
    }
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

    pub fn duration_m_s(total_seconds: &&u64) -> ::askama::Result<String> {
        let total_seconds = **total_seconds;
        let seconds = total_seconds % 60;
        let minutes = total_seconds / 60;
        Ok(format!("{:0>2}:{:0>2}", minutes, seconds))
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
