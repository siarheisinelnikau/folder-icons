//! Read-only plans, optimistic concurrency checks and per-folder write-ahead recovery.
use crate::{digest, ini, platform};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const STORE: &str = ".foldericons";
const MAX_INI: u64 = 1024 * 1024;
const MAX_MANIFEST: u64 = 48 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Apply,
    Restore,
}
#[derive(Clone, Debug)]
pub struct FolderPlan {
    pub folder: PathBuf,
    pub action: Action,
    pub had_ini: bool,
    before: Option<Vec<u8>>,
    manifest_before: Option<Vec<u8>>,
    folder_attrs: u32,
    ini_attrs: u32,
    after: Option<Vec<u8>>,
    icon: Option<Vec<u8>>,
    icon_name: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Manifest {
    product: String,
    schema: u32,
    original: Option<Vec<u8>>,
    folder_attrs: u32,
    ini_attrs: u32,
    applied: Vec<u8>,
    /// Prior bytes are accepted for recovery if a crash occurred before desktop.ini replacement.
    previous: Option<Vec<u8>>,
    icons: Vec<String>,
    restoring: bool,
}

fn manifest(folder: &Path) -> Result<Option<(Manifest, Vec<u8>)>> {
    let store = folder.join(STORE);
    if !store.try_exists()? {
        return Ok(None);
    }
    platform::safe_path(&store)?;
    ensure!(
        store.is_dir(),
        "Reserved .foldericons path is not a directory"
    );
    let bytes = platform::read_optional(&store.join("manifest.json"), MAX_MANIFEST)?
        .context("Unmanaged .foldericons directory; refusing to overwrite")?;
    let m: Manifest = serde_json::from_slice(&bytes)
        .context("Invalid recovery manifest; no files were overwritten")?;
    ensure!(
        m.product == "FolderIcons" && m.schema == 1,
        "Unknown recovery manifest"
    );
    for icon in &m.icons {
        let stem = icon
            .strip_suffix(".ico")
            .context("Invalid manifest icon filename")?;
        ensure!(
            stem.len() == 64 && stem.bytes().all(|x| x.is_ascii_hexdigit()),
            "Invalid manifest icon filename"
        );
    }
    Ok(Some((m, bytes)))
}

pub fn plan(folder: &Path, action: Action, icon: Option<&[u8]>) -> Result<FolderPlan> {
    ensure!(folder.is_absolute(), "Choose an absolute folder path");
    platform::safe_path(folder)?;
    let folder = fs::canonicalize(folder)?;
    ensure!(
        folder.is_dir() && folder.parent().is_some(),
        "Select a regular folder, not a drive root"
    );
    let ini_path = folder.join("desktop.ini");
    let before = platform::read_optional(&ini_path, MAX_INI)?;
    let folder_attrs = platform::attributes(&folder)?;
    let ini_attrs = if before.is_some() {
        platform::attributes(&ini_path)?
    } else {
        0
    };
    let existing = manifest(&folder)?;
    if let Some((m, _)) = &existing {
        let accepted = before.as_ref() == Some(&m.applied)
            || before == m.previous
            || (m.restoring && before == m.original);
        ensure!(
            accepted,
            "desktop.ini changed outside FolderIcons. Keep the backup and reconcile it manually before retrying"
        );
    }
    let (after, icon, icon_name) = match action {
        Action::Apply => {
            let icon = icon.context("Generate an icon before planning Apply")?;
            ensure!(
                icon.len() >= 6 && icon.len() <= 4 * 1024 * 1024 && icon[..4] == [0, 0, 1, 0],
                "Invalid ICO"
            );
            let name = format!("{}.ico", digest(icon));
            let after = ini::with_icon(
                before.as_deref().unwrap_or_default(),
                &format!("{STORE}\\{name}"),
            )?;
            ensure!(
                after.len() as u64 <= MAX_INI,
                "Unicode desktop.ini would exceed the supported size limit"
            );
            (Some(after), Some(icon.to_vec()), Some(name))
        }
        Action::Restore => {
            let (m, _) = existing
                .as_ref()
                .context("No FolderIcons backup exists in this folder")?;
            (m.original.clone(), None, None)
        }
    };
    Ok(FolderPlan {
        folder,
        action,
        had_ini: before.is_some(),
        before,
        manifest_before: existing.map(|(_, b)| b),
        folder_attrs,
        ini_attrs,
        after,
        icon,
        icon_name,
    })
}

struct Lock(PathBuf);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn execute(plan: &FolderPlan) -> Result<()> {
    platform::safe_path(&plan.folder)?;
    let lock_path = plan.folder.join(".foldericons.lock");
    let lock_file=fs::OpenOptions::new().write(true).create_new(true).open(&lock_path)
        .context("Folder is busy, or a stale .foldericons.lock remains after a crash. Close FolderIcons before removing the stale lock")?;
    let _lock = Lock(lock_path);
    drop(lock_file);
    let ini_path = plan.folder.join("desktop.ini");
    ensure!(
        platform::read_optional(&ini_path, MAX_INI)? == plan.before,
        "desktop.ini changed since the plan. Build a new plan"
    );
    ensure!(
        platform::attributes(&plan.folder)? == plan.folder_attrs,
        "Folder attributes changed since the plan"
    );
    if plan.before.is_some() {
        ensure!(
            platform::attributes(&ini_path)? == plan.ini_attrs,
            "desktop.ini attributes changed since the plan"
        );
    }
    let existing = manifest(&plan.folder)?;
    ensure!(
        existing.as_ref().map(|(_, b)| b) == plan.manifest_before.as_ref(),
        "Backup changed since the plan. Build a new plan"
    );
    let store = plan.folder.join(STORE);
    let mut m = existing.map(|(m, _)| m).unwrap_or_else(|| Manifest {
        product: "FolderIcons".into(),
        schema: 1,
        original: plan.before.clone(),
        folder_attrs: plan.folder_attrs,
        ini_attrs: plan.ini_attrs,
        applied: Vec::new(),
        previous: plan.before.clone(),
        icons: Vec::new(),
        restoring: false,
    });
    if !store.try_exists()? {
        fs::create_dir(&store)?;
    }
    match plan.action {
        Action::Apply => {
            let name = plan.icon_name.as_ref().context("Plan has no icon")?;
            let bytes = plan.icon.as_ref().context("Plan has no ICO bytes")?;
            let target = store.join(name);
            if let Some(existing) = platform::read_optional(&target, 4 * 1024 * 1024)? {
                ensure!(existing == *bytes, "Stored icon was modified");
            } else {
                platform::atomic_write(&target, bytes)?;
            }
            if !m.icons.contains(name) {
                m.icons.push(name.clone());
            }
            m.previous = plan.before.clone();
            m.applied = plan.after.clone().context("Plan has no INI")?;
            m.restoring = false;
            // Persist the original before modifying desktop.ini. Every later failure is recoverable.
            platform::atomic_write(
                &store.join("manifest.json"),
                &serde_json::to_vec_pretty(&m)?,
            )?;
            replace_ini(&ini_path, plan.after.as_deref(), plan.ini_attrs)?;
            platform::set_attributes(
                &ini_path,
                plan.ini_attrs | platform::HIDDEN | platform::SYSTEM,
            )?;
            platform::set_attributes(&plan.folder, plan.folder_attrs | platform::READONLY)?;
            platform::set_attributes(&store, platform::attributes(&store)? | platform::HIDDEN)?;
            // Commit the journal after all appearance writes. After this point an external
            // deletion of desktop.ini must not be mistaken for an interrupted first Apply.
            m.previous = Some(m.applied.clone());
            platform::atomic_write(
                &store.join("manifest.json"),
                &serde_json::to_vec_pretty(&m)?,
            )?;
        }
        Action::Restore => {
            m.restoring = true;
            platform::atomic_write(
                &store.join("manifest.json"),
                &serde_json::to_vec_pretty(&m)?,
            )?;
            replace_ini(&ini_path, plan.after.as_deref(), plan.ini_attrs)?;
            if plan.after.is_some() {
                platform::set_attributes(&ini_path, m.ini_attrs)?;
            }
            // Only revert the folder bit owned by FolderIcons; preserve unrelated later attributes.
            platform::set_attributes(
                &plan.folder,
                (plan.folder_attrs & !platform::READONLY) | (m.folder_attrs & platform::READONLY),
            )?;
            // Do not recurse or remove foreign files. Retain the journal if cleanup fails.
            for name in &m.icons {
                let path = store.join(name);
                if let Some(bytes) = platform::read_optional(&path, 4 * 1024 * 1024)? {
                    ensure!(
                        digest(&bytes) == name[..64],
                        "Stored icon changed; backup retained for manual cleanup"
                    );
                    fs::remove_file(path)?;
                }
            }
            fs::remove_file(store.join("manifest.json"))?;
            // A foreign file may have been placed here; leave it intact.
            if fs::read_dir(&store)?.next().is_none() {
                fs::remove_dir(&store)?;
            }
        }
    }
    Ok(())
}

fn replace_ini(path: &Path, bytes: Option<&[u8]>, attrs: u32) -> Result<()> {
    if path.try_exists()? {
        platform::set_attributes(path, attrs & !platform::READONLY)?;
    }
    let result = match bytes {
        Some(bytes) => platform::atomic_write(path, bytes),
        None => {
            if path.try_exists()? {
                fs::remove_file(path)?;
            }
            Ok(())
        }
    };
    if result.is_err() && path.try_exists()? {
        let _ = platform::set_attributes(path, attrs);
    }
    result
}

pub fn batch_plan(
    paths: &[PathBuf],
    action: Action,
    icon: Option<&[u8]>,
) -> Vec<(PathBuf, Result<FolderPlan>)> {
    let mut seen = std::collections::HashSet::new();
    paths
        .iter()
        .take(128)
        .map(|p| {
            let result = plan(p, action, icon).and_then(|plan| {
                let key = plan.folder.to_string_lossy().to_lowercase();
                if !seen.insert(key) {
                    bail!("Duplicate folder");
                }
                Ok(plan)
            });
            (p.clone(), result)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(tempfile::TempDir);
    impl Fixture {
        fn path(&self) -> &Path {
            self.0.path()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // Appearance tests may intentionally stop before Restore. Release the owned
            // directory's read-only bit so TempDir can clean up its synthetic contents.
            if let Ok(attrs) = platform::attributes(self.path()) {
                let _ = platform::set_attributes(self.path(), attrs & !platform::READONLY);
            }
        }
    }
    fn fixture() -> Fixture {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-output");
        fs::create_dir_all(&root).unwrap();
        Fixture(tempfile::tempdir_in(root).unwrap())
    }
    fn icon() -> Vec<u8> {
        crate::render::ico(&crate::render::render(
            &crate::render::Style::default(),
            None,
        ))
        .unwrap()
    }
    #[test]
    fn preserves_bytes_and_attributes_through_reapply_and_restore() {
        let dir = fixture();
        let f = dir.path();
        let original = ini::unicode(
            "; original\r\n[.ShellClassInfo]\r\nInfoTip=資料\r\nIconResource=old.dll,8\r\n[ViewState]\r\nMode=3\r\n",
        );
        fs::write(f.join("desktop.ini"), &original).unwrap();
        let folder_attrs = platform::attributes(f).unwrap();
        let ini_attrs = platform::attributes(&f.join("desktop.ini")).unwrap();
        execute(&plan(f, Action::Apply, Some(&icon())).unwrap()).unwrap();
        execute(&plan(f, Action::Apply, Some(&icon())).unwrap()).unwrap();
        execute(&plan(f, Action::Restore, None).unwrap()).unwrap();
        assert_eq!(fs::read(f.join("desktop.ini")).unwrap(), original);
        assert_eq!(platform::attributes(f).unwrap(), folder_attrs);
        assert_eq!(
            platform::attributes(&f.join("desktop.ini")).unwrap(),
            ini_attrs
        );
        assert!(!f.join(STORE).exists());
    }
    #[test]
    fn refuses_stale_plans_and_external_edits_without_data_loss() {
        let dir = fixture();
        let f = dir.path();
        let ico = icon();
        let p = plan(f, Action::Apply, Some(&ico)).unwrap();
        fs::write(f.join("desktop.ini"), b"[Other]\nName=user\n").unwrap();
        assert!(execute(&p).is_err());
        assert!(!f.join(STORE).exists());
        execute(&plan(f, Action::Apply, Some(&ico)).unwrap()).unwrap();
        fs::write(f.join("desktop.ini"), b"external change").unwrap();
        assert!(plan(f, Action::Restore, None).is_err());
        assert_eq!(fs::read(f.join("desktop.ini")).unwrap(), b"external change");
    }
    #[test]
    fn absent_ini_restores_to_absent_and_unmanaged_store_is_never_claimed() {
        let dir = fixture();
        let f = dir.path();
        execute(&plan(f, Action::Apply, Some(&icon())).unwrap()).unwrap();
        execute(&plan(f, Action::Restore, None).unwrap()).unwrap();
        assert!(!f.join("desktop.ini").exists());
        fs::create_dir(f.join(STORE)).unwrap();
        fs::write(f.join(STORE).join("personal"), b"keep").unwrap();
        assert!(plan(f, Action::Apply, Some(&icon())).is_err());
        assert_eq!(fs::read(f.join(STORE).join("personal")).unwrap(), b"keep");
    }
    #[test]
    fn pending_journal_recovers_when_ini_write_did_not_happen() {
        let dir = fixture();
        let f = dir.path();
        let p = plan(f, Action::Apply, Some(&icon())).unwrap();
        execute(&p).unwrap();
        let (mut m, _) = manifest(f).unwrap().unwrap();
        m.previous = None; // Simulate a journal committed before the INI write.
        platform::atomic_write(
            &f.join(STORE).join("manifest.json"),
            &serde_json::to_vec_pretty(&m).unwrap(),
        )
        .unwrap();
        fs::remove_file(f.join("desktop.ini")).unwrap();
        execute(&plan(f, Action::Restore, None).unwrap()).unwrap();
        assert!(!f.join(STORE).exists());
    }

    #[test]
    fn committed_journal_refuses_external_deletion_and_preserves_foreign_files() {
        let dir = fixture();
        let f = dir.path();
        execute(&plan(f, Action::Apply, Some(&icon())).unwrap()).unwrap();
        let applied = fs::read(f.join("desktop.ini")).unwrap();
        fs::remove_file(f.join("desktop.ini")).unwrap();
        assert!(plan(f, Action::Restore, None).is_err());
        fs::write(f.join("desktop.ini"), applied).unwrap();
        fs::write(f.join(STORE).join("keep.txt"), b"personal file").unwrap();
        execute(&plan(f, Action::Restore, None).unwrap()).unwrap();
        assert_eq!(
            fs::read(f.join(STORE).join("keep.txt")).unwrap(),
            b"personal file"
        );
    }

    #[test]
    fn malformed_manifest_name_is_rejected_without_panicking_or_touching_ini() {
        let dir = fixture();
        let f = dir.path();
        execute(&plan(f, Action::Apply, Some(&icon())).unwrap()).unwrap();
        let before = fs::read(f.join("desktop.ini")).unwrap();
        let (mut m, _) = manifest(f).unwrap().unwrap();
        // The previous byte-index check could split a multibyte character at offset 64.
        m.icons = vec![format!("{}éa.ico", "x".repeat(61))];
        platform::atomic_write(
            &f.join(STORE).join("manifest.json"),
            &serde_json::to_vec(&m).unwrap(),
        )
        .unwrap();
        assert!(plan(f, Action::Restore, None).is_err());
        assert_eq!(fs::read(f.join("desktop.ini")).unwrap(), before);
    }
}
