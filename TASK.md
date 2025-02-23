# FolderIcons

Repository: `folder-icons`
Priority: 10 of 15 (user-requested launch order)
Product: FolderIcons
Purpose: Custom colors, badges and photo covers for folders
Inspiration: https://www.folderico.com/

Independent open-source implementation; no affiliation, competitor code or copied assets.

## Requested implementation

Build a native Windows tool for choosing folders and applying colors, clear badges or photo-based icon covers with a live preview. Support batch selection, a compact set of original bundled themes, an Apply plan and Restore original appearance. Generate valid ICO assets and preserve existing desktop.ini content/settings; manage generated icon paths so appearance survives restarts. Use original or permissively licensed assets, not competitor assets.

Keep the icon rendering/config planning engine testable in memory and temporary folders. Explorer integration/context-menu registration should be an explicit optional app action documented for later testing. Do not alter the real user's folder icons, registry, shell settings or Explorer during this task. Include English UI/settings and a practical packaging strategy for bundled assets.

## Packaging and publication target

Windows standalone asset: `FolderIcons-vVERSION.exe` (for the initial release, `FolderIcons-v0.1.0.exe`). Append architecture only when distributing multiple executable architectures in the same release.
Planned canonical repository: https://github.com/siarheisinelnikau/folder-icons (not yet created or published; keep availability/error reporting truthful).

## Working agreement

Follow AGENTS.md. Complete as much real implementation and safe noninteractive verification as possible in this unattended code-only session. Manual UI, device and hardware tests will happen later. Record required external components and their verification status. Keep changes committed locally in meaningful increments.

