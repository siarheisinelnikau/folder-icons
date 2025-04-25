# Verification

## Safe automated scope

Run from the repository root using the existing MSVC toolchain:

```powershell
cargo fmt --check
cargo test --locked --offline -j 2
cargo clippy --locked --offline --all-targets -j 2 -- -D warnings
./scripts/package.ps1 -Offline
```

Tests render artwork in memory, parse local release fixtures, and create disposable directories under the ignored repository-local `.test-output` for INI/attribute/journal checks. They do not launch the GUI, call the updater, contact GitHub, register Explorer integration, send shell notifications, or modify existing user folders. Package generation compiles/copies files only; it never starts the generated EXE.

Implementation session (2026-10-31, Windows x64, Rust 1.96.1):

- `cargo check --offline --locked -j 2`: passed.
- `cargo fmt --check`: passed.
- `cargo clippy --offline --locked --all-targets -j 2 -- -D warnings`: passed. Cargo emitted environment-level incremental hard-link fallback warnings; source lint checks passed.
- `cargo test --offline --locked -j 2`: 13 tests passed, no failed/ignored tests. Covers ICO roundtrip and all badges, photo embedding and bounded decoding, INI text/encoding handling, reapply/restore byte and attribute preservation, stale-plan/external-edit rejection, pending-journal recovery, malformed manifest rejection, external deletion rejection and foreign-file preservation, stable release selection and malformed update payload/checksum rejection.
- `./scripts/package.ps1 -Offline`: passed; optimized Windows x64 build, versioned EXE, checksum and third-party notices generated. The EXE was never launched.
- SHA-256 of `dist/FolderIcons-v0.1.0.exe`: `38334576e83adf4e4c8abb2a8439b4b8f088e089e7253803932a499da8a4e21d`.
- The package verifier inspects the x64 PE32+ Windows GUI subsystem and checks that the sidecar SHA-256 names/matches the EXE. No deployment result is inferred from byte inspection.

The initial filesystem test run hit access-denied errors for atomic rename in the sandbox's virtualized system Temp directory. Fixtures were moved into the authorized repository-local `.test-output`; all filesystem scenarios then passed without elevation. The compiler also fell back from incremental-cache hard links to file copies in this environment. No real user folder appearance or registry settings were changed.

## Deferred manual acceptance checklist

Perform these later on disposable folders, with explicit permission for desktop interaction:

- Launch the packaged EXE on Windows 10/11 x64 without a Rust toolchain; confirm portable/static CRT deployment and driver compatibility.
- Verify the real layout, color controls, all badges, keyboard navigation, DPI scaling (100/150/200%), small window sizes, photo crop/transparency and both native file dialogs.
- Capture real main-window and Settings screenshots and embed them in the README.
- Apply to a single folder and a batch including Unicode/spaces. Check Explorer's small/large icons and thumbnails, targeted refresh and persistence after restarting the app/Explorer.
- Apply over a preexisting UTF-16LE `desktop.ini` with localized names/InfoTip and a custom icon. Reapply, then Restore and compare exact original bytes and appearance.
- Rename/move/copy the entire customized folder with `.foldericons` and confirm relative paths. Test access-denied, readonly, network and cloud folders without claiming support until verified.
- Test explicit context-menu enable/remove in HKCU, Windows 11 Show more options and paths containing spaces/non-ASCII. Confirm opening a folder from the menu performs no automatic mutation. Re-enable after moving the EXE.
- Measure idle/active CPU and UI responsiveness; minimize, restore and close while a bounded operation is active. No idle performance measurements were taken in this run.
- Against a future published test release, verify 404/unpublished and rate-limit states, a newer stable version, prerelease exclusion, opt-out making zero startup requests, manual checking, progress, length/checksum failures and read-only installation directories.
- Exercise the real helper handshake, replacement, restart, versioned rollback, a locked EXE, helper timeout and interrupted download. Confirm settings and on-folder backups survive. The updater was never executed in this session.

No hardware/Explorer/GUI results are inferred from successful compilation or unit tests. Authenticode signing, publication and real release API availability remain outside this local implementation phase.
