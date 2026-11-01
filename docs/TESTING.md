# FolderIcons 0.2.0 verification

The 0.2.0 change adds 119 six-language UI messages, message parameters retained until paint, and saved Light/Dark settings. Catalog tests check every translation and parameter set. Settings tests load the legacy format without language/theme fields and roundtrip a changed language/theme while preserving startup-update opt-out. A formatting regression ensures placeholder-like text inside user data is never interpreted as another parameter.

Source review retains the existing `egui_software_backend` CPU rasterizer and pinned `softbuffer` Windows GDI presentation path. `Host::paint` skips hidden/minimized/occluded windows; dirty worker state survives for restore; `about_to_wait` uses `Wait` or meaningful work deadlines. Applying the selected theme happens only on a real theme change, and saving preferences occurs only after a changed control. Language lookup uses an initialized-once built-in catalog, with no network requests or per-frame disk reads.

Real 0.2.0 production client-area images in `docs/screenshots` cover English light main/settings, Russian dark main, German dark settings, French light main, Spanish dark main and Italian light main. All eleven final capture states (main/settings across languages plus the three symbol categories and French Organization) use finite offscreen capture invocations; each invocation rendered four frames, saved fresh PNG/JSON files and exited with code 0. Each actual PNG was opened and inspected for text, spacing, controls, overflow, readable palette and preserved Unicode folder names. The captures do not certify OS input, native dialog controls, registry integration, idle/hidden rendering or subjective responsiveness.

To select a capture language/theme:

```powershell
./scripts/capture-ui.ps1 -FixtureRoot ./.test-output/visual-fixtures -Language ru -Theme dark
# Or one bounded offscreen production frame sequence:
./dist/FolderIcons-v0.2.0.exe --capture E:/captures/settings-de.png --screen settings --scenario populated --fixture-root E:/fixtures --language de --theme dark
```

Native Windows dialog button localization follows the OS; FolderIcons supplies translated dialog titles and filters. Optional Explorer registration uses the selected language only after the user chooses Enable context menu; merely switching language does not write to the registry. Technical errors remain original diagnostics under translated failure labels.

Formatting, all 21 tests (18 library and 3 application), strict all-target Clippy, and the optimized Windows x64 build passed. Byte-only packaging verifies AMD64 PE32+, the Windows GUI subsystem, version 0.2.0.0, nine embedded icon images matching the source artwork, and the SHA-256 sidecar. No real folder appearance, normal settings, registry entry or updater was modified during these checks.

The final packaged executable is 7,317,504 bytes; SHA-256 `6dd105ca71b9d21e77c06e6d24d7b26d126ac936cd030a22a85121bd55cfed1b`. A fresh finite native-window diagnostic on this exact EXE, PID 13972, used an isolated empty fixture, no network/user settings/input, and exited normally with `gate_passed=true`:

| State | Seconds | Raster frames | Presents | CPU, % of one core |
| --- | ---: | ---: | ---: | ---: |
| Visible settled idle | 5.000 | 0 | 0 | 0.0 |
| Hidden settled idle | 5.000 | 0 | 0 | 0.0 |
| Restore | 1.001 | 1 | 1 | 0.0 at process timer resolution |
| Return to idle | 3.000 | 0 | 0 | 0.0 |

Restore rasterization took 1.41 ms; GDI presentation took 0.62 ms. Startup rendered two frames in one second and is excluded from idle measurements. Twenty logical CPUs were exposed, so idle machine-normalized CPU was also 0.0%. The actual backend was `egui_software_backend` CPU rasterization plus `softbuffer` Windows GDI. GPU telemetry was unavailable. This sample establishes quiet default-theme rendering for the isolated fixture; theme/language input transitions, populated real workflows, minimized state, native dialogs and subjective smoothness remain manual acceptance checks. The owned diagnostic process exited; no normal application instance was launched.


# Historical verification (0.1.1 and earlier)

# Shared updater and icon audit, 0.1.1

