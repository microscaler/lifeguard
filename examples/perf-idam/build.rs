//! Entity registry for lifeguard-migrate discovery (same pattern as `examples/entities`).

use lifeguard_migrate::build_script;
use std::env;
use std::path::Path;

fn main() {
    let source_dir = Path::new("src");
    println!("cargo:rerun-if-changed=src");

    let entities = match build_script::discover_entities(source_dir) {
        Ok(entities) => entities,
        Err(e) => {
            println!("cargo:warning=discover_entities failed: {e}");
            Vec::new()
        }
    };

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR must be set");
    let registry_path = Path::new(&out_dir).join("entity_registry.rs");
    build_script::generate_registry_module(&entities, &registry_path)
        .unwrap_or_else(|e| panic!("generate_registry_module: {e}"));
}
