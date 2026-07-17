import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../../lib/api";

const KINDS = ["repo", "store", "order", "datasheet", "doc", "other"] as const;
const KIND_ICON: Record<string, string> = {
  repo: "⌥",
  store: "🛍",
  order: "📦",
  datasheet: "📕",
  doc: "📄",
  other: "🔗",
};

export function LinksTab({ projectId }: { projectId: number }) {
  const [title, setTitle] = useState("");
  const [url, setUrl] = useState("");
  const [kind, setKind] = useState<string>("repo");
  const qc = useQueryClient();

  const { data: links } = useQuery({
    queryKey: ["links", projectId],
    queryFn: () => api.listLinks(projectId),
  });
  const { data: badge } = useQuery({
    queryKey: ["gitbadge", projectId],
    queryFn: () => api.gitBadge(projectId),
  });

  const invalidate = () => qc.invalidateQueries({ queryKey: ["links", projectId] });

  const add = async () => {
    if (!url.trim()) return;
    await api.addLink(projectId, title.trim() || url.trim(), url.trim(), kind);
    setTitle("");
    setUrl("");
    invalidate();
  };

  const input =
    "rounded-md border border-line bg-panel-2 px-2.5 py-1.5 text-[12px] placeholder:text-muted focus:border-solder focus:outline-none";

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[720px] px-6 py-5">
        {badge && (
          <div className="mb-4 flex items-center gap-2 rounded-panel border border-line bg-panel px-4 py-2.5">
            <span className="font-mono text-[12px] text-solder"> {badge.branch}</span>
            <span
              className={`rounded-full px-2 py-0.5 font-mono text-[10px] ${
                badge.dirty ? "bg-st-risk/15 text-st-risk" : "bg-solder/15 text-solder"
              }`}
            >
              {badge.dirty ? "dirty" : "clean"}
            </span>
            <span className="text-[11px] text-muted">project folder is a git repo</span>
          </div>
        )}

        <div className="mb-4 flex gap-2">
          <select value={kind} onChange={(e) => setKind(e.target.value)} className={input}>
            {KINDS.map((k) => (
              <option key={k} value={k}>
                {k}
              </option>
            ))}
          </select>
          <input
            className={`${input} w-40`}
            placeholder="Title"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />
          <input
            className={`${input} flex-1`}
            placeholder="https://…"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && add()}
          />
          <button
            onClick={add}
            disabled={!url.trim()}
            className="rounded-md bg-solder px-3 py-1.5 text-[12px] font-semibold text-ink disabled:opacity-40"
          >
            Add
          </button>
        </div>

        <div className="overflow-hidden rounded-panel border border-line">
          {(links ?? []).length === 0 ? (
            <p className="bg-panel px-4 py-6 text-center text-[12px] text-muted">
              Pin the links that matter — repo, Shopify listing, JLC order
              pages, datasheets. They're all reachable from ⌘K too.
            </p>
          ) : (
            (links ?? []).map((l) => (
              <div
                key={l.id}
                className="group flex items-center gap-3 border-b border-line/50 bg-panel px-4 py-2.5 last:border-b-0"
              >
                <span className="w-5 text-center text-[13px]">{KIND_ICON[l.kind]}</span>
                <button
                  onClick={() => openUrl(l.url)}
                  className="min-w-0 flex-1 truncate text-left text-[12.5px] hover:text-solder"
                >
                  {l.title}
                </button>
                <span className="max-w-[240px] truncate font-mono text-[10px] text-muted">
                  {l.url}
                </span>
                <button
                  onClick={async () => {
                    await api.deleteLink(l.id);
                    invalidate();
                  }}
                  className="hidden text-[11px] text-muted hover:text-st-late group-hover:block"
                >
                  ✕
                </button>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
