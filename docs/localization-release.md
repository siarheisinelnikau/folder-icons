# FolderIcons v0.2.0

This is the first published release of FolderIcons.

## Initial capabilities

- Customize selected Windows folders with original colors, badges and PNG/JPEG photo covers using an in-app preview.
- Review each Apply or Restore plan, generate multi-resolution ICO files and preserve unrelated desktop.ini settings.
- Keep original appearance backups and recovery data alongside each folder; optional Explorer context-menu registration requires an explicit settings action.

## Localization and appearance

## Added
- Complete built-in English, Russian, French, German, Spanish and Italian interface catalogs, with native language names in Settings → Interface.
- Saved Light and Dark themes, applied immediately across the editor, review, settings, symbol picker and drop overlay.

## Improved
- Existing status messages change language immediately; file names, folder paths and technical diagnostic details retain their original content.
- Native file-dialog titles and optional Explorer shortcut labels use the selected language.
- Existing settings migrate without losing the startup-update preference. The complete English interface and original light appearance remain the defaults.
- Wider localized labels and wrapping main actions accommodate translated text without reducing font sizes.

## Verification and download
- Windows x64 portable executable: `FolderIcons-v0.2.0.exe`, with matching `.exe.sha256` and third-party notices.
- All 22 tests, formatting, strict all-target Clippy and the optimized x64 build passed. Catalog checks preserve placeholders; legacy settings migrate and roundtrip correctly.
- The final executable passed a finite isolated CPU/GDI diagnostic: no rasterization or presentation in visible/hidden idle, and one frame on restore. GPU telemetry and interactive acceptance remain unverified.
- Real client-area captures use the production CPU software renderer. They do not certify OS input, native dialog behavior or performance of normal operation.
- The application and updater are unsigned. Live in-place update installation and manual Explorer workflows remain separate acceptance checks.
