//! Public, unauthenticated release checks; no development-time updater invocation.
use crate::{VERSION, digest, platform};
use anyhow::{Context, Result, bail, ensure};
use semver::Version;
use serde::Deserialize;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

const API: &str = "https://api.github.com/repos/siarheisinelnikau/folder-icons/releases/latest";
const ASSETS: &str = "https://github.com/siarheisinelnikau/folder-icons/releases/download/";
const MAX_DOWNLOAD: u64 = 150 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
    #[serde(default)]
    pub digest: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub draft: bool,
    pub prerelease: bool,
    pub assets: Vec<Asset>,
}
#[derive(Clone, Debug)]
pub struct Available {
    pub version: String,
    pub asset: Asset,
    pub checksum: Option<Asset>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(90)))
        .timeout_connect(Some(Duration::from_secs(10)))
        .build()
        .into()
}
fn get(url: &str, max: u64) -> Result<Vec<u8>> {
    let mut response = agent()
        .get(url)
        .header(
            "User-Agent",
            concat!("FolderIcons/", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "application/vnd.github+json")
        .call()?;
    let mut data = Vec::new();
    response
        .body_mut()
        .with_config()
        .limit(max)
        .reader()
        .read_to_end(&mut data)?;
    Ok(data)
}

pub fn select(release: Release, installed: &str) -> Result<Option<Available>> {
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let version = Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name),
    )?;
    if !version.pre.is_empty() || version <= Version::parse(installed)? {
        return Ok(None);
    }
    ensure!(
        cfg!(all(windows, target_arch = "x86_64")),
        "Automatic installation supports Windows x64 only"
    );
    let name = format!("FolderIcons-v{version}.exe");
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == name)
        .cloned()
        .context("New release has no compatible Windows x64 portable asset")?;
    validate_asset(&asset)?;
    let checksum = release
        .assets
        .iter()
        .find(|a| a.name == format!("{name}.sha256"))
        .cloned();
    if let Some(c) = &checksum {
        validate_asset(c)?;
    }
    ensure!(
        asset
            .digest
            .as_deref()
            .is_some_and(|x| x.starts_with("sha256:"))
            || checksum.is_some(),
        "Release has no SHA-256 checksum; installation unavailable"
    );
    Ok(Some(Available {
        version: version.to_string(),
        asset,
        checksum,
    }))
}
fn validate_asset(asset: &Asset) -> Result<()> {
    let suffix = asset
        .browser_download_url
        .strip_prefix(ASSETS)
        .context("Release asset URL is outside the canonical repository")?;
    ensure!(
        !suffix.contains(['?', '#'])
            && suffix.split('/').count() == 2
            && suffix.ends_with(&format!("/{}", asset.name)),
        "Invalid release asset URL"
    );
    ensure!(
        asset.size > 0 && asset.size <= MAX_DOWNLOAD,
        "Release asset is empty or too large"
    );
    Ok(())
}
pub fn check() -> Result<Option<Available>> {
    let data=get(API,2*1024*1024).context("Release check unavailable (repository/release may not be published, network unavailable, or GitHub rate limit reached)")?;
    select(serde_json::from_slice(&data)?, VERSION)
}

pub fn checksum_text(bytes: &[u8], name: &str) -> Result<String> {
    let text = std::str::from_utf8(bytes)?;
    let mut parts = text.split_whitespace();
    let hash = parts.next().context("Empty checksum file")?;
    ensure!(
        hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "Invalid SHA-256 checksum"
    );
    if let Some(file) = parts.next() {
        ensure!(
            file.trim_start_matches('*') == name,
            "Checksum names a different asset"
        );
    }
    ensure!(parts.next().is_none(), "Ambiguous checksum file");
    Ok(hash.to_lowercase())
}
pub fn validate_pe(bytes: &[u8]) -> Result<()> {
    ensure!(
        bytes.len() > 64 && bytes.starts_with(b"MZ"),
        "Downloaded asset is not a Windows executable"
    );
    let offset = u32::from_le_bytes(bytes[60..64].try_into()?) as usize;
    ensure!(
        offset <= bytes.len().saturating_sub(6)
            && bytes[offset..offset + 4] == *b"PE\0\0"
            && bytes[offset + 4..offset + 6] == [0x64, 0x86],
        "Downloaded executable is not Windows x64"
    );
    Ok(())
}

