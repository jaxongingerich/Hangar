import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { api, DiscoveredSession } from "../../lib/api";
import { useToasts } from "../../lib/store";

const ghostBtn =
  "rounded-md border border-line px-2.5 py-1 text-[11.5px] font-medium text-muted transition-colors hover:border-solder hover:text-solder";
const primaryBtn =
  "rounded-md bg-solder px-3 py-1.5 text-[12px] font-semibold text-on-accent transition-opacity hover:opacity-90 disabled:opacity-40";

/** Display names for the places conversations can come from. */
const SOURCE_LABELS: Record<string, string> = {
  "claude-code": "Claude Code",
  codex: "Codex",
  "claude-export": "Claude (export)",
  "chatgpt-export": "ChatGPT (export)",
  hangar: "Hangar",
};

export function sourceLabel(source: string | undefined): string {
  if (!source) return "Hangar";
  return SOURCE_LABELS[source] ?? source;
}

function timeAgo(iso: string): string {
  if (!iso) return "";
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return "";
  const days = Math.floor((Date.now() - then) / 86_400_000);
  if (days <= 0) return "today";
  if (days === 1) return "yesterday";
  if (days < 30) return `${days}d ago`;
  if (days < 365) return `${Math.floor(days / 30)}mo ago`;
  return `${Math.floor(days / 365)}y ago`;
}

/**
 * Pulls past conversations into Hangar from three places:
 *
 *  - Claude Code and Codex session files already on this Mac
 *  - Claude / ChatGPT official data exports (the only supported route to cloud
 *    conversations — neither desktop app stores them locally and neither vendor
 *    exposes a history API)
 *  - CLI bridges that aren't installed yet, offered as a one-click install
 */
