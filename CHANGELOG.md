# Release notes

## 0.1.0 — unpublished first-release draft

FolderIcons introduces a portable Windows folder appearance editor with original generated artwork and a review step before changing folders.

### Added

- Six color themes, custom RGB colors, five badges and PNG/JPEG photo covers with live preview.
- Batch selection and reviewed Apply/Restore plans with per-folder results.
- Multi-resolution RGBA ICO generation and folder-local relative icon references.
- Preservation of unrelated desktop.ini settings, exact original-content backup, interrupted-operation journal and conservative conflict detection.
- Explicit per-user Explorer context-menu enable/remove controls.
- Installed-version display, opt-out background startup checks, manual stable-release checks and user-initiated SHA-256 verified portable update installation/restart.
- Local Windows packaging script, dependency license notices and noninteractive CI.
- A clearer light editor with larger typography, compact style controls, a single apply action in the review, and readable CJK folder names when the Windows fallback font is available.
- CPU software rendering with GDI presentation, accessibility support and bounded client-area capture/diagnostic modes.

### Fixed during local validation

- Initialize accessibility before showing the native window, avoiding a startup panic.
- Stop scheduling waits after an exit request so the process can shut down normally.
- Discard stale repaint deadlines and duplicate unchanged pointer positions; preserve a quiet idle/hidden window and the retained bitmap for restore.

### Platform and validation limits

Initial target: Windows 10/11 x64, one architecture. Planned standalone asset: `FolderIcons-v0.1.0.exe`, with its `.exe.sha256` checksum and license notices. No release is currently published. Binary is unsigned. The application rasterizes on the CPU and presents with Windows GDI.

Automated validation covers 13 engine tests, local compilation, real production-UI client captures and bounded visible/hidden/restore diagnostics; see [verification notes](docs/TESTING.md) for exact results. Perceived responsiveness, native dialogs, live Apply/Restore, Explorer appearance/integration, clean-machine deployment and an end-to-end update remain pending. Legacy non-Unicode INI code pages and reparse/cloud placeholder folders are deliberately refused. Do not describe deferred checks as passed when publishing this draft.
