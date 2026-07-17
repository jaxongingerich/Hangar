# Hangar — Product Spec

A native macOS app for a solo founder who spins up product ideas constantly
(ESP32 hardware, React apps, PCBs, firmware). Hangar is a **project-centric
file manager**: every idea becomes a real folder on disk with organized,
nameable subdivisions ("bins") — Gerbers, JLCPCB, Firmware, CAD, Datasheets,
BOM, Photos, Docs — layered with logs, milestones, tasks, a rich Progress
workspace, orders/spend tracking, revision snapshots, and AI file organization
(cloud Claude + local LLMs).

**The one inviolable rule: the filesystem is the source of truth.** Everything
Hangar makes is a normal folder/file, fully usable in Finder. Hangar is the
index, metadata, UI, and AI layer on top — never a proprietary vault. Delete
the app and every file remains perfectly organized on disk.

## Tech stack (locked)

- Tauri 2.x (Rust) + React + TypeScript + Vite + Tailwind
- SQLite via rusqlite (bundled) with FTS5 — the DB is a rebuildable cache,
  never authoritative
- notify (FSEvents) · walkdir · blake3 · trash · zip · chrono
- axum localhost MCP server in-process
- keyring → API keys in macOS Keychain only
- Frontend: Zustand + TanStack Query, @tanstack/react-virtual, recharts,
  framer-motion, cmdk, chrono-node
- macOS 13+, Apple Silicon first; universal .dmg for release

## Architecture principles

1. **Disk is truth.** "Rebuild Index" fully reconstructs state from folders +
   sidecars.
2. **Sidecar metadata.** Each project folder holds `.hangar/project.json`
   (name, emoji/color, progress, milestones, tasks, bins, tags, links,
   deadline) + `.hangar/log.md`. Portable, git-friendly.
3. **Non-destructive.** No hard deletes — macOS Trash only. Every AI/file
   mutation = plan → approve → execute, journaled, 50-step undo stack.
4. **Offline-first.** Full functionality with zero AI configured.
5. **Fast & quiet.** <100 ms interactions, incremental indexing, debounced FS
   events, background scans off the main thread.

## Design language — "Hangar Deck"

Precision instrument: the calm of Linear, the speed of Raycast, the tactile
density of a pro audio tool.

**Palette — "Blueprint at night"**

| Token | Value | Use |
| --- | --- | --- |
| `--ink` | `#0B0E14` | base background |
| `--panel` | `#131822` | panels |
| `--panel-2` | `#1A2130` | raised panels |
| `--line` | `#232C3B` | hairlines |
| `--text` | `#E6EBF2` | primary text |
| `--muted` | `#8A97AC` | secondary text |
| solder | `#22D3A6` | signature accent — progress fills, active states, focus rings only |

Status ramp: idea `#7C8AA5` · active `#22D3A6` · at-risk `#F5A524` · late
`#F5556D` · shipped `#8B5CF6`.

**Typography.** Inter for UI (headings tracked `-0.02em`); JetBrains Mono for
every metric, %, size, date, hash — mono-for-data is part of the identity.
Scale 12/13/14/16/20/28/40, weights 400/500/600, sentence case everywhere.

**Layout.** Custom title bar with inset traffic lights; left icon rail
(Dashboard · Today · Inbox · Progress · Space · Settings) + collapsible
project sidebar + content. Flat panels, 1 px hairlines, radius 10 px, one soft
shadow max, 8 px grid. No glassmorphism, no gradients except the ring sweep.

