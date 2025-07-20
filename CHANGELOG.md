## 0.1.1 — local changes

- Embed the original application icon in the executable and explicitly assign the window/taskbar identity.
- Use the shared Rust Auto Updator core while preserving per-EXE SHA-256 assets, startup opt-out and user-initiated installation.
- Retain hidden-window changes for restore and skip rasterization/presentation while occluded.
- Actual tray/taskbar interaction, online update installation and perceptual responsiveness require separate acceptance; this application has no tray.

# Release notes

## 0.1.0 — unpublished first-release draft

FolderIcons introduces a portable Windows folder appearance editor with original generated artwork and a review step before changing folders.

### Added

- Six color themes, custom RGB colors, 17 original symbols and PNG/JPEG photo covers with live preview.
- Native Explorer folder-drop handling, a highlighted target, bounded background validation, duplicate rejection and status feedback. Drops only update the selection.
- A compact visual symbol picker grouped into Content, Organization and Status, with matching anti-aliased artwork in preview and ICO output.
- Batch selection and reviewed Apply/Restore plans with per-folder results.
- Multi-resolution RGBA ICO generation and folder-local relative icon references.
- Preservation of unrelated desktop.ini settings, exact original-content backup, interrupted-operation journal and conservative conflict detection.
- Explicit per-user Explorer context-menu enable/remove controls.
- Installed-version display, opt-out background startup checks, manual stable-release checks and user-initiated SHA-256 verified portable update installation/restart.
- Local Windows packaging script, dependency license notices and noninteractive CI.
- A clearer light editor with larger typography, compact style controls, a single apply action in the review, and readable CJK folder names when the Windows fallback font is available.
- CPU software rendering with GDI presentation, accessibility support and bounded client-area capture/diagnostic modes.
- A compact 780 × 590 default window with a single folder list, horizontal appearance controls and a focused confirmation step. Text sizes and primary button sizes are preserved.

### Fixed during local validation

- Initialize accessibility before showing the native window, avoiding a startup panic.
- Stop scheduling waits after an exit request so the process can shut down normally.
- Discard stale repaint deadlines and duplicate unchanged pointer positions; preserve a quiet idle/hidden window and the retained bitmap for restore.
- Invalidate in-flight review results after a folder drop and keep queued intake timers independent of UI repaint timing.
- Treat an early diagnostic-window close as cancellation; use fresh report paths so an old rendering result cannot be mistaken for a new pass.

### Platform and validation limits

Initial target: Windows 10/11 x64, one architecture. Planned standalone asset: `FolderIcons-v0.1.0.exe`, with its `.exe.sha256` checksum and license notices. No release is currently published. Binary is unsigned. The application rasterizes on the CPU and presents with Windows GDI.

Automated validation covers 19 library/application tests, local compilation and production-UI captures of the editor, settings, themed symbols and drop target. An earlier compact build passed bounded visible/hidden/restore diagnostics; the latest post-drop runtime attempt ended before its report and is not a pass. Physical Explorer drag/drop, perceived responsiveness, native dialogs, live Apply/Restore, Explorer appearance/integration, clean-machine deployment and an end-to-end update remain pending. See [verification notes](docs/TESTING.md). Legacy non-Unicode INI code pages and reparse/cloud placeholder folders are deliberately refused.
