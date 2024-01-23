use base64::{prelude::BASE64_STANDARD, Engine};

pub struct Album {
    pub album_name: String,
    pub year: Option<i32>,
    pub art: Option<String>,
}

async fn connect() -> Result<mpd_client::Client, String> {
    let connection = tokio::net::TcpStream::connect("localhost:6600")
        .await
        .map_err(|e| e.to_string());
    let (mpd_client, _) = mpd_client::Client::connect(connection?)
        .await
        .map_err(|e| e.to_string())?;
    Ok(mpd_client)
}

pub async fn get_artists(name_filter: Option<String>) -> Result<Vec<String>, String> {
    let mpd_client = connect().await?;
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

pub async fn get_albums(artist: &str) -> Result<Vec<Album>, String> {
    let mpd_client = connect().await?;
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
        let year = first_song
            .and_then(|song| song.tags.get(&mpd_client::tag::Tag::Date))
            .and_then(|tag_values| tag_values.first())
            .and_then(|d| d.parse::<i32>().ok());

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
