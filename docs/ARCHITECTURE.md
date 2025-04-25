# Architecture and recovery

## Modules and boundaries

`render` draws the original folder silhouette, color gradients, badges and optional photo into an RGBA buffer. Themes are source-defined colors and analytic geometry, embedded in the EXE. PNG/JPEG decoding is bounded to 25 MB input, 12,000 pixels per dimension and 128 MB decoder allocation. A photo is cropped to a small cached image; multi-size ICO frames are PNG encoded.

`ini` parses the text line by line. Only `IconResource`, `IconFile` and `IconIndex` entries in the case-insensitive `.ShellClassInfo` section are replaced. Unrelated section/key/comment text is retained. Serialization uses UTF-16LE with BOM. Restore uses the backed-up bytes, not a reconstructed INI. Embedded NULs and ambiguous section headers are refused before any writes.

`engine` has no dialogs, registry writes, network calls or shell notifications. Plan creation reads bounded files and captures the canonical folder path, current INI bytes/attributes, folder attributes and current recovery manifest. Execution takes a per-folder exclusive create-new lock, rechecks those snapshots, and refuses a stale plan. Plans are not persisted between app launches. The UI accepts at most 128 selected folders.

`platform` isolates UTF-16 Win32 paths, file attributes, atomic same-directory replacement, targeted shell notifications and opt-in HKCU context menu registration. The engine's tests modify attributes only on disposable fixture folders. They never notify Explorer or register the context menu. Read-only core planning/rendering logic is portable; applying appearance is supported only through the Windows application.

`settings` writes the startup preference atomically outside the installation directory. Missing settings use startup checks enabled. Malformed/unreadable settings disable automatic requests for the session and show a diagnostic rather than discarding the persisted preference silently.

`updates` parses semantic versions, excludes drafts/prereleases, validates canonical asset URLs, bounds network operations, verifies downloads and launches the deferred replacement helper only after an explicit user action. The helper confirms readiness after acquiring a handle to the parent process, preventing an exit/wait race. If installation fails, a sibling diagnostic is written and the preserved/replaced application is reopened when possible to show that diagnostic. Replacing the installed executable never touches settings or customized folders.

## Threads and window lifecycle

The main thread runs the eframe 0.37 event-driven UI and processes compact result events. File selection, bounded rendering/image decode, plan creation, filesystem mutations, settings and optional registry changes run on one serial file worker. Network checks/downloads run on a separate serial network worker. Queues are bounded. Workers block on channel receive while idle and explicitly request a UI wake after results. Preview edits use a short debounce and generation IDs; stale render results cannot become the current Apply icon. Loading a photo is cached until the path changes/removal.

eframe 0.37 was selected from the available cached dependencies after inspecting its redraw scheduling: it avoids the `ControlFlow::Poll` path present in eframe 0.33.3. This is source-level reasoning, not an idle CPU measurement. There is no tray/hide-on-close behavior. Close is deferred during file operations or update installation so they can finish; the update worker closes the UI when its helper is ready. Results are also processed in `App::logic` when minimized. Performance, dialogs, rendering and restoration still need real-window verification.

## On-folder layout

```text
selected-folder/
  desktop.ini                     # UTF-16LE, hidden + system during customization
  .foldericons.lock                # temporary lock, removed at end of an operation
  .foldericons/                    # hidden after successful Apply
    manifest.json                 # product/schema, exact original bytes + attrs, journal
    <sha256>.ico                  # immutable content-addressed generated assets
```

The INI references `.foldericons\<sha256>.ico,0` using a relative path. Generated icons are not stored in a volatile global cache. Previously generated icons remain until Restore, avoiding deletion of files Explorer might still have cached. The manifest holds original data directly as JSON byte arrays for transparent recovery, with a bounded read limit. It may contain localized names/infotips and should be treated as user data when sharing folders.

## Apply protocol

1. Reject reparse/symlink paths and drive roots; check snapshots and acquire the per-folder lock. Another live FolderIcons instance must use the same lock.
2. Create/verify the hash-named ICO. Refuse an existing file with different content. Refuse an unmanaged store.
3. Atomically write/sync the journal containing the original INI bytes/attributes, prior INI and proposed INI. The first original backup persists across subsequent Apply operations.
4. Atomically replace `desktop.ini`, retaining unrelated settings; mark it hidden/system. Add the folder read-only bit so Explorer enables customization. Hide the generated store.
5. Finalize the journal so only the applied INI is accepted during ordinary future operations. The UI sends a targeted Explorer refresh after success.

An error after the journal write retains the backup. A pending journal accepts either the prior or proposed INI so the user can retry or Restore. If a crash occurs before a first journal exists, appearance has not yet been changed, but the generated unmanaged store may require manual inspection/removal. Stale temporary files/locks after a crash are reported instead of guessed away.

## Restore protocol

1. Verify current INI against the committed/pending journal. Refuse arbitrary outside edits.
2. Mark the journal as restoring before changing files. This allows retry after restoring the INI but before cleanup finishes.
3. Restore the exact original bytes, or remove `desktop.ini` if it did not exist originally. Restore original INI attributes. Revert only the folder read-only bit owned by this app, preserving unrelated later folder attribute changes.
4. Verify hashes and remove only recorded icon files. Remove the manifest last; remove the store only if empty. Foreign files are never recursively removed. A leftover store with foreign files must be manually reconciled before a new Apply.

Atomic file replacement is not a multi-file or cross-folder transaction. Changes by external processes between final validation and writes cannot be fully excluded. Windows ACLs, alternate streams and file IDs are not backed up; the preservation guarantee concerns INI content/settings and relevant appearance attributes. Replacing an INI may inherit directory security. Do not use on specialized managed folders until manually validated.

## Primary references

- [Microsoft: customizing folders with Desktop.ini](https://learn.microsoft.com/en-us/windows/win32/shell/how-to-customize-folders-with-desktop-ini)
- [Microsoft: SetFileAttributesW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileattributesw)
- [eframe 0.37 native redraw implementation](https://docs.rs/crate/eframe/0.37.0/source/src/native/run.rs)
- [ureq 3.5 API and timeouts](https://docs.rs/ureq/3.5.0/ureq/)
