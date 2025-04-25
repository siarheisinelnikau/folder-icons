# FolderIcons

An independent Windows desktop tool for original folder colors, badges and photo covers. Built in Rust with an egui desktop interface. MIT licensed. Inspired by the folder-organization use case of [FolderIco](https://www.folderico.com/); no affiliation, copied code or competitor assets.

**Status: first implementation, pending interactive Windows validation.** No repository or release has been published by this session. Planned canonical repository: [siarheisinelnikau/folder-icons](https://github.com/siarheisinelnikau/folder-icons).

## Capabilities

- Select up to 128 folders, individually or in a batch. Apply only to selected folders; no recursive traversal.
- Six original procedural color themes, custom RGB colors, and Star, Done, Favorite, Work and Priority badges.
- PNG/JPEG photo covers with centered crop and an in-app live preview. Photos are embedded in the ICO and are never modified.
- Review an Apply or Restore plan before any appearance change. Each folder has its own result; a failure does not undo successful folders in the batch.
- Generate RGBA ICO files containing 16, 24, 32, 48, 64, 128 and 256 pixel frames.
- Preserve unrelated `desktop.ini` lines/settings. Back up exact original bytes and attributes before changing appearance. Restore the pre-FolderIcons appearance, including an existing custom icon.
- Keep icons and recovery data beside the target folder, in a hidden `.foldericons` directory. Relative icon paths survive application restarts and folder moves when this directory travels with the folder.
- Optional per-user Explorer context menu, enabled and removed explicitly in Settings. No shell extension, service or administrator installation.
- Background startup update checks (on by default, persistent opt-out), manual checks, and user-initiated download, checksum verification, executable replacement and restart.

## Requirements and build

Windows 10/11 x64 is the implementation target. Interactive support on those versions is **not yet verified**. The UI uses OpenGL through eframe/glow and requires a compatible graphics driver. No runtime model, AI API, WebView or external theme assets are needed. Network access is used only for enabled update checks and user-requested updates.

Build with Rust 1.95+ and the MSVC C++ toolchain/Windows SDK:

```powershell
Set-Location E:\work\github\folder-icons
cargo test --locked -j 2
cargo clippy --locked --all-targets -j 2 -- -D warnings
./scripts/package.ps1
```

Build output is repository-local. Cargo is limited to two jobs. The x64 MSVC configuration requests the static CRT. Packaging creates `dist/FolderIcons-v0.1.0.exe`, `FolderIcons-v0.1.0.exe.sha256`, and third-party license notices. Dependency downloads may be required on a fresh machine; `--offline` works with a populated cache. See [verification notes](docs/TESTING.md).

## Use

1. Start the portable EXE from a writable, permanent location.
2. Click **Add folders…**, select folders, and choose a theme, badge and/or photo. The large preview shows the resulting artwork.
3. Click **Build Apply plan**. Review the exact folder paths and file/attribute changes. Unsupported folders are listed as skipped.
4. Click **Apply reviewed plan**. Read each result. Explorer may refresh lazily; reopening the folder or pressing Refresh may be needed. The application sends a targeted shell notification only after an explicit successful action.
5. To undo, select the same folders, click **Build Restore plan**, then **Restore reviewed plan**. Repeated Apply retains the original first backup, rather than replacing it with an already customized state.

Paths passed on the command line preselect folders without changing them. Example: `FolderIcons-v0.1.0.exe "D:\Projects\Sample"`.

Settings live in `%LOCALAPPDATA%\FolderIcons\settings.json`. **Settings & updates** shows the installed version, startup-check toggle, manual update action, and Explorer integration controls. The context menu opens this editor with a folder selected; on Windows 11 it may appear in **Show more options**. Remove it before moving/deleting the executable, or enable it again from the new location.

## Preservation and recovery

FolderIcons updates the icon keys in `[.ShellClassInfo]`, preserves unrelated text, and writes UTF-16LE with a BOM as required by Windows. Existing UTF-8/ASCII and UTF-16LE INI files are supported. Ambiguous legacy code pages, malformed encodings and duplicate shell sections are refused rather than re-encoded incorrectly. Exact original bytes are kept for Restore.

The application rejects drive roots, symlinks/junctions/reparse points (including cloud placeholders), oversized INI files, unmanaged `.foldericons` directories and stale plans. Read-only/protected/network folders may report permission errors; there is no elevation flow. Local regular folders are the initial intended workload. Cloud sync, network shares, ACL preservation and Explorer caching need manual validation.

If `desktop.ini` changes after Apply, Restore refuses to overwrite it. Keep `.foldericons/manifest.json` and reconcile the changes manually. A write-ahead journal supports retry or Restore after partial operations. It is not a cross-folder transaction or protection against an uncooperative process changing files at exactly the same time. See [architecture and recovery details](docs/ARCHITECTURE.md).

Do not remove `.foldericons` while its icon is in use. To reset an appearance, use Restore first. After an application crash, a stale `.foldericons.lock` can remain in a selected folder; close all FolderIcons processes before removing that one lock file and building a new plan. An unmanaged directory is never automatically claimed or recursively deleted.

## Updates and release preparation

Stable checks use the canonical public GitHub Releases API without tokens. A 404, unpublished repository, network error or rate limit is an unavailable check, never an up-to-date result. Startup errors and no-update results remain in Settings without modal dialogs; an available update appears as a banner.

**Download and install** accepts the matching versioned x64 EXE from the canonical repository. It verifies advertised length, SHA-256 (GitHub asset digest and/or the matching `.exe.sha256` asset), and the x64 PE header before staging. A helper waits for this app to exit, keeps a versioned rollback executable, replaces the running EXE at its existing path, and restarts. Settings and folder backups are outside that replacement. No unattended download/install occurs. This workflow is implemented but has not been executed in this development session; it needs a real published test release and manual validation.

The executable directory must be writable. Rollback files `*.exe.previous-VERSION`, `.FolderIcons-helper-*.exe` and helper `*.error.txt` diagnostics are retained for inspection and can be removed after all FolderIcons processes exit and the update is verified. Interrupted `.FolderIcons-update-VERSION.exe` stages may also need removal before a retry. Release files are unsigned in this initial build. The [release-note draft](CHANGELOG.md) states the validation limits. Publishing is deliberately outside the current task.

## Screenshots

Real application and **Settings & updates** screenshots are pending. GUI launch, screenshots and desktop interaction were prohibited for this development session. No mock screenshots or broken image links are supplied. Add real screenshots after completing the manual checklist in [docs/TESTING.md](docs/TESTING.md).