The old optimized executable had no PE icon resource section. The application now embeds matching original folder artwork at 16, 20, 24, 32, 40, 48, 64, 128 and 256 pixels, and explicitly assigns the window/taskbar artwork. `inspect_executable.py` compares the final PE payload with the source ICO and checks file/product versions; `verify_icons.py` draws loaded native icon resources on light/dark backgrounds without opening a window. This editor has no tray.

The updater's local module now contains product configuration only. Shared Service events wake the reactive host. The persisted startup toggle/manual check flow and per-EXE `.sha256` release contract are preserved. The new helper protocol is parsed before ordinary startup, and cleanup starts after the first successful presentation. Old helpers use copies of the old executable and can still install this release; old leftover helper/rollback files are preserved for inspection.

Source review confirms CPU rasterization with the pinned egui software backend and softbuffer 0.4.8 Win32 DIB/BitBlt presentation. A hidden worker wake now marks the scene dirty for restoration; minimized, hidden and occluded windows skip rasterization/presentation. Idle waits use explicit useful work/repaint deadlines. A native resource proof and finite runtime diagnostic are separate checks; neither certifies user-perceived lag or native tray interaction.

Current verification used repository-local targets with one Cargo job: formatting, Clippy with `-D warnings`, all 18 tests, and optimized Windows x64 compilation passed. The PE is GUI subsystem 2, AMD64, Windows version 0.1.1.0; all nine embedded ICO payloads exactly match the checked-in source. The [native icon resource proof](icon-resource-proof.png) was opened and visually reviewed.

The finite isolated empty-fixture diagnostic launched the exact optimized binary, PID 56568, and exited normally with `gate_passed=true`, without input automation, network or user settings:

| State | Seconds | Raster frames | Presents | CPU, % of one core |
| --- | ---: | ---: | ---: | ---: |
| Visible settled idle | 5.009 | 0 | 0 | 0.0 |
| Hidden settled idle | 5.003 | 0 | 0 | 0.0 |
| Restore | 1.001 | 1 | 1 | 0.0 at process timer resolution |
| Return to idle | 3.001 | 0 | 0 | 0.0 |

The restore raster took 2.17 ms and its presentation 0.84 ms in this sample. Startup produced two frames in 1.005 s; startup is not an idle sample. The backend was the production CPU rasterizer and pinned Win32 GDI presenter. GPU telemetry was unavailable. This establishes quiet rendering for the isolated sample; populated real workflows, pointer/keyboard behavior, minimized state and perceived smoothness require separate validation.

The historical results below refer to older binaries unless explicitly labeled otherwise. Current final test/build/diagnostic results are included in the task handoff.

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
- Repeat bounded diagnostics with representative populated workloads when needed; the 0.1.1 isolated empty-fixture visible/hidden/restore sample is recorded above.
- Physically drag folders from Explorer onto empty, populated and review states; verify hover/cancel, multiple folders, duplicate/file rejection and queued intake during work. Source and injected-event tests cover these paths; native gesture acceptance remains unverified.
- Verify keyboard traversal, accessibility actions, native folder/photo dialogs, pointer/scroll/resize behavior and actual DPI changes; the static 150% capture checks layout only.
- Through the GUI, Apply to a disposable folder and a batch with Unicode/spaces, reapply, then Restore. Confirm Explorer refresh and exact original INI/attributes. Engine tests pass; live GUI Apply/Restore was not completed before computer use was stopped and subsequently excluded.
- Inspect visible restore/minimize behavior and fonts/resources on the real screen. Counts verify quiet hidden operation and a restore frame, not visual correctness of every OS interaction.
- Test Windows 10/11 clean-machine portable deployment and network/cloud/ACL scenarios. Reparse/cloud-placeholder folders are deliberately rejected.
- Validate explicit HKCU context-menu enable/remove, Windows 11 Show more options, EXE relocation and command-line folder handoff.
- Against a future published test release, exercise availability, disabled-startup zero-request behavior, progress, checksum errors, helper handshake, replacement, restart and rollback. The real updater has never been executed in this session.

No release has been published. Authenticode signing and remote GitHub operations remain outside this local phase.
