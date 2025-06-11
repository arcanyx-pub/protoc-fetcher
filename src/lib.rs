//! Download official protobuf compiler (protoc) releases with a single command, pegged to the
//! version of your choice.

use anyhow::bail;
use reqwest::StatusCode;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;

/// Downloads an official [release] of the protobuf compiler (protoc) and returns the path to it.
///
/// The release archive matching the given `version` will be downloaded, and the protoc binary will
/// be extracted into a subdirectory of `out_dir`. You can choose a `version` from the
/// [release] page, for example "31.1". Don't prefix it with a "v".
///
/// `out_dir` can be anywhere you want, but if calling this function from a build script, you should
/// probably use the `OUT_DIR` env var (which is set by Cargo automatically for build scripts).
///
/// A previously downloaded protoc binary of the correct version will be reused if already present
/// in `out_dir`.
///
/// # Examples:
///
/// ```no_run
/// # use std::env;
/// # use std::path::Path;
/// // From within build.rs...
/// let out_dir = env::var("OUT_DIR").unwrap();
/// let protoc_path = protoc_fetcher::protoc("31.1", Path::new(&out_dir));
/// ```
///
/// If you are using [tonic-build] (or [prost-build]), you can instruct it to use the fetched
/// `protoc` binary by setting the `PROTOC` env var.
///
/// ```no_run
/// # use std::env;
/// # use std::path::Path;
/// # let out_dir = env::var("OUT_DIR").unwrap();
/// # let path_to_my_protos = Path::new("a/b/c");
/// # let protoc_path = protoc_fetcher::protoc("31.1", Path::new(&out_dir)).unwrap();
/// env::set_var("PROTOC", &protoc_path);
/// tonic_build::compile_protos(path_to_my_protos);
/// ```
///
/// [release]: https://github.com/protocolbuffers/protobuf/releases
/// [tonic-build]: https://crates.io/crates/tonic-build
/// [prost-build]: https://crates.io/crates/prost-build
pub fn protoc(version: &str, out_dir: &Path) -> anyhow::Result<PathBuf> {
    let protoc_path = ensure_protoc_installed(version, out_dir)?;

    Ok(protoc_path)
}

/// Checks for an existing protoc of the given version; if not found, then the official protoc
/// release is downloaded and "installed", i.e., the binary is copied from the release archive
/// into the `generated` directory.
fn ensure_protoc_installed(version: &str, install_dir: &Path) -> anyhow::Result<PathBuf> {
    let release_name = get_protoc_release_name(version);

    let protoc_dir = install_dir.join(format!("protoc-fetcher/{release_name}"));
    let protoc_path = protoc_dir.join("bin/protoc");
    if protoc_path.exists() {
        println!("protoc with correct version is already installed.");
    } else {
        println!("protoc v{version} not found, downloading...");
        download_protoc(&protoc_dir, &release_name, version)?;
    }
    println!(
        "`protoc --version`: {}",
        get_protoc_version(&protoc_path).unwrap()
    );

    Ok(protoc_path)
}

fn download_protoc(protoc_dir: &Path, release_name: &str, version: &str) -> anyhow::Result<()> {
    let archive_url = protoc_release_archive_url(release_name, version);
    let response = reqwest::blocking::get(archive_url)?;
    if response.status() != StatusCode::OK {
        bail!(
            "Error downloading release archive: {} {}",
            response.status(),
            response.text().unwrap_or_default()
        );
    }
    println!("Download successful.");

    fs::create_dir_all(protoc_dir)?;
    let cursor = Cursor::new(response.bytes()?);
    zip_extract::extract(cursor, protoc_dir, false)?;
    println!("Extracted archive.");

    #[cfg(unix)]
    let protoc_path = protoc_dir.join("bin/protoc");

    #[cfg(windows)]
    let protoc_path = protoc_dir.join("bin/protoc.exe");

    if !protoc_path.exists() {
        bail!("Extracted protoc archive, but could not find bin/protoc!");
    }

    println!("protoc installed successfully: {:?}", &protoc_path);
    Ok(())
}

fn protoc_release_archive_url(release_name: &str, version: &str) -> String {
    let archive_url =
        format!("https://github.com/protocolbuffers/protobuf/releases/download/v{version}/{release_name}.zip");
    println!("Release URL: {archive_url}");

    archive_url
}

fn get_protoc_release_name(version: &str) -> String {
    // Adjust values to match the protoc release names. Examples:
    //   - linux 64-bit: protoc-21.2-linux-x86_64.zip
    //   - macos ARM: protoc-21.2-osx-aarch_64.zip
    //   - windows 32-bit: protoc-21.2-win32.zip

    #[allow(unused)]
    let name = "";

    #[cfg(all(target_os = "linux", target_arch="aarch64"))]
    let name = "linux-aarch_64";

    #[cfg(all(target_os = "linux", target_arch="x86"))]
    let name = "linux-x86_32";

    #[cfg(all(target_os = "linux", target_arch="x86_64"))]
    let name = "linux-x86_64";

    #[cfg(all(target_os = "macos", target_arch="aarch64"))]
    let name = "osx-aarch_64";

    #[cfg(all(target_os = "macos", target_arch="x86_64"))]
    let name = "osx-x86_64";

    #[cfg(all(target_os = "macos", not(target_arch="aarch64"), not(target_arch="x86_64")))]
    let name = "osx-universal_binary";

    #[cfg(all(windows, target_pointer_width = "32"))]
    let name = "win32";

    #[cfg(all(windows, target_pointer_width = "64"))]
    let name = "win64";

    if name == "" {
        panic!("`protoc` unsupported platform");
    }

    println!("Detected: {}", name);

    format!("protoc-{version}-{name}")
}

fn get_protoc_version(protoc_path: &Path) -> anyhow::Result<String> {
    let version = String::from_utf8(Command::new(&protoc_path).arg("--version").output()?.stdout)?;
    Ok(version)
}
