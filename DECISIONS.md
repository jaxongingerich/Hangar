# Decisions

Judgment calls made while building Hangar, newest first.

## M4

- **Multi-root (F22) deferred to post-v1.** Every file mutation is
  root-guarded; making that "any of N roots" safely touches all ops and the
  watcher lifecycle. Single root ships first; the settings shape (`root` key)
  leaves room for a `roots` list later.
- **Finder tags via `mdls` (read) + `plutil`/`xattr` (write)** — on-demand per
  file from the toolbar, not a bulk round-trip during scans, to keep scans
  fast and avoid touching xattrs of files the user never asked about.
- **Global quick capture is ⌘⇧H (raise Hangar).** The dedicated mini-window
  arrives with the tray in M6; a system-wide raise shortcut covers the
  workflow until then.
- **Backups zip the whole root** (excluding nothing — sidecars included) with
  an open-and-verify pass; pruning keeps the newest N by filename timestamp.

## M3

- **One-pager exports as print-ready HTML** (open → ⌘P → PDF) rather than
  binding a PDF library; identical output, zero heavy dependencies.
- **Global rules suggest bins by bin *name*** (built-ins) while user rules
  target concrete `dest_bin_id`s; a global rule can't point at a per-project
  bin id, so name-matching is the portable fallback.
- **Snapshot zips live in `.hangar/snapshots/`** inside the project — they
  travel with the folder (git-ignorable), and "delete the app" still leaves
  every revision on disk.

## M2

- **"Doing" milestones with no tasks earn 50% credit** in weighted progress —
  zero would hide started work, full credit would lie forward.
- **Health**: late = past target and unfinished; at-risk = projection past
  target, or 14+ days untouched (21+ with no target). Non-active projects are
  never flagged.
- **Recurrence** is a simple daily/weekly/monthly respawn on completion,
  keyed off the due date — no RRULE engine for v1.

## M1

- **Idea backlog is its own table** (`ideas`), not a `projects` row — ideas
  have no folder, and the scanner treats folder-less project rows as deleted.
  Promotion creates the real folder + template.
- **Watcher debounce**: absorb event bursts until 700 ms of quiet, then one
  full rescan. Simple, correct, and comfortably under the "<2 s to reflect
  Finder changes" bar.

## M0

- **React 19 instead of 18.** `create-tauri-app` ships React 19; it is fully
  compatible with every locked library (zustand, TanStack Query/Virtual,
  framer-motion, recharts, cmdk). Staying current beats pinning back.
- **Scanner skips build junk.** Firmware/app bins will contain whole repos, so
  the indexer skips `node_modules`, `.git`, `target`, `dist`, `build`,
  `__pycache__`, `.venv`, `.next`, `.cache`, `DerivedData` and hidden dirs.
  Indexing those would bloat the DB with files nobody manages by hand.
- **Deterministic project colors.** New projects get a color from a 7-color
  palette keyed on a hash of the name, so rescans and rebuilds always assign
  the same color without storing extra state.
- **Sidecar is authoritative on scan.** A full scan overwrites DB project rows
  from `.hangar/project.json` (disk is truth). App mutations write the sidecar
  first, then the DB.
- **File identity per scan uses an `indexed_at` token.** Each scan stamps
  touched rows with a fresh token and deletes rows without it — a simple,
  correct "remove vanished files" pass without diffing sets in memory.
- **Fonts bundled via @fontsource** (Inter Variable, JetBrains Mono Variable)
  so the app never depends on network or system fonts.
