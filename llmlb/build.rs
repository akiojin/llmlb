use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

fn main() {
    generate_dashboard_assets_build_stamp();

    // Windows: embed icon into the executable
    if env::var("CARGO_CFG_TARGET_OS")
        .map(|os| os == "windows")
        .unwrap_or(false)
    {
        println!("cargo:rerun-if-changed=../assets/icons/load balancer.ico");

        let mut res = winresource::WindowsResource::new();
        res.set_icon("../assets/icons/load balancer.ico");
        res.compile()
            .expect("failed to embed load balancer Windows resources");
    }
}

fn generate_dashboard_assets_build_stamp() {
    let static_dir = Path::new("src/web/static");
    let mut files = Vec::<PathBuf>::new();
    collect_files(static_dir, &mut files);
    files.sort();

    let mut hasher = DefaultHasher::new();
    for path in &files {
        // Ensure Cargo rebuilds when embedded assets change.
        println!("cargo:rerun-if-changed={}", path.display());

        path.to_string_lossy().hash(&mut hasher);
        if let Ok(meta) = fs::metadata(path) {
            meta.len().hash(&mut hasher);
            if let Ok(modified) = meta.modified() {
                if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                    duration.as_secs().hash(&mut hasher);
                    duration.subsec_nanos().hash(&mut hasher);
                }
            }
        }
    }

    let stamp = format!("{:016x}\n", hasher.finish());
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest = Path::new(&out_dir).join("dashboard_assets_build_stamp.txt");
    fs::write(dest, stamp).expect("failed to write dashboard assets build stamp");
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files);
        } else {
            files.push(path);
        }
    }
}