**Signature elements.** The progress ring (status-colored sweep, count-up,
solder glow on milestone completion) and the spine (a 14-day activity tick
ribbon on each card — the project's heartbeat).

**Motion.** 150 ms ease-out transitions, layout animations, count-ups,
milestone pulse. Respect `prefers-reduced-motion`. No ambient motion.

**Craft floor.** Full keyboard nav with visible focus rings, ⌘K everywhere,
empty states that teach, Undo toast on every mutation, responsive to a narrow
window.

## Data model

SQLite tables: `projects`, `progress_history`, `bins`, `files`, `file_notes`,
`logs`, `milestones`, `tasks`, `links`, `orders`, `snapshots`, `time_entries`,
`tags`/`file_tags`/`project_tags`, `collections`, `rules`, `components`/
`component_uses`, `ai_runs`, `settings` — see `src-tauri/src/db.rs` for the
authoritative schema. `health` (on_track/at_risk/late) is derived from
target date vs. velocity and staleness. Every progress change writes
`progress_history`; every meaningful event writes an auto `logs` row.

## On-disk layout

```
~/Projects/                ← root, chosen on first launch; multi-root later
  _Inbox/  _Archive/
  Verdant-Pro-V1/
    .hangar/project.json  .hangar/log.md
    Gerbers/ JLCPCB/ Firmware/ CAD/ Datasheets/ BOM/ Photos/ Docs/
```

Bins are folders (rename = folder rename). One nesting level allowed.

## Features

**Files & capture** — F1 Dashboard card grid (ring, spine, health chip, file
count/size/last-touch, next due task, arriving-order badge, sort modes, board
grouping) · F2 Project detail (Files/Progress/Log/Orders/Links tabs; bins
rail, virtualized grid/list, thumbnails, Quick Look, drag-drop, multi-select,
inline rename, Reveal in Finder, pin, hover notes) · F3 Inbox & filing (rules
+ AI suggestions, "File all", rules editor with live tester) · F4 Logs (⌘L
notes + auto events, mirrored to `.hangar/log.md`) · F16 Finder tags
round-trip · F17 Watched folders (~/Downloads sweeps) · F18 Clipboard/quick
capture (⌘⇧V paste-as-file, global quick-log) · F23 Menu-bar quick capture.

**Progress workspace (the centerpiece)** — per-project Progress tab: animated
% ring, velocity Δ%/wk, projected finish vs. target with on-track/at-risk/late
badge, days-since-touch, hours-this-week; progress-over-time chart annotated
with milestones; weighted milestone kanban (Todo/Doing/Done, partial credit
from task checklists); tasks with natural dates, priority, blocked+reason,
recurrence; blockers strip; 26-week activity heatmap; drafted status reports.
Global Progress view (portfolio ranking) and Today view (morning screen).
Hardware milestone template ships: Idea → Schematic → PCB Layout → Gerbers Out
→ Boards In → Firmware Bring-up → Enclosure → App → Beta → Ship.

**Power** — F5 ⌘K search + command mode · F7 Space & health (treemap,
blake3 duplicate finder, stale projects, reclaim report, low-disk alert) · F8
templates · F10 export & archive (zip archive/restore, JLCPCB package with
layer validation, BOM normalize, one-pager PDF) · F11 revision snapshots +
file-level diff · F12 orders/spend tracker · F13 links hub + git badge · F14
file notes & pins · F15 smart collections · F19 light time tracking · F20
scheduled backups · F21 granular notifications · F22 multi-root · F24 Gerber
preview (tracespace) · F25 parts library · F26 global timeline · F27 idea
backlog (promote to project) · F28 health rollup · F29 AI Sync import (claude.ai
export → AI bin markdown) · F30 Context Bridge (build context bundles, chat
with any provider, save threads to AI bin) · F31 universal import (drag-drop
from Finder anywhere, copy-never-move, root-guarded, journaled).

## AI layer

One `AiProvider` trait (Rust) + TS mirror. Providers: Anthropic Messages API
(Keychain key, tool-use), Ollama (localhost:11434 autodetect), LM Studio /
OpenAI-compatible (custom base URL). Per-action provider override; monthly
token/cost meter from `ai_runs`.

Actions (each: dry-run plan → review sheet → approve → execute → journaled →
undoable): organize inbox · organize this mess · auto-tag · summarize/catch
me up · weekly digest · natural-language ops via ⌘K · auto-milestones · next
best action · status reports · project chat · smart rename scheme · AI Sync
import · Context Bridge send.

Guardrails: AI never hard-deletes (Trash proposals only), never touches paths
outside Hangar roots, never executes without approval unless auto-file is
explicitly enabled for rules-matched inbox items. All runs logged.

## MCP server

In-process axum server at `http://127.0.0.1:41748/mcp` (streamable HTTP,
bearer token in Settings) so Claude Code/Desktop can drive Hangar: full CRUD
toolset over projects, bins, files, tasks, milestones, orders, snapshots,
exports (see §11 of the original brief for the tool list). Mutating tools
return a plan by default and require `confirm=true` on a second call.

## Build phases

M0 skeleton → M1 core files → M2 progress suite → M3 space/export/revisions →
M4 power features → M5 AI → M6 MCP + polish. Each phase compiles, passes
tests, and launches before the next. Non-goals for v1: cloud sync,
multi-user, iOS, in-app file editing, full git client, Windows/Linux.

## Standards

Rust: clippy clean, thiserror, tracing. TS: strict, eslint+prettier. Tests:
Rust units for ops/rules/export validation/snapshot diff/progress math/health
derivation; vitest for stores; one happy-path e2e per milestone. Conventional
commits per milestone. Every judgment call goes in DECISIONS.md. All UI
derives color/type strictly from the Hangar Deck tokens.
