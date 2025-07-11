# Verification

## Automated checks

```powershell
cargo fmt --check
cargo test --locked --offline -j 2
cargo clippy --locked --offline --all-targets -j 2 -- -D warnings
./scripts/package.ps1 -Offline
```

The implementation/follow-up session used Windows x64 and Rust 1.96.1. All 19 tests passed (16 library and 3 application tests), formatting passed, Clippy passed with `-D warnings`, and the optimized x64 release was packaged. Cargo's incremental-cache hard-link warnings are environment-level file-copy fallbacks, not source lint failures.

Tests cover distinct artwork and multi-size ICO roundtrips for all 17 symbols, photo embedding and bounded decoding, INI text/encoding handling, reapply/restore byte and attribute preservation, stale plans and external-edit rejection, pending journals, malformed manifests, external deletion/foreign-file preservation, and release/checksum parsing. Selection tests cover Unicode folders, files/missing paths, case/alias duplicates and limits. Application tests cover queued drops during preview/planning, rejection of late review results and a symbol catalog that stops scheduling after loading. These tests inject paths/events; they do not perform a physical Explorer drag. Fixtures live under ignored `.test-output`. The first session encountered access-denied rename failures in the sandbox's virtualized system Temp; repository-local fixtures passed without elevation.

Packaging never launches the normal application or updater. Its byte verifier checks x64 PE32+, the Windows GUI subsystem and the matching SHA-256 sidecar. These checks do not certify deployment or responsiveness. The final checksum is in `dist/FolderIcons-v0.1.0.exe.sha256`.

## Runtime rendering diagnostics

The initial OpenGL build was reported to cause severe lag. It was replaced by egui 0.33.3 → egui_software_backend 0.0.2 CPU rasterization → softbuffer 0.4.8 Win32 GDI. The active dependency tree contains no eframe/glow/glutin/wgpu renderer. The prior CPU build's PE imports included `gdi32.dll` and no direct OpenGL/DirectX graphics imports. This does not disable Windows composition or constitute GPU telemetry.

The real host instruments UI/raster frames, GDI presents, their wall time, own-process CPU time, repaint causes and non-sensitive input-event counts. Diagnostics use the optimized application, isolated fixtures, no startup network/settings loading, a 30-second internal limit and a 35-second external limit. They show, hide, restore and exit only the owned test instance. No computer-use tools are required.

Current packaged executable: `FolderIcons-v0.1.0.exe`, 8,438,784 bytes, SHA-256 `58b56cefa3f625fc127d5daf4e6cc4b9268f2c923ef19fbe9a1d4ba97377f263`. The latest native-window attempt ended before producing its report, so the runtime gate for the folder-drop/symbol update remains incomplete. Diagnostic cancellation now returns an error, and the script requires a fresh uniquely named report instead of accepting an older result. Windowless captures completed separately; they do not pass this gate.

The following measurements are a historical baseline from the earlier compact build, before the folder-drop/symbol changes: 8,413,696 bytes, SHA-256 `99f2fa8da9b6a93eabe4bf51a6f559e534784d463b94e4b5bcb8c53f7851662a`. That packaged-binary diagnostic used PID 54820 and completed with exit code 0. Twenty logical CPUs were exposed to the process. Clean measured intervals:

| State | Duration | UI/raster frames | GDI presents | CPU, one logical core |
| --- | ---: | ---: | ---: | ---: |
| Visible, settled | 5 s | 0 | 0 | 0.0% at sampling resolution |
| Hidden | 5 s | 0 | 0 | 0.0% at sampling resolution |
| Restore | 1 s | 1 | 1 | 0.0% at sampling resolution |
| Return to settled idle | 3 s | 0 | 0 | 0.0% at sampling resolution |

The compact build rendered two startup frames in total. Restore rasterized in 0.68 ms and presented in 0.32 ms. These are local samples, not universal latency claims. Earlier apparent idle spikes included real pointer motion/clicks; input-contaminated samples are labeled and repeated. Small finite focus/window invalidations also require a new clean interval. Ongoing frames, sustained CPU load without input, or failure to settle within the budget fails the gate.

