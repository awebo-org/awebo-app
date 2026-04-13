use std::{
    env,
    path::{Path, PathBuf},
};

/// Resolves a relative resource path to the correct location,
/// whether running from a .app bundle or via `cargo run`.
pub fn resource_path(rel: impl AsRef<Path>) -> PathBuf {
    let exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    if let Some(macos_dir) = exe.parent() {
        if macos_dir.ends_with("MacOS") {
            if let Some(contents_dir) = macos_dir.parent() {
                let res = contents_dir.join("Resources");
                if res.exists() {
                    return res.join(rel.as_ref());
                }
            }
        }
    }
    PathBuf::from(rel.as_ref())
}