export function ImportPanel({
  profileId,
  onImported,
}: {
  profileId: string | null;
  onImported: () => void;
}) {
  const qc = useQueryClient();
  const { push } = useToasts();
  const [selected, setSelected] = useState<Set<string>>(new Set());

  const { data: sessions, isLoading } = useQuery({
    queryKey: ["aiSessions"],
    queryFn: api.aiDiscoverSessions,
  });
  const { data: bridges } = useQuery({
    queryKey: ["aiBridges"],
    queryFn: api.aiCliBridgeStatus,
  });

  const available = useMemo(
    () => (sessions ?? []).filter((s) => !s.imported),
    [sessions],
  );
  const alreadyIn = (sessions ?? []).length - available.length;

  // Group by source so the list reads as "here's what each AI has".
  const grouped = useMemo(() => {
    const m = new Map<string, DiscoveredSession[]>();
    for (const s of available) {
      const list = m.get(s.source) ?? [];
      list.push(s);
      m.set(s.source, list);
    }
    return [...m.entries()].sort((a, b) => b[1].length - a[1].length);
  }, [available]);

  const key = (s: DiscoveredSession) => `${s.source}:${s.id}`;
  const toggle = (s: DiscoveredSession) =>
    setSelected((prev) => {
      const next = new Set(prev);
      const k = key(s);
      next.has(k) ? next.delete(k) : next.add(k);
      return next;
    });

  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["aiSessions"] });
    qc.invalidateQueries({ queryKey: ["aiChats"] });
  };

  const importSelected = useMutation({
    mutationFn: async (list: DiscoveredSession[]) => {
      if (list.length === 0) throw new Error("Pick at least one conversation");
      return api.aiImportSessions(list, profileId);
    },
    onSuccess: (r) => {
      push(
        `Imported ${r.imported} chat${r.imported === 1 ? "" : "s"} · ${r.messages} messages` +
          (r.skipped ? ` (${r.skipped} already in)` : ""),
        "info",
      );
      if (r.errors.length) push(r.errors[0], "error");
      setSelected(new Set());
      refresh();
      onImported();
    },
    onError: (e) => push(String(e), "error"),
  });

  const importExport = useMutation({
    mutationFn: async () => {
      const picked = await open({
        multiple: false,
        filters: [{ name: "Conversations export", extensions: ["json"] }],
      });
      if (!picked || typeof picked !== "string") return null;
      return api.aiImportExportFile(picked, profileId);
    },
    onSuccess: (r) => {
      if (!r) return; // user cancelled the picker
      push(`Imported ${r.imported} conversations · ${r.messages} messages`, "info");
      refresh();
      onImported();
    },
    onError: (e) => push(String(e), "error"),
  });

  const installBridge = useMutation({
    mutationFn: (command: string) => api.aiInstallCliBridge(command),
    onSuccess: (msg) => {
      push(`${msg} — your past chats can now be imported`, "info");
      qc.invalidateQueries({ queryKey: ["aiBridges"] });
      refresh();
    },
    onError: (e) => push(String(e), "error"),
  });

  const missingBridges = (bridges ?? []).filter((b) => !b.installed);

  return (
    <div className="flex-1 overflow-y-auto px-5 py-4">
      <div className="mb-4 flex items-center gap-2">
        <h2 className="text-[14px] font-semibold">Import past conversations</h2>
        <button onClick={refresh} className={`ml-auto ${ghostBtn}`}>
          Rescan
        </button>
      </div>

      {/* CLI bridges that would unlock more history if installed */}
      {missingBridges.map((b) => (
        <div
          key={b.command}
          className="mb-3 rounded-lg border border-line bg-panel px-3.5 py-3"
        >
          <div className="text-[12.5px] font-medium">
            {b.command === "codex" ? "Codex" : "Claude Code"} CLI isn't installed
          </div>
          <p className="mt-1 text-[11.5px] leading-relaxed text-muted">
            {b.has_history
              ? `You have ${b.command === "codex" ? "Codex" : "Claude"} history on this Mac, but the CLI that lets Hangar talk to it is missing. Installing it enables sending messages too.`
              : `Installing it lets Hangar chat with ${b.command === "codex" ? "ChatGPT/Codex" : "Claude"} using your existing login — no API key.`}
          </p>
          <div className="mt-2 flex items-center gap-2">
            <button
              onClick={() => installBridge.mutate(b.command)}
              disabled={installBridge.isPending}
              className={primaryBtn}
            >
              {installBridge.isPending ? "Installing…" : `Install ${b.command}`}
            </button>
            <code className="text-[10.5px] text-muted">{b.install_hint}</code>
          </div>
        </div>
      ))}

      {/* Cloud exports */}
      <div className="mb-4 rounded-lg border border-line bg-panel px-3.5 py-3">
        <div className="text-[12.5px] font-medium">
          Claude or ChatGPT app conversations
        </div>
        <p className="mt-1 text-[11.5px] leading-relaxed text-muted">
          The Claude and ChatGPT desktop apps keep conversations on their servers
          and store nothing on this Mac, so Hangar can't read them directly.
          Request your data export from either service, then load the{" "}
          <code>conversations.json</code> it emails you.
        </p>
        <button
          onClick={() => importExport.mutate()}
          disabled={importExport.isPending}
          className={`mt-2 ${primaryBtn}`}
        >
          {importExport.isPending ? "Importing…" : "Choose export file…"}
        </button>
      </div>

      {/* Sessions found on disk */}
      {isLoading ? (
        <p className="text-[12px] text-muted">Looking for past conversations…</p>
      ) : available.length === 0 ? (
        <p className="text-[12px] leading-relaxed text-muted">
          {alreadyIn > 0
            ? `All ${alreadyIn} conversations found on this Mac are already imported.`
            : "No past conversations found on this Mac yet."}
        </p>
      ) : (
        <>
          <div className="mb-2 flex items-center gap-2">
            <span className="text-[11.5px] text-muted">
              {available.length} found on this Mac
              {alreadyIn > 0 && ` · ${alreadyIn} already imported`}
            </span>
            <button
              onClick={() =>
                setSelected(
                  selected.size === available.length
                    ? new Set()
                    : new Set(available.map(key)),
                )
              }
              className={`ml-auto ${ghostBtn}`}
            >
              {selected.size === available.length ? "Clear" : "Select all"}
            </button>
            <button
              onClick={() =>
                importSelected.mutate(
                  available.filter((s) => selected.has(key(s))),
                )
              }
              disabled={importSelected.isPending || selected.size === 0}
              className={primaryBtn}
            >
              {importSelected.isPending
                ? "Importing…"
                : `Import ${selected.size || ""}`.trim()}
            </button>
          </div>

          {grouped.map(([source, list]) => (
            <div key={source} className="mb-4">
              <div className="mb-1 px-0.5 text-[11px] font-semibold uppercase tracking-wide text-muted">
                {sourceLabel(source)} · {list.length}
              </div>
              {list.map((s) => (
                <label
                  key={key(s)}
                  className="mb-0.5 flex cursor-pointer items-center gap-2.5 rounded-md px-2.5 py-2 hover:bg-panel"
                >
                  <input
                    type="checkbox"
                    checked={selected.has(key(s))}
                    onChange={() => toggle(s)}
                    className="shrink-0"
                  />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[12.5px]">{s.title}</span>
                    <span className="mt-0.5 block text-[10.5px] text-muted">
                      {s.message_count} msgs · {timeAgo(s.started_at)}
                      {s.cwd && ` · ${s.cwd.split("/").slice(-1)[0]}`}
                    </span>
                  </span>
                </label>
              ))}
            </div>
          ))}
        </>
      )}
    </div>
  );
}
