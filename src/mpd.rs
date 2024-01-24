use base64::{prelude::BASE64_STANDARD, Engine};
use mpd_client::responses::PlayState;

pub struct Album {
    pub album_name: String,
    pub year: Option<i32>,
    pub art: Option<String>,
}

pub struct Status {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<i32>,
    pub play_state: mpd_client::responses::PlayState,
    pub has_next: bool,
    pub has_prev: bool,
    pub has_song: bool,
}

pub struct SongInQueue {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub playing: bool,
}

pub async fn connect() -> Result<(mpd_client::Client, mpd_client::client::ConnectionEvents), String>
{
    let connection = tokio::net::TcpStream::connect("localhost:6600")
        .await
        .map_err(|e| e.to_string());
    let (mpd_client, connection_events) = mpd_client::Client::connect(connection?)
        .await
        .map_err(|e| e.to_string())?;
    Ok((mpd_client, connection_events))
}

pub async fn get_artists(name_filter: Option<String>) -> Result<Vec<String>, String> {
    let (mpd_client, _) = connect().await?;
    let cmd = mpd_client::commands::List::new(mpd_client::tag::Tag::Artist);
    let response = mpd_client
        .command(cmd)
        .await
        .map_err(|command_error| command_error.to_string());

    let name_filter = name_filter.map(|v| v.to_lowercase());
    Ok(response?
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

pub async fn get_songs(
    mpd_client: &mpd_client::Client,
    artist: &str,
    album: &str,
) -> Result<Vec<mpd_client::responses::Song>, String> {
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
    let mut result = mpd_client
        .command(cmd)
        .await
        .map_err(|command_error| command_error.to_string())?;

    result.sort_by_key(|song| song.number());
    Ok(result)
}

pub fn get_year(song: &mpd_client::responses::Song) -> Option<i32> {
    song.tags
        .get(&mpd_client::tag::Tag::Date)
        .and_then(|tag_values| tag_values.first())
        .and_then(|d| d.parse::<i32>().ok())
}

pub async fn get_albums(artist: &str) -> Result<Vec<Album>, String> {
    let (mpd_client, _) = connect().await?;
    let response = mpd_client
        .command(
            mpd_client::commands::List::new(mpd_client::tag::Tag::Album).filter(
                mpd_client::filter::Filter::new(
                    mpd_client::tag::Tag::Artist,
                    mpd_client::filter::Operator::Equal,
                    artist,
                ),
            ),
        )
        .await
        .map_err(|command_error| command_error.to_string())?;

    let album_names = response.into_iter().collect::<Vec<String>>();
    let mut albums = vec![];
    for album_name in album_names {
        let songs = get_songs(&mpd_client, artist, &album_name).await?;
        let first_song = songs.first();
        let year = first_song.and_then(get_year);

        let art = if let Some(first_song) = first_song {
            mpd_client
                .album_art(&first_song.url)
                .await
                .ok()
                .flatten()
                .and_then(|(bytes, _mime)| {
                    if !bytes.is_empty() {
                        Some(BASE64_STANDARD.encode(bytes))
                    } else {
                        None
                    }
                })
        } else {
            None
        };
        albums.push(Album {
            album_name,
            year,
            art,
        });
    }
    albums.sort_by_key(|album| album.year);
    Ok(albums)
}

pub async fn get_status(mpd_client: &mpd_client::Client) -> Result<Status, String> {
    let (status, current_song) = mpd_client
        .command_list((
            mpd_client::commands::Status,
            mpd_client::commands::CurrentSong,
        ))
        .await
        .map_err(|e| e.to_string())?;

    let play_state = status.state;
    let has_song = status.current_song.is_some();
    let has_next = play_state != PlayState::Stopped
        && status
            .current_song
            .is_some_and(|current_song| current_song.0 .0 + 1 < status.playlist_length);
    let has_prev = play_state != PlayState::Stopped && has_song;

    if let Some(current_song) = current_song {
        let title = current_song.song.title().map(|s| s.to_string());
        let artist = current_song.song.artists().first().map(|s| s.to_string());
        let album = current_song.song.album().map(|s| s.to_string());
        let year = get_year(&current_song.song);

        Ok(Status {
            title,
            artist,
            album,
            year,
            play_state,
            has_next,
            has_prev,
            has_song,
        })
    } else {
        Ok(Status {
            title: None,
            artist: None,
            album: None,
            year: None,
            play_state,
            has_next,
            has_prev,
            has_song,
        })
    }
}

pub async fn prev() -> Result<(), String> {
    let (c, _) = connect().await?;
    c.command(mpd_client::commands::Previous)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn next() -> Result<(), String> {
    let (c, _) = connect().await?;
    c.command(mpd_client::commands::Next)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn pause(pause: bool) -> Result<(), String> {
    let (c, _) = connect().await?;
    c.command(mpd_client::commands::SetPause(pause))
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn play() -> Result<(), String> {
    let (c, _) = connect().await?;
    c.command(mpd_client::commands::Play::current())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn play_song(song_id: u64) -> Result<(), String> {
    let (c, _) = connect().await?;
    c.command(mpd_client::commands::Play::song(
        mpd_client::commands::SongId(song_id),
    ))
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_playlist(mpd_client: &mpd_client::Client) -> Result<Vec<SongInQueue>, String> {
    let (queue, current_song) = mpd_client
        .command_list((
            mpd_client::commands::Queue,
            mpd_client::commands::CurrentSong,
        ))
        .await
        .map_err(|e| e.to_string())?;

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

pub async fn clear_playlist() -> Result<(), String> {
    let (c, _) = connect().await?;
    c.command(mpd_client::commands::ClearQueue)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