Validation fixed an AccessKit startup panic (adapter must precede window visibility), stale repaint scheduling, unchanged duplicate pointer invalidation, and a shutdown stall caused by setting another wait after `exit`. The repaired bounded diagnostic completed successfully and exited. GPU/driver telemetry and user-perceived smoothness remain unverified. Keyboard, pointer, native dialogs and live folder workflows have not been certified by static captures.

## Real client-area captures without computer use

```powershell
$fixtureRoot = ./scripts/create-visual-fixtures.ps1
./scripts/capture-ui.ps1 -FixtureRoot $fixtureRoot
# Optional monitored native-window test, separate from static capture:
./scripts/capture-ui.ps1 -FixtureRoot $fixtureRoot -Diagnostics
# Select the packaged executable explicitly:
./scripts/capture-ui.ps1 -FixtureRoot $fixtureRoot -ExecutablePath ./dist/FolderIcons-v0.1.0.exe
```

The fixture creator refuses to claim unmarked existing directories. It creates disposable folders, a Unicode INI and an original geometric PNG. Capture mode runs the production UI and software renderer for exactly four finite passes, writes PNG/metadata files, and exits. Each child has an external 12-second timeout. It does not use computer use, native dialogs, a desktop screenshot service, normal user settings, network requests, registry registration, the updater or folder mutations. The review scenario reads real plans without executing them.

All nine generated states were inspected as actual PNG files: main/editor, Settings, empty, confirmation, drop hover, the Content/Organization/Status symbol categories and the compact 150% layout. The compact follow-up removed the tall header and permanent sidebar, grouped folders into one list, and placed the preview beside appearance controls. Confirmation has its own compact state with exact paths and a Back action. The symbol picker uses cached thumbnails from the same renderer as the ICO artwork, with a height fitted to each category. Font sizes and action target sizes were preserved. The [740 × 560 logical viewport at 150% scale](screenshots/compact.png) scrolls rather than shrinking text. README images are 780 × 590 logical/physical pixels at 100% scale. They capture the client surface, not native title bars, Explorer or OS dialogs. The drop-hover image establishes the target's appearance only.

PNG and JSON files in `docs/screenshots` are real application output. Metadata records scenario, renderer and scale. CI creates isolated fixtures and captures these production screens without opening native windows.

## Remaining manual acceptance

- Confirm perceived smoothness and responsiveness on the user's desktop. No screenshot or CPU sample replaces that confirmation.
- Complete a fresh bounded native rendering diagnostic for the current packaged binary, including settled visible, hidden and restored states.
- Physically drag folders from Explorer onto empty, populated and review states; verify hover/cancel, multiple folders, duplicate/file rejection and queued intake during work. Source and injected-event tests cover these paths; native gesture acceptance remains unverified.
- Verify keyboard traversal, accessibility actions, native folder/photo dialogs, pointer/scroll/resize behavior and actual DPI changes; the static 150% capture checks layout only.
- Through the GUI, Apply to a disposable folder and a batch with Unicode/spaces, reapply, then Restore. Confirm Explorer refresh and exact original INI/attributes. Engine tests pass; live GUI Apply/Restore was not completed before computer use was stopped and subsequently excluded.
- Inspect visible restore/minimize behavior and fonts/resources on the real screen. Counts verify quiet hidden operation and a restore frame, not visual correctness of every OS interaction.
- Test Windows 10/11 clean-machine portable deployment and network/cloud/ACL scenarios. Reparse/cloud-placeholder folders are deliberately rejected.
- Validate explicit HKCU context-menu enable/remove, Windows 11 Show more options, EXE relocation and command-line folder handoff.
- Against a future published test release, exercise availability, disabled-startup zero-request behavior, progress, checksum errors, helper handshake, replacement, restart and rollback. The real updater has never been executed in this session.

No release has been published. Authenticode signing and remote GitHub operations remain outside this local phase.