/// Download and verify into the executable's directory; existing installation is untouched.
pub fn download(update: &Available, mut progress: impl FnMut(u64, u64)) -> Result<PathBuf> {
    validate_asset(&update.asset)?;
    let expected = if let Some(d) = &update.asset.digest {
        d.strip_prefix("sha256:")
            .context("Unsupported asset digest")?
            .to_lowercase()
    } else {
        let c = update.checksum.as_ref().context("No checksum")?;
        validate_asset(c)?;
        checksum_text(&get(&c.browser_download_url, 4096)?, &update.asset.name)?
    };
    ensure!(
        expected.len() == 64 && expected.bytes().all(|x| x.is_ascii_hexdigit()),
        "Invalid expected digest"
    );
    let exe = std::env::current_exe()?;
    let stage = exe
        .parent()
        .context("No executable directory")?
        .join(format!(".FolderIcons-update-{}.exe", update.version));
    let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&stage).context("Cannot stage update next to this executable (check permissions or an earlier staged update)")?;
    let result = (|| -> Result<()> {
        let mut response = agent()
            .get(&update.asset.browser_download_url)
            .header("User-Agent", "FolderIcons")
            .call()?;
        let mut reader = response
            .body_mut()
            .with_config()
            .limit(MAX_DOWNLOAD)
            .reader();
        let mut buffer = [0; 65536];
        let mut total = 0;
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            total += count as u64;
            ensure!(
                total <= update.asset.size,
                "Download exceeds advertised size"
            );
            file.write_all(&buffer[..count])?;
            progress(total, update.asset.size);
        }
        ensure!(total == update.asset.size, "Incomplete download");
        file.sync_all()?;
        drop(file);
        let bytes = fs::read(&stage)?;
        ensure!(digest(&bytes) == expected, "SHA-256 verification failed");
        validate_pe(&bytes)?;
        if let Some(c) = &update.checksum {
            let advertised =
                checksum_text(&get(&c.browser_download_url, 4096)?, &update.asset.name)?;
            ensure!(advertised == expected, "Release checksums disagree");
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&stage);
    }
    result?;
    Ok(stage)
}

/// Spawn a copy of this executable as a helper, then let the UI close normally.
pub fn start_install(stage: &Path) -> Result<()> {
    platform::require_windows()?;
    let exe = std::env::current_exe()?;
    ensure!(
        stage.parent() == exe.parent(),
        "Update must be beside the installed executable"
    );
    let bytes = fs::read(stage)?;
    validate_pe(&bytes)?;
    let helper = exe
        .parent()
        .context("No executable directory")?
        .join(format!(".FolderIcons-helper-{}.exe", std::process::id()));
    let mut helper_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&helper)?;
    helper_file.write_all(&fs::read(&exe)?)?;
    helper_file.sync_all()?;
    drop(helper_file);
    let ready = helper.with_extension("ready");
    ensure!(
        !ready.exists(),
        "An earlier update handshake exists; close the app and clean up the stale helper files"
    );
    let mut command = std::process::Command::new(&helper);
    command
        .arg("--finish-update")
        .arg(std::process::id().to_string())
        .arg(&exe)
        .arg(stage)
        .arg(digest(&bytes))
        .arg(&ready);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            let _ = fs::remove_file(helper);
            return Err(e.into());
        }
    };
    // Handshake guarantees that the helper holds the parent process handle before
    // the UI can exit, avoiding a race with OpenProcess on an already exited PID.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if ready.try_exists()? {
            ensure!(
                platform::read_optional(&ready, 32)?.as_deref() == Some(b"ready"),
                "Invalid update handshake"
            );
            fs::remove_file(&ready)?;
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            bail!(
                "Update helper exited before installation ({status}). Check {}",
                helper.with_extension("error.txt").display()
            );
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "Update helper did not become ready within 10 seconds; the installed executable is unchanged"
            );
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}

