use pathfinder_resources::ResourceLoader;

include!(concat!(env!("OUT_DIR"), "/resource_bindings.rs"));

#[derive(Default)]
pub struct MemResourceLoader {
    content: ResourceContent,
}

impl ResourceLoader for MemResourceLoader {
    fn slurp(&self, virtual_path: &str) -> Result<Vec<u8>, std::io::Error> {
        let path = format!("resources/{}", virtual_path);

        match self.content.map.get(path.as_str()) {
            Some(data) => Ok(data.clone()),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("pi_svg resource isn't find, path = {}", virtual_path),
            )),
        }
    }
}
