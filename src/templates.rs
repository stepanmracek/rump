use crate::mpd::{Album, Song, SongInQueue, Status};
use askama::Template;

pub enum Page {
    Library(LibraryTemplate),
    Albums(AlbumsTemplate),
    Songs(AlbumSongsTemplate),
    NowPlaying(NowPlayingTemplate),
    Database(DatabaseTemplate),
    Playlist(PlaylistTemplate),
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub error: Option<String>,
    pub page: Page,
    pub tabs: TabsTemplate,
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
    pub tabs: Option<TabsTemplate>,
}

#[derive(Template)]
#[template(path = "artists.html")]
pub struct ArtistsTemplate {
    pub artists: Vec<(char, Vec<String>)>,
}

impl ArtistsTemplate {
    pub fn new(artists_vec: Vec<String>) -> Self {
        let mut artists_vec = artists_vec.clone();
        artists_vec.sort_by_key(|a| a.to_lowercase());
        let mut artists: Vec<(char, Vec<String>)> = vec![];
        for artist in artists_vec.into_iter() {
            let letter: char = artist
                .chars()
                .next()
                .unwrap_or_default()
                .to_uppercase()
                .next()
                .unwrap_or_default();
            if artists.is_empty() || artists.last().unwrap().0 != letter {
                artists.push((letter, vec![artist]))
            } else {
                artists.last_mut().unwrap().1.push(artist);
            }
        }
        ArtistsTemplate { artists }
    }
}

#[derive(Template)]
#[template(path = "albums.html")]
pub struct AlbumsTemplate {
    pub tabs: Option<TabsTemplate>,
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
    pub tabs: Option<TabsTemplate>,
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
    pub tabs: Option<TabsTemplate>,
    pub artist: String,
    pub album: String,
    pub songs: Vec<Song>,
}

#[derive(Template)]
#[template(path = "database.html")]
pub struct DatabaseTemplate {
    pub tabs: Option<TabsTemplate>,
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
    pub tabs: Option<TabsTemplate>,
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
                return Some(elapsed * 100.0 / duration);
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

    pub fn duration_m_s(total_seconds: &&f64) -> ::askama::Result<String> {
        let total_seconds = (**total_seconds) as u64;
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
