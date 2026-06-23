use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AssetServer {
    cache: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl AssetServer {
    pub fn new() -> Self {
        Self { cache: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn load_bytes(&self, path: &str) -> Result<Vec<u8>, String> {
        let mut cache = self.cache.lock().unwrap();
        if let Some(cached) = cache.get(path) {
            return Ok(cached.clone());
        }
        let bytes = std::fs::read(path)
            .map_err(|e| format!("Failed to load {path}: {e}"))?;
        cache.insert(path.to_string(), bytes.clone());
        Ok(bytes)
    }
}

impl Default for AssetServer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_file(name: &str, content: &[u8]) -> String {
        let path = std::env::temp_dir().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content).unwrap();
        path.to_str().unwrap().to_string()
    }

    #[test]
    fn asset_server_loads_bytes() {
        let path = write_temp_file("test_asset.bin", b"hello asset");
        let server = AssetServer::new();
        let bytes = server.load_bytes(&path).expect("Failed to load");
        assert_eq!(bytes, b"hello asset");
    }

    #[test]
    fn asset_server_caches_by_path() {
        let path = write_temp_file("test_cached.bin", b"cached data");
        let server = AssetServer::new();
        let bytes1 = server.load_bytes(&path).unwrap();
        let bytes2 = server.load_bytes(&path).unwrap();
        assert_eq!(bytes1, bytes2);
    }
}
