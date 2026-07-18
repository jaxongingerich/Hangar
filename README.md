# Hangar

A native macOS app for makers who spin up product ideas constantly — ESP32
hardware, React apps, PCBs, firmware. Hangar is a **project-centric file
manager**: every idea becomes a real folder on disk with organized bins, layered
with logs, milestones, tasks, a rich Progress workspace, orders/spend tracking,
revision snapshots, and AI file organization.

New projects start with general bins (Docs, Files, Photos, Notes, Exports).
Specialized hardware bins — Gerbers, BOM, JLCPCB, Firmware, CAD, Datasheets — are
one click away via the Hardware template or the suggestion chips under "New bin",
so a project never starts cluttered with folders it may never use.

**The one inviolable rule: the filesystem is the source of truth.** Everything
Hangar makes is a normal folder/file, fully usable in Finder. Delete the app
and every file remains perfectly organized on disk.

See `SPEC.md` for the full product spec and `DECISIONS.md` for judgment calls.

## Highlights

- **Dashboard** — project cards with progress rings, 14-day activity spines,
  health chips, and an idea backlog.
- **Progress workspace** — weighted milestone kanban, tasks with natural-language
  dates, velocity and projected-finish math, activity heatmap, status reports.
- **Inbox & filing** — drop files anywhere; rules and AI suggestions file them
  into the right project bins. Copy-never-move, journaled, undoable.
- **Space** — treemap, blake3 duplicate finder, stale projects, archives.
- **AI tab** — connect any number of AIs, two ways:
  - **No key needed** — Hangar auto-detects AI CLIs already signed in on your
    Mac (Claude Code, codex, gemini, `llm`, sgpt) and local servers (Ollama,
    LM Studio). One click to connect; it talks to the tool you already use.
    The ChatGPT and Claude *desktop* apps expose no local API, so Hangar can't
    talk to them directly — if it finds one installed it shows a "Setup needed"
    card with the exact command to install its CLI bridge (`codex`, `claude`).
  - **With a key** — Claude, OpenAI/Codex, Hermes, OpenRouter, Groq, Together,
    Fireworks, Cerebras, DeepSeek, Perplexity, Mistral, Grok, or any
    OpenAI-compatible server. Keys live in the macOS Keychain.
  Saved chats, per-project context, file attachments, and mid-conversation
  model switching — the whole history carries over between AIs.
- **AI actions** — filing, summaries, milestones, renames. Every action is
  plan → approve → execute, never destructive.
- **MCP server** — optional (Settings toggle): `http://127.0.0.1:41748/mcp`
  lets Claude Code/Desktop drive Hangar with a full toolset. Local-only,
  bearer-token protected, and runs inside the app — no terminal involved. The
  shipped `.app` never opens a terminal window, at launch or on quit.

## Installing

Download the latest `.dmg` from the [Releases page](../../releases), open it, and
drag Hangar to Applications.

**Requires an Apple Silicon Mac (M1 or newer).** The current build is `arm64`
only and will not launch on Intel Macs.

**First launch:** the app is adhoc-signed, not notarized with an Apple Developer
ID, so macOS Gatekeeper blocks a normal double-click the first time. **Right-click
the app and choose Open**, then confirm in the dialog. macOS remembers the choice
and it opens normally from then on. This is expected for unnotarized apps and
isn't a sign anything is wrong.

## Keyboard

| Keys | Action |
| --- | --- |
| ⌘K | Search & commands |
| ⌘1–⌘7 | Jump to Projects / Today / Inbox / AI / Progress / Space / Settings |
| ⌘N | New project |
| ⌘Z | Undo last file operation |
| Esc | Back to Projects (from a project) |
| ⌘⇧H | Raise Hangar from anywhere (global) |

## Development

Prereqs: Rust (stable), Node 20+, Xcode CLT.

```sh
npm install
npm run tauri dev                    # run the app
npm run tauri build -- --bundles app # produce Hangar.app
```

**Don't run a bare `npm run tauri build`.** Its DMG step (`bundle_dmg.sh`) drives
Finder over AppleScript to lay out the disk-image window, which cannot work from
a headless or background shell — it hangs indefinitely, and killing the AppleScript
aborts the whole build (`set -e`, `exit 64`). Build the `.app` only, then package
the DMG yourself:

```sh
codesign --remove-signature Hangar.app
codesign --force --deep --sign - Hangar.app
codesign --verify --deep --strict Hangar.app
hdiutil create -volname Hangar -srcfolder <stage> -ov \
  -format UDZO -imagekey zlib-level=9 Hangar.dmg
```

where `<stage>` is a folder holding `Hangar.app` plus a `ln -s /Applications`
symlink.

- Frontend: React + TypeScript + Vite + Tailwind (`src/`)
- Backend: Tauri 2 + Rust + SQLite/FTS5 (`src-tauri/`)
- Icon: edit `src-tauri/icons/source.svg`, then
  `npm run tauri icon src-tauri/icons/source.svg -- -o src-tauri/icons`
  (Claude-light theme: ivory squircle, coral arch, transparent corners)

## Tests

```sh
cargo test --manifest-path src-tauri/Cargo.toml
npx vitest run
```
