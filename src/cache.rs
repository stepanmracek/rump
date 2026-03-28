use bytes::Bytes;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use tokio::sync::Mutex;

use crate::{error::AppError, mpd::Mpd};

pub struct AlbumArtCache {
    cache: HashMap<(String, String), Bytes>,
    keys: VecDeque<(String, String)>,
}

impl AlbumArtCache {
    pub fn new() -> Self {
        let cache = HashMap::new();
        let keys = VecDeque::from([]);
        Self { cache, keys }
    }

    pub fn get(&self, key: &(String, String)) -> Option<Bytes> {
        self.cache.get(key).cloned()
    }

    pub fn set(&mut self, key: (String, String), value: Bytes) {
        let old_val = self.cache.insert(key.clone(), value);
        if old_val.is_none() {
            // new value was added
            tracing::debug!(target: "album_art", "caching new value {}-{}", key.0, key.1);
            self.keys.push_back(key);

            while self.keys.len() > 100 {
                let to_delete = self.keys.pop_front().unwrap();
                tracing::debug!(target: "album_art", "removing cached value {}-{}", to_delete.0, to_delete.1);
                self.cache.remove(&to_delete);
            }
        }
    }
}

pub async fn get_set(
    key: (String, String),
    album_art_cache: Arc<Mutex<AlbumArtCache>>,
    mpd: &Mpd,
) -> Result<Bytes, AppError> {
    {
        let cache = album_art_cache.lock().await;
        if let Some(cached) = cache.get(&key) {
            tracing::debug!(target: "album_art", "returning cached value for {}-{}", key.0, key.1);
            return Ok(cached);
        }
    }

    let art = mpd.album_art(&key.0, &key.1).await?;

    {
        let mut cache = album_art_cache.lock().await;
        cache.set(key, art.clone());
    }

    Ok(art)
}
