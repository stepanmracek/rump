use anyhow::Result;
use bytes::BytesMut;
use mpd_client::responses::PlayState;
use std::io::Read;

pub struct Album {
    pub album_name: String,
    pub year: Option<i32>,
}

pub struct Status {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    // pub year: Option<i32>,
    pub play_state: mpd_client::responses::PlayState,
    pub has_next: bool,
    pub has_prev: bool,
    pub has_song: bool,
    pub single_mode: mpd_client::commands::SingleMode,
    pub repeat: bool,
    pub random: bool,
    // pub consume: bool,
    pub ubdating_db: bool,
    pub elapsed: Option<f64>,
    pub duration: Option<f64>,
}

pub struct SongInQueue {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub playing: bool,
}

pub struct Song {
    pub url: String,
    // pub artist: String,
    // pub album: String,
    pub title: String,
    pub year: Option<i32>,
}

pub struct Mpd {
    pub mpd_client: mpd_client::Client,
    pub connection_events: mpd_client::client::ConnectionEvents,
}

pub fn get_single_tag_value<T>(
    song: &mpd_client::responses::Song,
    tag: &mpd_client::tag::Tag,
) -> Option<T>
where
    T: std::str::FromStr,
{
    song.tags
        .get(tag)
        .and_then(|tag_values| tag_values.first())
        .and_then(|d| d.parse::<T>().ok())
}

pub fn mpd_addr() -> String {
    let host = std::env::var("MPD_HOST").unwrap_or("localhost".to_string());
    let port = std::env::var("MPD_PORT").unwrap_or("6600".to_string());
    format!("{host}:{port}")
}

pub fn scale_down_if_needed(bytes: BytesMut, max_size: u32) -> Result<Vec<u8>> {
    let img = image::load_from_memory(&bytes)?;
    if img.width() > max_size {
        let resized = image::imageops::resize(
            &img,
            max_size,
            max_size,
            image::imageops::FilterType::Triangle,
        );
        let mut cursor = std::io::Cursor::new(vec![]);
        resized.write_to(&mut cursor, image::ImageFormat::Png)?;
        Ok(cursor.into_inner())
    } else {
        Ok(bytes.into_iter().collect())
    }
}

impl Mpd {
    pub async fn connect() -> Result<Self> {
        let addr = mpd_addr();
        let connection = tokio::net::TcpStream::connect(addr).await;
        let (mpd_client, connection_events) = mpd_client::Client::connect(connection?).await?;
        Ok(Self {
            mpd_client,
            connection_events,
        })
    }

    pub async fn get_artists(&self, name_filter: Option<String>) -> Result<Vec<String>> {
        let cmd = mpd_client::commands::List::new(mpd_client::tag::Tag::Artist);
        let response = self.mpd_client.command(cmd).await?;

        let name_filter = name_filter.map(|v| v.to_lowercase());
        Ok(response
            .into_iter()
            .filter(|artist| {
                if let Some(name_filter) = &name_filter {
                    artist.to_lowercase().contains(name_filter)
                } else {
                    true
                }
            })
            .collect::<Vec<_>>())
    }

    pub async fn get_songs(&self, artist: &str, album: &str) -> Result<Vec<Song>> {
        let cmd = mpd_client::commands::Find::new(
            mpd_client::filter::Filter::new(
                mpd_client::tag::Tag::Artist,
                mpd_client::filter::Operator::Equal,
                artist,
            )
            .and(mpd_client::filter::Filter::new(
                mpd_client::tag::Tag::Album,
                mpd_client::filter::Operator::Equal,
                album,
            )),
        );
        let mut result = self.mpd_client.command(cmd).await?;

        result.sort_by_key(|song| song.number());
        Ok(result
            .into_iter()
            .map(|song| Song {
                // artist: get_single_tag_value(&song, &mpd_client::tag::Tag::Artist).unwrap_or_default(),
                // album: get_single_tag_value(&song, &mpd_client::tag::Tag::Album).unwrap_or_default(),
                title: get_single_tag_value(&song, &mpd_client::tag::Tag::Title)
                    .unwrap_or_default(),
                year: get_single_tag_value(&song, &mpd_client::tag::Tag::Date),
                url: song.url,
            })
            .collect())
    }

    pub async fn album_art(&self, artist: &str, album: &str) -> Result<Vec<u8>> {
        let fallback = || {
            let mut file = std::fs::File::open("assets/lp.png").unwrap();
            let mut buf = vec![];
            file.read_to_end(&mut buf).unwrap();
            buf
        };

        let url = self
            .get_songs(artist, album)
            .await?
            .first()
            .map(|first_song| first_song.url.clone());
        if url.is_none() {
            return Ok(fallback());
        }

        let art = self
            .mpd_client
            .album_art(&url.unwrap())
            .await?
            .and_then(|(bytes, _)| scale_down_if_needed(bytes, 256).ok());

        if let Some(art) = art {
            Ok(art)
        } else {
            Ok(fallback())
        }
    }

    pub async fn get_albums(&self, artist: &str) -> Result<Vec<Album>> {
        let response = self
            .mpd_client
            .command(
                mpd_client::commands::List::new(mpd_client::tag::Tag::Album).filter(
                    mpd_client::filter::Filter::new(
                        mpd_client::tag::Tag::Artist,
                        mpd_client::filter::Operator::Equal,
                        artist,
                    ),
                ),
            )
            .await?;

        let album_names = response.into_iter().collect::<Vec<String>>();
        let mut albums = vec![];
        for album_name in album_names {
            let songs = self.get_songs(artist, &album_name).await?;
            let first_song = songs.first();
            let year = first_song.and_then(|song| song.year);

            albums.push(Album { album_name, year });
        }
        albums.sort_by_key(|album| album.year);
        Ok(albums)
    }

