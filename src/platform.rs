use anyhow::{Context, Result, bail, ensure};
use std::{fs, io::Write, path::Path};

pub const READONLY: u32 = 1;
pub const HIDDEN: u32 = 2;
pub const SYSTEM: u32 = 4;

#[cfg(windows)]
pub fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

pub fn attributes(path: &Path) -> Result<u32> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        Ok(fs::symlink_metadata(path)?.file_attributes())
    }
    #[cfg(not(windows))]
    {
        let _ = fs::metadata(path)?;
        Ok(0)
    }
}
pub fn set_attributes(path: &Path, attrs: u32) -> Result<()> {
    #[cfg(windows)]
    {
        // Only attributes accepted by SetFileAttributes, never DIRECTORY or REPARSE_POINT.
        let writable = attrs & (1 | 2 | 4 | 32 | 256 | 4096 | 8192);
        let flags = if writable == 0 { 128 } else { writable };
        // SAFETY: nul-terminated path lives through the call; flags contain only valid bits.
        ensure!(
            unsafe {
                windows_sys::Win32::Storage::FileSystem::SetFileAttributesW(
                    wide(path).as_ptr(),
                    flags,
                )
            } != 0,
            "Cannot set attributes for {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        );
    }
    #[cfg(not(windows))]
    {
        let _ = (path, attrs);
    }
    Ok(())
}

/// Reject junctions/symlinks throughout the path, not just the final directory.
pub fn safe_path(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "Symbolic links are not supported: {}",
            ancestor.display()
        );
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            ensure!(
                metadata.file_attributes() & 0x400 == 0,
                "Reparse points (including cloud placeholders) are not supported: {}",
                ancestor.display()
            );
        }
    }
    Ok(())
}
pub fn read_optional(path: &Path, max: u64) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            safe_path(path)?;
            ensure!(
                meta.is_file() && meta.len() <= max,
                "Not a regular file or too large: {}",
                path.display()
            );
            use std::io::Read;
            let mut bytes = Vec::new();
            fs::File::open(path)?
                .take(max + 1)
                .read_to_end(&mut bytes)?;
            ensure!(bytes.len() as u64 <= max, "File exceeded size limit");
            Ok(Some(bytes))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Missing parent directory"))?;
    safe_path(parent)?;
    let temp = parent.join(format!(".FolderIcons-{}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)
            .with_context(|| format!("Cannot atomically replace {}", path.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Called by the interactive application only, never by the planning/test engine.
pub fn notify_folder(path: &Path) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::{SHCNE_UPDATEITEM, SHCNF_PATHW, SHChangeNotify};
        // SAFETY: PATHW tells the shell that the first pointer is a terminated path.
        unsafe {
            SHChangeNotify(
                SHCNE_UPDATEITEM as i32,
                SHCNF_PATHW,
                wide(path).as_ptr().cast(),
                std::ptr::null(),
            );
        }
    }
    #[cfg(not(windows))]
    let _ = path;
}

pub fn context_menu(enable: bool, label: &str) -> Result<()> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::{Foundation::ERROR_SUCCESS, System::Registry::*};
        let key = wide(Path::new(
            "Software\\Classes\\Directory\\shell\\FolderIcons",
        ));
        if !enable {
            // SAFETY: HKCU is predefined and the key is a fixed application-owned path.
            let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, key.as_ptr()) };
            ensure!(
                status == ERROR_SUCCESS || status == 2,
                "Could not remove context menu: Windows error {status}"
            );
            return Ok(());
        }
        fn set(sub: &str, name: &str, value: &str) -> Result<()> {
            use windows_sys::Win32::{Foundation::ERROR_SUCCESS, System::Registry::*};
            let mut handle = std::ptr::null_mut();
            let sub = wide(Path::new(sub));
            let name = wide(Path::new(name));
            let value = wide(Path::new(value));
            // SAFETY: pointers are valid and out handle is initialized by the API.
            let status = unsafe {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    sub.as_ptr(),
                    0,
                    std::ptr::null(),
                    REG_OPTION_NON_VOLATILE,
                    KEY_SET_VALUE,
                    std::ptr::null(),
                    &mut handle,
                    std::ptr::null_mut(),
                )
            };
            ensure!(
                status == ERROR_SUCCESS,
                "Could not create context menu: Windows error {status}"
            );
            // SAFETY: value length is in bytes and includes the terminator.
            let status = unsafe {
                RegSetValueExW(
                    handle,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    value.as_ptr().cast(),
                    (value.len() * 2) as u32,
                )
            };
            unsafe {
                RegCloseKey(handle);
            }
            ensure!(
                status == ERROR_SUCCESS,
                "Could not write context menu: Windows error {status}"
            );
            Ok(())
        }
        let exe = std::env::current_exe()?;
        let base = "Software\\Classes\\Directory\\shell\\FolderIcons";
        set(base, "", label)?;
        set(base, "Icon", &format!("\"{}\",0", exe.display()))?;
        set(
            &format!("{base}\\command"),
            "",
            &format!("\"{}\" \"%1\"", exe.display()),
        )?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (enable, label);
        bail!("Explorer integration is Windows-only")
    }
}

pub fn require_windows() -> Result<()> {
    if !cfg!(windows) {
        bail!("Folder customization is supported on Windows only");
    }
    Ok(())
}
