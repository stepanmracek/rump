use serde::Deserialize;

#[derive(Deserialize)]
pub struct GenericQuery {
    pub q: Option<String>,
}

#[derive(Deserialize)]
pub struct ArtistQuery {
    pub artist: String,
}

#[derive(Deserialize)]
pub struct UrlQuery {
    pub url: String,
}

#[derive(Deserialize)]
pub struct ArtistAlbumQuery {
    pub artist: String,
    pub album: String,
}

#[derive(Deserialize)]
pub struct SongIdQuery {
    pub song_id: Option<u64>,
}
