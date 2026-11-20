use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let releases = manifest.join("..").join("docs").join("releases");
    println!("cargo:rerun-if-changed={}", releases.display());
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    if let Ok(read) = fs::read_dir(&releases) {
        for entry in read.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let version = name.strip_prefix('v').and_then(|n| n.strip_suffix(".md"));
            if let Some(version) = version {
                println!("cargo:rerun-if-changed={}", path.display());
                entries.push((version.to_string(), path));
            }
        }
    }
    entries.sort();
    let mut out = String::from("pub const EMBEDDED: &[(&str, &str)] = &[\n");
    for (version, path) in entries {
        out.push_str(&format!(
            "    ({:?}, include_str!({:?})),\n",
            version,
            path.canonicalize().unwrap_or(path).display().to_string()
        ));
    }
    out.push_str("];\n");
    let target = PathBuf::from(env::var("OUT_DIR").unwrap()).join("embedded_releases.rs");
    fs::write(target, out).unwrap();
}
