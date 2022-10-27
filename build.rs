use std::{env, fs::File, io::Write, path::PathBuf};
use walkdir::WalkDir;

pub fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let crates_path = env::var("CARGO_MANIFEST_DIR").unwrap().replace('\\', "/");
    let mut content = String::new();

    content += r#"
        struct ResourceContent {
            map: pi_hash::XHashMap<String, Vec<u8>>,
        }
    
        impl Default for ResourceContent {
            fn default() -> Self {
                let mut map = pi_hash::XHashMap::default();
        "#;

    for entry in WalkDir::new(crates_path.clone() + "/resources") {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            let path = entry.path().to_str().unwrap();
            let path = path.replace('\\', "/");

            let relative_path = path.strip_prefix(crates_path.as_str()).unwrap();

            content += format!(
                "map.insert(\"{}\".to_string(), include_bytes!(\"{}\").to_vec());\n",
                relative_path, path
            )
            .as_str();
        }
    }

    content += r#"
            Self { map }
            }
        }"#;

    let dest = PathBuf::from(&env::var("OUT_DIR").unwrap());

    println!("@@@@@@@@@@@@@@@ build, dst_path = {:?}", dest);
    let mut file = File::create(&dest.join("resource_bindings.rs")).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file.flush().unwrap();
}
