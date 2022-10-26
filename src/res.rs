use pathfinder_resources::ResourceLoader;
use pi_hash::XHashMap;

pub struct MemResourceLoader {
    map: XHashMap<String, Vec<u8>>,
}

impl Default for MemResourceLoader {
    fn default() -> Self {
        let map = Default::default();

        Self { map }
    }
}

impl ResourceLoader for MemResourceLoader {
    fn slurp(&self, virtual_path: &str) -> Result<Vec<u8>, std::io::Error> {
        match self.map.get(virtual_path) {
            Some(data) => Ok(data.clone()),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("pi_svg resource isn't find, path = {}", virtual_path),
            )),
        }
    }
}
