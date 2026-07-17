# Decisions

Judgment calls made while building Hangar, newest first.

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
