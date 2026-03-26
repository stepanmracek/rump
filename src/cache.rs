use bytes::Bytes;
use std::collections::{HashMap, VecDeque};

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
