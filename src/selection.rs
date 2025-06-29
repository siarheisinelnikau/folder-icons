//! Bounded, read-only folder intake shared by native drops and the folder picker.
use crate::platform;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

pub const MAX_FOLDERS: usize = 128;
pub const MAX_PENDING: usize = 256;

#[derive(Debug, Default)]
pub struct Intake {
    pub folders: Vec<PathBuf>,
    pub duplicates: usize,
    pub rejected: Vec<String>,
    pub overflow: usize,
}
impl Intake {
    pub fn message(&self) -> String {
        let count = self.folders.len();
        let mut text = format!(
            "Added {count} {}.",
            if count == 1 { "folder" } else { "folders" }
        );
        if self.duplicates > 0 {
            text.push_str(&format!(" {} already selected.", self.duplicates));
        }
        if self.overflow > 0 {
            text.push_str(&format!(
                " {} skipped: limit is {MAX_FOLDERS} selected folders.",
                self.overflow
            ));
        }
        if !self.rejected.is_empty() {
            text.push_str(&format!(
                " {} unsupported item(s). {}",
                self.rejected.len(),
                self.rejected[0]
            ));
        }
        text
    }
}

fn key(path: &Path) -> String {
    let value = path.to_string_lossy().into_owned();
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value
    }
}

pub fn inspect(candidates: &[PathBuf], existing: &[PathBuf], overflow: usize) -> Intake {
    let mut seen: HashSet<_> = existing
        .iter()
        .map(|p| key(&std::fs::canonicalize(p).unwrap_or_else(|_| p.clone())))
        .collect();
    let mut result = Intake {
        overflow: overflow + candidates.len().saturating_sub(MAX_PENDING),
        ..Default::default()
    };
    for candidate in candidates.iter().take(MAX_PENDING) {
        let checked = (|| -> anyhow::Result<PathBuf> {
            anyhow::ensure!(candidate.is_absolute(), "Choose an absolute folder path");
            platform::safe_path(candidate)?;
            let folder = std::fs::canonicalize(candidate)?;
            anyhow::ensure!(
                folder.is_dir(),
                "Only folders can be added; use Choose photo for images"
            );
            anyhow::ensure!(folder.parent().is_some(), "Drive roots are not supported");
            Ok(folder)
        })();
        match checked {
            Err(error) => result.rejected.push(format!(
                "{}: {error}",
                candidate.file_name().unwrap_or_default().to_string_lossy()
            )),
            Ok(folder) => {
                if !seen.insert(key(&folder)) {
                    result.duplicates += 1;
                } else if existing.len() + result.folders.len() >= MAX_FOLDERS {
                    result.overflow += 1;
                } else {
                    result.folders.push(folder);
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> tempfile::TempDir {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-output");
        std::fs::create_dir_all(&root).unwrap();
        tempfile::tempdir_in(root).unwrap()
    }
    #[test]
    fn accepts_unicode_folders_and_rejects_files_and_missing_paths() {
        let dir = fixture();
        let folder = dir.path().join("Photos 旅行");
        std::fs::create_dir(&folder).unwrap();
        let file = dir.path().join("cover.png");
        std::fs::write(&file, b"fixture").unwrap();
        let result = inspect(
            &[folder.clone(), file.clone(), dir.path().join("missing")],
            &[],
            0,
        );
        assert_eq!(result.folders, [std::fs::canonicalize(&folder).unwrap()]);
        assert_eq!(result.rejected.len(), 2);
        assert_eq!(std::fs::read(file).unwrap(), b"fixture");
        assert_eq!(std::fs::read_dir(folder).unwrap().count(), 0);
    }
    #[test]
    fn canonical_aliases_and_existing_selection_are_not_added_twice() {
        let dir = fixture();
        let folder = dir.path().join("Project");
        std::fs::create_dir(&folder).unwrap();
        let result = inspect(
            &[folder.clone(), folder.join(".")],
            std::slice::from_ref(&folder),
            0,
        );
        assert!(result.folders.is_empty());
        assert_eq!(result.duplicates, 2);
        #[cfg(windows)]
        {
            let changed_case = PathBuf::from(folder.to_string_lossy().to_uppercase());
            assert_eq!(inspect(&[changed_case], &[folder], 0).duplicates, 1);
        }
    }
    #[test]
    fn selection_limit_is_reported_without_mutating_inputs() {
        let dir = fixture();
        let folder = dir.path().join("Next");
        std::fs::create_dir(&folder).unwrap();
        let existing: Vec<_> = (0..MAX_FOLDERS)
            .map(|i| dir.path().join(format!("Existing{i}")))
            .collect();
        let result = inspect(std::slice::from_ref(&folder), &existing, 3);
        assert_eq!(result.overflow, 4);
        assert!(result.folders.is_empty());
        assert!(folder.is_dir());
        assert!(
            !inspect(&[PathBuf::from("relative-folder")], &[], 0)
                .rejected
                .is_empty()
        );
    }
}
