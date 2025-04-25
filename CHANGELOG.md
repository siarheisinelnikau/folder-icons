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

### Platform and validation limits

Initial target: Windows 10/11 x64, one architecture. Planned standalone asset: `FolderIcons-v0.1.0.exe`, with its `.exe.sha256` checksum and license notices. No release is currently published. Binary is unsigned. OpenGL-compatible graphics is required for the UI.

Automated validation covers the rendering/configuration engine and local compilation; see [verification notes](docs/TESTING.md) for the exact recorded results. Real GUI/settings screenshots, Explorer appearance and integration, idle performance, portable deployment and an end-to-end update against a published release are pending. Legacy non-Unicode INI code pages and reparse/cloud placeholder folders are deliberately refused. Do not describe those deferred checks as passed when publishing this draft.