/// Internal helper mode. Never used by tests. A failed replacement keeps a rollback copy.
pub fn finish_install(args: &[std::ffi::OsString]) -> Result<()> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, WAIT_OBJECT_0},
            System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
        };
        ensure!(args.len() == 5, "Invalid update-helper arguments");
        let pid = args[0].to_str().context("Invalid PID")?.parse::<u32>()?;
        let exe = PathBuf::from(&args[1]);
        let stage = PathBuf::from(&args[2]);
        let ready = PathBuf::from(&args[4]);
        let helper = std::env::current_exe()?;
        ensure!(
            ready == helper.with_extension("ready") && ready.parent() == exe.parent(),
            "Invalid helper handshake path"
        );
        ensure!(
            exe.is_absolute()
                && stage.parent() == exe.parent()
                && stage
                    .file_name()
                    .and_then(|x| x.to_str())
                    .is_some_and(|n| n.starts_with(".FolderIcons-update-") && n.ends_with(".exe")),
            "Invalid update paths"
        );
        platform::safe_path(&exe)?;
        platform::safe_path(&stage)?;
        let bytes = fs::read(&stage)?;
        validate_pe(&bytes)?;
        ensure!(
            digest(&bytes) == args[3].to_str().context("Invalid checksum")?,
            "Staged update checksum changed"
        );
        // SAFETY: the process handle is used only for waiting and closed on every path.
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        if !handle.is_null() {
            if let Err(error) = (|| -> Result<()> {
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&ready)?;
                file.write_all(b"ready")?;
                file.sync_all()?;
                Ok(())
            })() {
                unsafe {
                    CloseHandle(handle);
                }
                return Err(error);
            }
            let wait = unsafe { WaitForSingleObject(handle, 30000) };
            unsafe {
                CloseHandle(handle);
            }
            ensure!(
                wait == WAIT_OBJECT_0,
                "Application did not exit within 30 seconds"
            );
        } else {
            bail!("Cannot wait for the running application; installation cancelled");
        }
        let backup = exe.with_extension(format!("exe.previous-{VERSION}"));
        ensure!(
            !backup.exists(),
            "A previous rollback executable already exists"
        );
        fs::rename(&exe, &backup).context("Cannot move installed executable to rollback copy")?;
        if let Err(e) = fs::rename(&stage, &exe) {
            let rollback = fs::rename(&backup, &exe);
            bail!("Update replacement failed: {e}. Rollback: {rollback:?}");
        }
        let mut command = std::process::Command::new(&exe);
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        command
            .spawn()
            .context("Update installed, but relaunch failed. Start the executable manually")?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = args;
        bail!("Updater is Windows-only")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(version: &str) -> Release {
        let name = format!("FolderIcons-v{version}.exe");
        Release {
            tag_name: format!("v{version}"),
            draft: false,
            prerelease: false,
            assets: vec![Asset {
                name: name.clone(),
                browser_download_url: format!("{ASSETS}v{version}/{name}"),
                size: 100,
                digest: Some(format!("sha256:{}", "a".repeat(64))),
            }],
        }
    }
    #[test]
    fn stable_numeric_versions_and_missing_assets() {
        assert!(select(release("0.1.0"), "0.1.0").unwrap().is_none());
        assert!(select(release("0.0.9"), "0.1.0").unwrap().is_none());
        let mut r = release("0.2.0");
        r.prerelease = true;
        assert!(select(r, "0.1.0").unwrap().is_none());
        #[cfg(all(windows, target_arch = "x86_64"))]
        {
            assert_eq!(
                select(release("0.10.0"), "0.9.0").unwrap().unwrap().version,
                "0.10.0"
            );
            let mut r = release("0.2.0");
            r.assets.clear();
            assert!(select(r, "0.1.0").is_err());
        }
    }
    #[test]
    fn rejects_untrusted_assets_and_corrupt_payloads() {
        let mut a = release("0.2.0").assets.remove(0);
        a.browser_download_url = "https://example.com/bad.exe".into();
        assert!(validate_asset(&a).is_err());
        assert!(validate_pe(b"not an exe").is_err());
        assert!(
            checksum_text(format!("{} other.exe", "b".repeat(64)).as_bytes(), "a.exe").is_err()
        );
        assert_eq!(
            checksum_text("a".repeat(64).as_bytes(), "a.exe").unwrap(),
            "a".repeat(64)
        );
    }
}
