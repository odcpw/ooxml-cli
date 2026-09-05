//! Portable package metadata checks; no tags, uploads, or package publication.
use serde_json::Value;
use std::path::Path;
use std::process::Command;

#[test]
fn cargo_package_and_binary_versions_agree_and_sources_are_packaged() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let metadata = Command::new(&cargo)
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        metadata.status.success(),
        "{}",
        String::from_utf8_lossy(&metadata.stderr)
    );
    let metadata: Value = serde_json::from_slice(&metadata.stdout).unwrap();
    let package = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "ooxml-cli")
        .unwrap();
    assert_eq!(package["version"], env!("CARGO_PKG_VERSION"));
    let binary = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(["--json", "version"])
        .output()
        .unwrap();
    assert!(binary.status.success());
    let version: Value = serde_json::from_slice(&binary.stdout).unwrap();
    assert_eq!(version["version"], package["version"]);
    let listing = Command::new(cargo)
        .args(["package", "--list", "--locked", "--allow-dirty"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        listing.status.success(),
        "{}",
        String::from_utf8_lossy(&listing.stderr)
    );
    let listing = String::from_utf8(listing.stdout)
        .unwrap()
        .replace('\\', "/");
    for required in ["Cargo.toml", "Cargo.lock", "src/main.rs", "src/lib.rs"] {
        assert!(
            listing.lines().any(|line| line == required),
            "package missing {required}"
        );
    }
}
