use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=OTVI_EMBED_FRONTEND");
    println!("cargo:rerun-if-env-changed=OTVI_EMBED_FRONTEND_DIR");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));
    let output_path = out_dir.join("embedded_frontend_assets.rs");

    if !embed_frontend_enabled() {
        write_assets(&output_path, &[]);
        return;
    }

    let dist_dir = frontend_dist_dir();
    if !dist_dir.exists() {
        panic!(
            "frontend embedding requested but dist directory is missing: {}",
            dist_dir.display()
        );
    }

    println!("cargo:rerun-if-changed={}", dist_dir.display());

    let mut assets = Vec::new();
    collect_assets(&dist_dir, &dist_dir, &mut assets);
    assets.sort_by(|left, right| left.0.cmp(&right.0));

    write_assets(&output_path, &assets);
}

fn embed_frontend_enabled() -> bool {
    matches!(
        env::var("OTVI_EMBED_FRONTEND")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn frontend_dist_dir() -> PathBuf {
    if let Ok(dir) = env::var("OTVI_EMBED_FRONTEND_DIR") {
        return PathBuf::from(dir);
    }

    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"))
        .join("../../dist")
}

fn collect_assets(root: &Path, current: &Path, assets: &mut Vec<(String, PathBuf)>) {
    let entries = fs::read_dir(current)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", current.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "failed to read directory entry in {}: {error}",
                current.display()
            )
        });
        let path = entry.path();

        if path.is_dir() {
            collect_assets(root, &path, assets);
            continue;
        }

        if !path.is_file() {
            continue;
        }

        let canonical_path = path
            .canonicalize()
            .unwrap_or_else(|error| panic!("failed to canonicalize {}: {error}", path.display()));
        let relative_path = path
            .strip_prefix(root)
            .unwrap_or_else(|error| {
                panic!(
                    "failed to strip prefix {} from {}: {error}",
                    root.display(),
                    path.display()
                )
            })
            .to_string_lossy()
            .replace('\\', "/");

        println!("cargo:rerun-if-changed={}", canonical_path.display());
        assets.push((format!("/{relative_path}"), canonical_path));
    }
}

fn write_assets(output_path: &Path, assets: &[(String, PathBuf)]) {
    let mut source = String::from("pub static EMBEDDED_ASSETS: &[(&str, &[u8])] = &[\n");

    for (route, path) in assets {
        source.push_str(&format!(
            "    ({route:?}, include_bytes!(r#\"{}\"#) as &[u8]),\n",
            path.display()
        ));
    }

    source.push_str("];\n");

    fs::write(output_path, source)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", output_path.display()));
}