    pub async fn get_status(&self) -> Result<Status> {
        let (status, current_song) = self
            .mpd_client
            .command_list((
                mpd_client::commands::Status,
                mpd_client::commands::CurrentSong,
            ))
            .await?;

        let play_state = status.state;
        let has_song = status.current_song.is_some();
        let has_next = play_state != PlayState::Stopped
            && status
                .current_song
                .is_some_and(|current_song| current_song.0 .0 + 1 < status.playlist_length);
        let has_prev = play_state != PlayState::Stopped && has_song;

        let title = current_song
            .as_ref()
            .and_then(|song| song.song.title().map(|s| s.to_string()));
        let artist = current_song
            .as_ref()
            .and_then(|song| song.song.artists().first().map(|s| s.to_string()));
        let album = current_song
            .as_ref()
            .and_then(|song| song.song.album().map(|s| s.to_string()));
        // let year = current_song
        //     .as_ref()
        //     .and_then(|song| get_single_tag_value::<i32>(&song.song, &mpd_client::tag::Tag::Date));
        let elapsed = status.elapsed.map(|e| e.as_secs_f64());
        let duration = status.duration.map(|d| d.as_secs_f64());

        Ok(Status {
            title,
            artist,
            album,
            // year,
            play_state,
            has_next,
            has_prev,
            has_song,
            // consume: status.consume,
            single_mode: status.single,
            repeat: status.repeat,
            random: status.random,
            ubdating_db: status.update_job.is_some(),
            elapsed,
            duration,
        })
    }

    pub async fn prev(&self) -> Result<()> {
        self.mpd_client
            .command(mpd_client::commands::Previous)
            .await?;
        Ok(())
    }

    pub async fn next(&self) -> Result<()> {
        self.mpd_client.command(mpd_client::commands::Next).await?;
        Ok(())
    }

    pub async fn pause(&self, pause: bool) -> Result<()> {
        self.mpd_client
            .command(mpd_client::commands::SetPause(pause))
            .await?;
        Ok(())
    }

    pub async fn play(&self) -> Result<()> {
        self.mpd_client
            .command(mpd_client::commands::Play::current())
            .await?;
        Ok(())
    }

    pub async fn play_song(&self, song_id: u64) -> Result<()> {
        self.mpd_client
            .command(mpd_client::commands::Play::song(
                mpd_client::commands::SongId(song_id),
            ))
            .await?;
        Ok(())
    }

    pub async fn play_song_by_url(&self, url: &str) -> Result<()> {
        let song_id = self
            .mpd_client
            .command_list((
                mpd_client::commands::ClearQueue,
                mpd_client::commands::Add::uri(url),
            ))
            .await?
            .1;
        self.play_song(song_id.0).await?;
        Ok(())
    }

    pub async fn append_song_by_url(&self, url: &str) -> Result<()> {
        self.mpd_client
            .command(mpd_client::commands::Add::uri(url))
            .await?;
        Ok(())
    }

    pub async fn get_playlist(&self) -> Result<Vec<SongInQueue>> {
        let (queue, current_song) = self
            .mpd_client
            .command_list((
                mpd_client::commands::Queue,
                mpd_client::commands::CurrentSong,
            ))
            .await?;

        Ok(queue
            .iter()
            .map(|song| SongInQueue {
                id: song.id.0,
                artist: song
                    .song
                    .artists()
                    .first()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                title: song.song.title().map(|s| s.to_string()).unwrap_or_default(),
                playing: current_song
                    .as_ref()
                    .is_some_and(|current_song| current_song.id == song.id),
            })
            .collect())
    }

    pub async fn remove_from_playlist(&self, song_id: u64) -> Result<()> {
        self.mpd_client
            .command(mpd_client::commands::Delete::id(song_id.into()))
            .await?;
        Ok(())
    }

    pub async fn clear_playlist(&self) -> Result<()> {
        self.mpd_client
            .command(mpd_client::commands::ClearQueue)
            .await?;
        Ok(())
    }

    pub async fn append_album_to_playlist(&self, artist: &str, album: &str) -> Result<()> {
        let songs = self.get_songs(artist, album).await?;
        let commands = songs
            .iter()
            .map(|song| mpd_client::commands::Add::uri(&song.url))
            .collect::<Vec<_>>();
        self.mpd_client.command_list(commands).await?;
        Ok(())
    }

    pub async fn play_album(&self, artist: &str, album: &str) -> Result<()> {
        self.clear_playlist().await?;
        let songs = self.get_songs(artist, album).await?;
        let commands = songs
            .iter()
            .map(|song| mpd_client::commands::Add::uri(&song.url))
            .collect::<Vec<_>>();
        let ids = self.mpd_client.command_list(commands).await?;
        if let Some(id) = ids.first() {
            self.play_song(id.0).await?;
        }
        Ok(())
    }

    pub async fn toggle_repeat(&self) -> Result<()> {
        let repeat = self
            .mpd_client
            .command(mpd_client::commands::Status)
            .await?
            .repeat;
        self.mpd_client
            .command(mpd_client::commands::SetRepeat(!repeat))
            .await?;
        Ok(())
    }

    pub async fn toggle_random(&self) -> Result<()> {
        let random = self
            .mpd_client
            .command(mpd_client::commands::Status)
            .await?
            .random;
        self.mpd_client
            .command(mpd_client::commands::SetRandom(!random))
            .await?;
        Ok(())
    }

    pub async fn stats(&self) -> Result<mpd_client::responses::Stats> {
        Ok(self.mpd_client.command(mpd_client::commands::Stats).await?)
    }

    pub async fn update_db(&self) -> Result<()> {
        self.mpd_client
            .command(mpd_client::commands::Update::new())
            .await?;
        Ok(())
    }
}
