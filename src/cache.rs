//! Persistent directory metadata only; no audio data or proxy credentials are stored.
use crate::config::{Config, Station};
use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_BYTES: u64 = 10 * 1024 * 1024;
const MAX_ENTRIES: usize = 128;
static TEMP_ID: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Debug)]
pub struct CacheKey(pub(crate) String);
#[derive(Serialize, Deserialize)]
struct Entry {
    version: u8,
    key: String,
    saved_at: u64,
    stations: Vec<Station>,
}
pub struct CachedDirectory {
    pub saved_at: u64,
    pub fresh: bool,
    pub stations: Vec<Station>,
}
#[derive(Clone)]
pub struct DirectoryCache {
    pub root: PathBuf,
    gate: Arc<Mutex<()>>,
    epoch: Arc<AtomicU64>,
    max_bytes: u64,
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn key(config: &Config, query: &str) -> Option<CacheKey> {
    let source = match config.source {
        0 => "radio-browser".to_string(),
        1 => "somafm".to_string(),
        2 | 3 => return None,
        i => {
            let location = &config.subscriptions.get(i - 4)?.location;
            if !location.starts_with("http://") && !location.starts_with("https://") {
                return None;
            }
            format!("subscription:{location}")
        }
    };
    let identity = serde_json::to_vec(&(source, query.trim(), &config.proxy)).ok()?;
    Some(CacheKey(digest(&identity)))
}
pub fn ttl(query: &str) -> u64 {
    if query.trim().is_empty() { 3600 } else { 600 }
}
pub fn timestamp(saved_at: u64) -> String {
    chrono::DateTime::from_timestamp(saved_at as i64, 0)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "未知时间".into())
}
impl DirectoryCache {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            gate: Arc::new(Mutex::new(())),
            epoch: Arc::new(AtomicU64::new(0)),
            max_bytes: MAX_BYTES,
        }
    }
    pub fn for_config(path: &Path) -> Self {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        };
        let base = ProjectDirs::from("", "", "tunerift")
            .map(|d| d.cache_dir().to_path_buf())
            .unwrap_or_else(|| {
                path.parent()
                    .unwrap_or(Path::new("."))
                    .join(".tunerift-cache")
            });
        Self::new(base.join(digest(absolute.as_os_str().as_encoded_bytes())))
    }
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::SeqCst)
    }
    pub fn read(&self, key: &CacheKey, query: &str) -> Option<CachedDirectory> {
        self.read_at(key, query, now())
    }
    fn read_at(&self, key: &CacheKey, query: &str, time: u64) -> Option<CachedDirectory> {
        let _guard = self.gate.lock().ok()?;
        let file = fs::File::open(self.root.join(format!("{}.json", key.0))).ok()?;
        if file.metadata().ok()?.len() > self.max_bytes {
            return None;
        }
        let mut bytes = vec![];
        file.take(self.max_bytes + 1).read_to_end(&mut bytes).ok()?;
        if bytes.len() as u64 > self.max_bytes {
            return None;
        }
        let entry: Entry = serde_json::from_slice(&bytes).ok()?;
        if entry.version != 1 || entry.key != key.0 || entry.saved_at > time.saturating_add(300) {
            return None;
        }
        Some(CachedDirectory {
            fresh: time.saturating_sub(entry.saved_at) < ttl(query),
            saved_at: entry.saved_at,
            stations: entry.stations,
        })
    }
    pub fn write(&self, key: &CacheKey, stations: &[Station], epoch: u64) -> Result<bool> {
        self.write_at(key, stations, epoch, now())
    }
    pub(crate) fn write_at(
        &self,
        key: &CacheKey,
        stations: &[Station],
        epoch: u64,
        time: u64,
    ) -> Result<bool> {
        let _guard = self.gate.lock().unwrap();
        if self.epoch() != epoch {
            return Ok(false);
        }
        let bytes = serde_json::to_vec(&Entry {
            version: 1,
            key: key.0.clone(),
            saved_at: time,
            stations: stations.to_vec(),
        })?;
        if bytes.len() as u64 > self.max_bytes {
            return Ok(false);
        }
        fs::create_dir_all(&self.root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))?;
        }
        let temporary = self.root.join(format!(
            "{}.{}.{}.tmp",
            key.0,
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| -> Result<()> {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, self.root.join(format!("{}.json", key.0)))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
        self.prune(&self.root.join(format!("{}.json", key.0)))?;
        Ok(true)
    }
    fn files(&self) -> Result<Vec<(PathBuf, u64, SystemTime)>> {
        if !self.root.exists() {
            return Ok(vec![]);
        }
        let mut files = vec![];
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name
                .strip_suffix(".json")
                .is_some_and(|stem| stem.len() == 64 && stem.bytes().all(|b| b.is_ascii_hexdigit()))
            {
                let metadata = entry.metadata()?;
                files.push((
                    path,
                    metadata.len(),
                    metadata.modified().unwrap_or(UNIX_EPOCH),
                ));
            }
        }
        Ok(files)
    }
    fn prune(&self, keep: &Path) -> Result<()> {
        let mut files = self.files()?;
        files.sort_by_key(|f| f.2);
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        let mut count = files.len();
        for (path, size, _) in files {
            if path == keep {
                continue;
            }
            if total <= self.max_bytes && count <= MAX_ENTRIES {
                break;
            }
            fs::remove_file(path)?;
            total = total.saturating_sub(size);
            count -= 1;
        }
        Ok(())
    }
    pub fn size(&self) -> u64 {
        let _guard = self.gate.lock().unwrap();
        self.files()
            .map(|files| files.iter().map(|f| f.1).sum())
            .unwrap_or(0)
    }
    pub fn clear(&self) -> Result<()> {
        let _guard = self.gate.lock().unwrap();
        self.epoch.fetch_add(1, Ordering::SeqCst);
        for (path, _, _) in self.files()? {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn station() -> Station {
        Station {
            id: "one".into(),
            name: "中国之声".into(),
            urls: vec!["https://example.com/live".into()],
            ..Default::default()
        }
    }
    #[test]
    fn persists_and_uses_distinct_directory_search_ttls() {
        let d = tempfile::tempdir().unwrap();
        let cache = DirectoryCache::new(d.path().into());
        let key = key(&Config::default(), "").unwrap();
        cache.write_at(&key, &[station()], 0, 1000).unwrap();
        let second = DirectoryCache::new(d.path().into());
        assert!(second.read_at(&key, "", 2000).unwrap().fresh);
        assert!(!second.read_at(&key, "search", 2000).unwrap().fresh);
        assert!(!second.read_at(&key, "", 4600).unwrap().fresh);
    }
    #[test]
    fn keys_isolate_source_query_proxy_and_hide_secrets() {
        let mut c = Config::default();
        let first = key(&c, "one").unwrap().0;
        assert_ne!(first, key(&c, "two").unwrap().0);
        c.source = 1;
        assert_ne!(first, key(&c, "one").unwrap().0);
        c.source = 0;
        c.proxy = "http://user:secret@localhost:1".into();
        let proxy = key(&c, "one").unwrap().0;
        assert_ne!(first, proxy);
        assert!(!proxy.contains("secret"));
        assert_eq!(proxy.len(), 64);
        c.source = 2;
        assert!(key(&c, "").is_none());
    }
    #[test]
    fn corrupt_entries_are_ignored_and_replaced() {
        let d = tempfile::tempdir().unwrap();
        let cache = DirectoryCache::new(d.path().into());
        let key = key(&Config::default(), "").unwrap();
        fs::write(d.path().join(format!("{}.json", key.0)), b"broken").unwrap();
        assert!(cache.read(&key, "").is_none());
        cache.write(&key, &[station()], 0).unwrap();
        assert_eq!(cache.read(&key, "").unwrap().stations[0], station());
    }
    #[test]
    fn cache_clear_prevents_inflight_write_and_preserves_other_files() {
        let d = tempfile::tempdir().unwrap();
        let cache = DirectoryCache::new(d.path().into());
        let key = key(&Config::default(), "").unwrap();
        let epoch = cache.epoch();
        cache.write(&key, &[station()], epoch).unwrap();
        fs::write(d.path().join("config.toml"), "keep").unwrap();
        cache.clear().unwrap();
        assert!(cache.read(&key, "").is_none());
        assert!(!cache.write(&key, &[station()], epoch).unwrap());
        assert!(d.path().join("config.toml").exists());
        assert_eq!(cache.size(), 0);
    }
    #[test]
    fn bounded_cache_evicts_old_entries() {
        let d = tempfile::tempdir().unwrap();
        let mut cache = DirectoryCache::new(d.path().into());
        cache.max_bytes = 600;
        for i in 0..5 {
            let key = key(&Config::default(), &format!("search-{i}")).unwrap();
            cache.write(&key, &[station()], 0).unwrap();
        }
        assert!(cache.size() <= 600);
        assert!(cache.files().unwrap().len() < 5);
    }
    #[test]
    fn local_sources_bypass_cache() {
        let c = Config {
            source: 4,
            subscriptions: vec![crate::config::Subscription {
                name: "local".into(),
                location: "/tmp/stations.m3u".into(),
            }],
            ..Default::default()
        };
        assert!(key(&c, "").is_none());
    }
}
