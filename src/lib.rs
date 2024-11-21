//! Download official protobuf compiler (protoc) releases with a single command, pegged to the
//! version of your choice.

use anyhow::{bail, Context};
use reqwest::StatusCode;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

/// Downloads an official [release] of the protobuf compiler (protoc) and returns the path to it.
///
/// The release archive matching the given `version` will be downloaded, and the protoc binary will
/// be extracted into a subdirectory of `out_dir`. You can choose a `version` from the
/// [release] page, for example "21.2". Don't prefix it with a "v".
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
/// let protoc_path = protoc_fetcher::protoc("21.2", Path::new(&out_dir));
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
/// # let protoc_path = protoc_fetcher::protoc("21.2", Path::new(&out_dir)).unwrap();
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
    let response = reqwest::blocking::get(&archive_url)
        .with_context(|| format!("Failed to download archive from {}", archive_url))?;
    if response.status() != StatusCode::OK {
        bail!(
            "Error downloading release archive: {} {}",
            response.status(),
            response.text().unwrap_or_default()
        );
    }
    println!("Download successful.");

    fs::create_dir_all(protoc_dir)
        .with_context(|| format!("Failed to create dir: {:?}", protoc_dir))?;
    let cursor = Cursor::new(response.bytes()?);
    zip_extract::extract(cursor, protoc_dir, false).with_context(|| {
        format!(
            "Failed to extract archive to {:?} (from {})",
            protoc_dir, archive_url
        )
    })?;
    println!("Extracted archive.");

    let protoc_path = protoc_dir.join("bin/protoc");
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
    let mut platform = env::consts::OS;
    let mut arch = env::consts::ARCH;
    println!("Detected: {}, {}", platform, arch);

    // Adjust values to match the protoc release names. Examples:
    //   - linux 64-bit: protoc-21.2-linux-x86_64.zip
    //   - macos ARM: protoc-21.2-osx-aarch_64.zip
    if platform == "macos" {
        platform = "osx"; // protoc is stuck in the past XD
    }
    if arch == "aarch64" {
        arch = "aarch_64";
    }

    format!("protoc-{version}-{platform}-{arch}")
}

fn get_protoc_version(protoc_path: &Path) -> anyhow::Result<String> {
    let version = String::from_utf8(
        Command::new(&protoc_path)
            .arg("--version")
            .output()
            .with_context(|| format!("Failed to run `{:?} --version`", protoc_path))?
            .stdout,
    )?;
    Ok(version)
}

#[cfg(test)]
mod test {
    use tempfile::tempdir;

    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn test_protoc_runs_without_error() {
        let version = "28.0";
        let temp_dir = tempdir().unwrap();

        let result = protoc(version, temp_dir.path());
        assert!(result.is_ok());
    }

    #[googletest::test]
    // If one crate uses protoc_fetcher to download protoc, and a dependency crate of it also uses protoc_fetcher to the same directory.
    // i.e. both crates are in the same workspace and download to the same workspace or cache directory, then there will be more than one
    // process trying to download protoc to the same directory.
    // This is the test case for that scenario.
    fn test_two_processes_on_same_directory_1s_apart_run_without_error() {
        let version = "28.0";
        let temp_dir = tempdir().unwrap();

        let (tx, rx) = std::sync::mpsc::channel(); // Channel for thread communication

        let handles: Vec<_> = (0..2)
            .map(|i| {
                let version = version.to_string();
                let temp_dir = temp_dir.path().to_path_buf();
                let tx = tx.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(i * 1000));
                    let result = protoc(&version, &temp_dir);
                    tx.send((i, result)).expect("Failed to send result");
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        let mut results = rx.iter().take(2).collect::<Vec<_>>();
        results.sort_by_key(|&(i, _)| i);
        let result_from_first = &results[0].1;
        let result_from_second = &results[1].1;

        verify_that!(result_from_first, ok(anything())).and_log_failure();
        verify_that!(result_from_second, ok(anything())).and_log_failure();
    }
}
