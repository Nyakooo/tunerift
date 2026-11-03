//! Update only the running executable from a checksum-verified official release.
use crate::config::{Config, validate_proxy};
use anyhow::{Context, Result, bail, ensure};
use reqwest::blocking::{Client, Response};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    time::Duration,
};

const REPOSITORY: &str = "https://github.com/Nyakooo/tunerift";
const LATEST: &str = "https://api.github.com/repos/Nyakooo/tunerift/releases/latest";
const MAX_ARCHIVE: u64 = 128 * 1024 * 1024;
const MAX_BINARY: u64 = 256 * 1024 * 1024;
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
}
fn target(os: &str, arch: &str) -> Result<String> {
    let os = match os {
        "linux" => "linux",
        "macos" => "macos",
        "windows" => "windows",
        _ => bail!("Unsupported operating system: {os}"),
    };
    let arch = match arch {
        "x86_64" => "x86_64",
        "aarch64" => "arm64",
        _ => bail!("Unsupported architecture: {arch}"),
    };
    Ok(format!("{os}-{arch}"))
}
fn release_version(tag: &str) -> Result<semver::Version> {
    let value = tag
        .strip_prefix('v')
        .context("Release tag must start with v")?;
    let version = semver::Version::parse(value).context("Invalid release version")?;
    ensure!(version.pre.is_empty(), "Expected a stable release");
    Ok(version)
}
fn checksum(text: &str, asset: &str) -> Result<String> {
    let hashes: Vec<_> = text
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let hash = words.next()?;
            let name = words.next()?;
            (name == asset && words.next().is_none()).then_some(hash)
        })
        .collect();
    ensure!(
        hashes.len() == 1,
        "Missing or duplicate checksum for {asset}"
    );
    let hash = hashes[0];
    ensure!(
        hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "Invalid SHA256 checksum"
    );
    Ok(hash.to_ascii_lowercase())
}
fn bounded_text(mut response: Response, max: u64) -> Result<String> {
    let mut bytes = vec![];
    response.by_ref().take(max + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= max,
        "Update metadata exceeds size limit"
    );
    String::from_utf8(bytes).context("Update metadata is not UTF-8")
}
fn download(mut source: impl Read, destination: &Path, expected: &str) -> Result<()> {
    let mut file = File::create(destination)?;
    let mut hash = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let size = source.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        total += size as u64;
        ensure!(total <= MAX_ARCHIVE, "Release package exceeds size limit");
        hash.update(&buffer[..size]);
        file.write_all(&buffer[..size])?;
    }
    ensure!(
        format!("{:x}", hash.finalize()) == expected,
        "SHA256 mismatch; executable was not changed"
    );
    file.sync_all()?;
    Ok(())
}
fn copy_binary(mut reader: impl Read, size: u64, output: &Path) -> Result<()> {
    ensure!(size > 0 && size <= MAX_BINARY, "Invalid executable size");
    let mut file = File::create(output)?;
    let copied = std::io::copy(&mut reader.by_ref().take(MAX_BINARY + 1), &mut file)?;
    ensure!(copied == size, "Executable size mismatch");
    file.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}
#[cfg(not(windows))]
fn extract(package: &Path, member: &str, output: &Path) -> Result<()> {
    let decoder = flate2::read::GzDecoder::new(File::open(package)?);
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries()? {
        let entry = entry?;
        if entry.path()?.as_ref() == Path::new(member) {
            ensure!(
                entry.header().entry_type().is_file(),
                "Executable must be a regular file"
            );
            let size = entry.header().size()?;
            return copy_binary(entry, size, output);
        }
    }
    bail!("Release package does not contain the expected executable")
}
#[cfg(windows)]
fn extract(package: &Path, member: &str, output: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(File::open(package)?)?;
    let entry = archive
        .by_name(member)
        .context("Release package does not contain the expected executable")?;
    ensure!(
        entry.is_file() && !entry.is_symlink(),
        "Executable must be a regular file"
    );
    let size = entry.size();
    copy_binary(entry, size, output)
}

