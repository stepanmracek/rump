use crate::cache::AlbumArtCache;
use crate::mpd::Mpd;
use mpd_client::client::Subsystem;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub mpd: Mpd,
    pub album_art_cache: Arc<Mutex<AlbumArtCache>>,
    pub event_tx: broadcast::Sender<Subsystem>,
}
