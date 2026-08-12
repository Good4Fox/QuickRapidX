# Changelog

Notable changes to QuickRapidX. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Until 1.0 the minor number carries breaking changes: the app is in active
development and its shape is still moving.

## [Unreleased]

## [0.5.0]

The release where the app stopped being a shell of the old one. The interface
came over from the previous version; the machinery underneath is new.

### Added

- **Sets** — programs, folders and links gathered into one list, opened from a
  tray icon or a desktop shortcut. Sets nest inside each other, carry their own
  icons, captions and tooltips, and lay out as a row or a column per set.
- **Found on this machine** — a sweep across four sources at once: the uninstall
  registry, Store packages, developer tools on PATH, and portable programs no
  installer ever recorded. Search, filter by kind, launch, open the folder.
- **Storage** — a folder declared as a Place, holding installers and portable
  builds. A Place can be moved to another drive or carried away whole.
- **Isolation** *(experimental)* — running a program with cut-down rights across
  three engines: a built-in one on AppContainer, Windows Sandbox, and Sandboxie.
  Per-program permissions, roles, and snapshots of the data folder.
- **Recycle Bin** — a separate tray icon with five fill levels and custom icons
  per level, size limits set per drive as a share of the disk or a value of your
  own, and the shell's own confirmation, sound and progress switches.
- **Recovery** — component store checks and repair through DISM, Windows Sandbox
  reinstall, and removal of `Windows.old` including the parts ordinary deletion
  refuses to touch.
- **Catalog** — your own record of a program: name, link, winget id, icon and
  description, grouped by role.
- **First-run wizard** — language, what you use the machine for, theme, and a few
  Windows settings worth having on day one.
- Japanese alongside English and Russian. The app picks the language from the
  system on the first run.

### Changed

- Icons are kept in one append-only pack file instead of a tree of folders, and
  handed to the interface through a custom uri scheme — the picture itself never
  crosses into the page as base64 any more.
- Hidden windows are put to sleep rather than kept in memory: the working set of
  the app dropped from about 694 MB to 86 MB.
- Every string in the interface goes through the dictionary; sizes, dates and
  relative time follow the chosen language too.

### Fixed

- Reordering entries in the set window: the first tile could not be dragged back
  to the first place, and the tile lagged behind the pointer by a factor of nine.
- Icons in the set window went missing after the window had been put to sleep
  once — a failed request was remembered as final and never retried.
- Removal of `Windows.old` walked into junctions and reparse points instead of
  deleting them.
- Programs from the Microsoft Store are now refused by the isolation engines with
  an explanation, instead of starting an empty sandbox: those are redirection
  points, not programs.

[Unreleased]: https://github.com/Good4Fox/QuickRapidX/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/Good4Fox/QuickRapidX/releases/tag/v0.5.0