pub fn run(config: &Config) -> Result<()> {
    let language = config.language;
    println!(
        "{}",
        language.text("正在检查更新…", "Checking for updates…")
    );
    validate_proxy(&config.proxy)?;
    let mut builder = Client::builder()
        .no_proxy()
        .https_only(true)
        .user_agent(concat!("Tunerift/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300));
    if !config.proxy.trim().is_empty() {
        builder = builder.proxy(reqwest::Proxy::all(config.proxy.trim())?);
    }
    let client = builder.build()?;
    let release: Release = serde_json::from_str(&bounded_text(
        client
            .get(LATEST)
            .send()?
            .error_for_status()
            .context("Cannot check GitHub releases")?,
        2 * 1024 * 1024,
    )?)?;
    let version = release_version(&release.tag_name)?;
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
    if version <= current {
        println!(
            "{} ({current})",
            language.text("已是最新版本", "Already up to date")
        );
        return Ok(());
    }
    let platform = target(std::env::consts::OS, std::env::consts::ARCH)?;
    let name = format!("tunerift-{}-{platform}", release.tag_name);
    let extension = if cfg!(windows) { "zip" } else { "tar.gz" };
    let asset = format!("{name}.{extension}");
    ensure!(
        release.assets.iter().any(|a| a.name == asset)
            && release.assets.iter().any(|a| a.name == "SHA256SUMS"),
        "Release is missing the package or checksums for {platform}"
    );
    let base = format!("{REPOSITORY}/releases/download/{}", release.tag_name);
    let checksums = bounded_text(
        client
            .get(format!("{base}/SHA256SUMS"))
            .send()?
            .error_for_status()?,
        1024 * 1024,
    )?;
    let expected = checksum(&checksums, &asset)?;
    println!(
        "{} {current} → {version} ({platform})",
        language.text("正在更新", "Updating")
    );
    let temporary = tempfile::tempdir()?;
    let package = temporary.path().join(&asset);
    download(
        client
            .get(format!("{base}/{asset}"))
            .send()?
            .error_for_status()?,
        &package,
        &expected,
    )?;
    let executable = if cfg!(windows) {
        "tunerift.exe"
    } else {
        "tunerift"
    };
    let staged = temporary.path().join(executable);
    extract(&package, &format!("{name}/{executable}"), &staged)?;
    self_replace::self_replace(&staged).context(
        "Cannot replace the executable; check write permission and close other running copies",
    )?;
    println!(
        "{} {version}. {}",
        language.text("已更新至", "Updated to"),
        language.text(
            "配置和收藏保持不变。",
            "Configuration and favorites are unchanged."
        )
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn platform_mapping_and_versions_are_strict() {
        for (os, arch, expected) in [
            ("linux", "x86_64", "linux-x86_64"),
            ("linux", "aarch64", "linux-arm64"),
            ("macos", "x86_64", "macos-x86_64"),
            ("macos", "aarch64", "macos-arm64"),
            ("windows", "x86_64", "windows-x86_64"),
            ("windows", "aarch64", "windows-arm64"),
        ] {
            assert_eq!(target(os, arch).unwrap(), expected);
        }
        assert!(target("linux", "arm").is_err());
        assert!(release_version("v0.2.0").unwrap() > release_version("v0.1.9").unwrap());
        for tag in ["0.2.0", "v../../bad", "v0.2.0-beta.1"] {
            assert!(release_version(tag).is_err());
        }
    }
    #[test]
    fn checksum_requires_one_exact_asset_and_a_valid_hash() {
        let hash = "a".repeat(64);
        assert_eq!(
            checksum(
                &format!("{hash}  package.zip\n{}  other.zip", "b".repeat(64)),
                "package.zip"
            )
            .unwrap(),
            hash
        );
        for content in [
            "bad  package.zip".to_string(),
            format!("{hash}  other.zip"),
            format!("{hash}  package.zip\n{hash}  package.zip"),
        ] {
            assert!(checksum(&content, "package.zip").is_err());
        }
    }
    #[test]
    fn download_verifies_bytes_before_any_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let package = directory.path().join("package");
        let bytes = b"download fixture";
        let hash = format!("{:x}", Sha256::digest(bytes));
        download(bytes.as_slice(), &package, &hash).unwrap();
        assert_eq!(std::fs::read(&package).unwrap(), bytes);
        assert!(download(bytes.as_slice(), &package, &"0".repeat(64)).is_err());
    }
    #[cfg(not(windows))]
    fn archive(path: &Path, member: &str, symlink: bool) {
        let encoder = flate2::write::GzEncoder::new(
            File::create(path).unwrap(),
            flate2::Compression::default(),
        );
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o755);
        if symlink {
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_link_name("/tmp/not-an-executable").unwrap();
            header.set_size(0);
            header.set_cksum();
            builder
                .append_data(&mut header, member, std::io::empty())
                .unwrap();
        } else {
            header.set_size(7);
            header.set_cksum();
            builder
                .append_data(&mut header, member, &b"fixture"[..])
                .unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap();
    }
    #[cfg(windows)]
    fn archive(path: &Path, member: &str, _symlink: bool) {
        let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
        writer
            .start_file(member, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"fixture").unwrap();
        writer.finish().unwrap();
    }
    #[test]
    fn extract_selects_only_the_exact_executable_member() {
        let directory = tempfile::tempdir().unwrap();
        let package = directory.path().join("package");
        let binary = directory.path().join("binary");
        archive(&package, "root/tunerift", false);
        extract(&package, "root/tunerift", &binary).unwrap();
        assert_eq!(std::fs::read(&binary).unwrap(), b"fixture");
        assert!(
            extract(
                &package,
                "other/tunerift",
                &directory.path().join("missing")
            )
            .is_err()
        );
        assert!(copy_binary(std::io::empty(), MAX_BINARY + 1, &binary).is_err());
    }
    #[test]
    #[cfg(not(windows))]
    fn extraction_rejects_symlinks() {
        let directory = tempfile::tempdir().unwrap();
        let package = directory.path().join("package");
        let binary = directory.path().join("binary");
        archive(&package, "root/tunerift", true);
        assert!(extract(&package, "root/tunerift", &binary).is_err());
        assert!(!binary.exists());
    }
}
