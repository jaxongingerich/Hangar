import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { formatAgo, formatBytes } from "../lib/format";
import { useToasts, useUi } from "../lib/store";

type Section = "usage" | "duplicates" | "archives" | "parts";

export function Space() {
  const [section, setSection] = useState<Section>("usage");
  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div className="flex items-center gap-3 px-6 pb-4 pt-5">
        <h1 className="text-[20px] font-semibold">Space</h1>
        <div className="ml-4 flex rounded-lg border border-line p-0.5">
          {(
            [
              ["usage", "Usage"],
              ["duplicates", "Duplicates"],
              ["archives", "Archives"],
              ["parts", "Parts library"],
            ] as [Section, string][]
          ).map(([s, label]) => (
            <button
              key={s}
              onClick={() => setSection(s)}
              className={`rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors ${
                section === s ? "bg-panel-2 text-text" : "text-muted hover:text-text"
              }`}
            >
              {label}
            </button>
          ))}
        </div>
      </div>
      {section === "usage" && <Usage />}
      {section === "duplicates" && <Duplicates />}
      {section === "archives" && <Archives />}
      {section === "parts" && <Parts />}
    </div>
  );
}

function Usage() {
  const { openProject } = useUi();
  const { data } = useQuery({ queryKey: ["space"], queryFn: api.spaceReport });
  if (!data) return null;
  const maxSize = Math.max(1, ...data.projects.map((p) => p.size_bytes));
  const stale = data.projects.filter((p) => (p.days_since_touch ?? 0) > 21);

  return (
    <div className="flex-1 overflow-y-auto px-6 pb-10">
      <div className="mb-5 flex gap-4 font-mono text-[11px] text-muted">
        <span>{formatBytes(data.total_bytes)} indexed</span>
        <span>{formatBytes(data.disk_free_bytes)} free on volume</span>
        {data.loose_root_files > 0 && (
          <span className="text-st-risk">{data.loose_root_files} loose files in root</span>
        )}
      </div>

      <div className="mb-6 flex flex-col gap-2">
        {data.projects.map((p) => (
          <button
            key={p.id}
            onClick={() => openProject(p.id)}
            className="rounded-panel border border-line bg-panel p-3 text-left transition-colors hover:border-[#2E3A4E]"
          >
            <div className="mb-1.5 flex items-center gap-2">
              <span>{p.emoji}</span>
              <span className="text-[13px] font-medium">{p.name}</span>
              <span className="font-mono text-[11px] text-muted">
                {formatBytes(p.size_bytes)} · {p.file_count} files
              </span>
              {(p.days_since_touch ?? 0) > 21 && (
                <span className="rounded-full bg-st-risk/15 px-2 py-0.5 text-[10px] text-st-risk">
                  stale {p.days_since_touch}d
                </span>
              )}
              {p.empty_bins.length > 0 && (
                <span className="ml-auto font-mono text-[10px] text-muted">
                  {p.empty_bins.length} empty bins
                </span>
              )}
            </div>
            {/* Proportional bin treemap strip */}
            <div className="flex h-4 w-full overflow-hidden rounded" style={{ opacity: 0.9 }}>
              {p.size_bytes > 0 ? (
                p.bins
                  .filter(([, s]) => s > 0)
                  .map(([name, size], i) => (
                    <div
                      key={name}
                      title={`${name} · ${formatBytes(size)}`}
                      style={{
                        width: `${Math.max(1, (size / maxSize) * 100)}%`,
                        background: p.color,
                        opacity: 1 - i * 0.13,
                      }}
                    />
                  ))
              ) : (
                <div className="w-full bg-panel-2" />
              )}
            </div>
          </button>
        ))}
      </div>

      {data.largest.length > 0 && (
        <section className="mb-6">
          <h2 className="mb-2 text-[13px] font-semibold text-muted">Largest files</h2>
          <div className="overflow-hidden rounded-panel border border-line">
            {data.largest.slice(0, 12).map((f) => (
              <div
                key={f.id}
                className="flex items-center gap-3 border-b border-line/50 bg-panel px-4 py-2 last:border-b-0"
              >
                <span className="min-w-0 flex-1 truncate text-[12px]">{f.name}</span>
                <span className="text-[11px] text-muted">{f.project_name}</span>
                <span className="w-20 text-right font-mono text-[11px]">
                  {formatBytes(f.size)}
                </span>
                <button
                  onClick={() => revealItemInDir(f.abs_path)}
                  className="text-[11px] text-muted hover:text-solder"
                >
                  reveal
                </button>
              </div>
            ))}
          </div>
        </section>
      )}

      {stale.length > 0 && (
        <p className="text-[12px] text-muted">
          💤 {stale.length} project{stale.length === 1 ? "" : "s"} untouched for
          3+ weeks — consider archiving from the project header.
        </p>
      )}
    </div>
  );
}

function Duplicates() {
  const qc = useQueryClient();
  const { push } = useToasts();
  const [ran, setRan] = useState(false);
  const dupes = useMutation({
    mutationFn: api.findDuplicates,
    onSuccess: () => setRan(true),
  });

  const trash = useMutation({
    mutationFn: (ids: number[]) => api.trashFiles(ids),
    onSuccess: (n) => {
      push(`${n} duplicate${n === 1 ? "" : "s"} → Trash`);
      dupes.mutate();
      qc.invalidateQueries({ queryKey: ["space"] });
    },
    onError: (e) => push(String(e), "error"),
  });

  const groups = dupes.data ?? [];
  const reclaimable = groups.reduce((s, g) => s + g.size * (g.files.length - 1), 0);

  return (
    <div className="flex-1 overflow-y-auto px-6 pb-10">
      <div className="mb-4 flex items-center gap-3">
        <button
          onClick={() => dupes.mutate()}
          disabled={dupes.isPending}
          className="rounded-lg bg-solder px-3.5 py-1.5 text-[12px] font-semibold text-ink disabled:opacity-50"
        >
          {dupes.isPending ? "Hashing…" : ran ? "Re-scan" : "Find duplicates"}
        </button>
        {ran && (
          <span className="font-mono text-[11px] text-muted">
            {groups.length} groups · {formatBytes(reclaimable)} reclaimable
          </span>
        )}
      </div>
      {ran && groups.length === 0 && (
        <p className="text-muted">No duplicates found. Tidy hangar. ✨</p>
      )}
      <div className="flex flex-col gap-3">
        {groups.map((g) => {
          // Safe delete keeps the first (newest kept upstream by sort) — here keep index 0.
          const extras = g.files.slice(1);
          return (
            <div key={g.hash} className="rounded-panel border border-line bg-panel p-3">
              <div className="mb-1.5 flex items-center gap-2 font-mono text-[11px] text-muted">
                <span>{formatBytes(g.size)}</span>
                <span>× {g.files.length}</span>
                <span className="truncate">{g.hash.slice(0, 16)}…</span>
                <button
                  onClick={() => trash.mutate(extras.map((f) => f.id))}
                  className="ml-auto rounded-md border border-line px-2 py-0.5 text-[11px] text-st-late hover:bg-st-late/10"
                >
                  Trash {extras.length} extra{extras.length === 1 ? "" : "s"}
                </button>
              </div>
              {g.files.map((f, i) => (
                <div key={f.id} className="flex items-center gap-2 py-0.5 text-[12px]">
                  <span className={i === 0 ? "text-solder" : "text-muted"}>
                    {i === 0 ? "keep" : "dupe"}
                  </span>
                  <span className="truncate">{f.project_name} / {f.rel_path}</span>
                  <button
                    onClick={() => revealItemInDir(f.abs_path)}
                    className="ml-auto text-[11px] text-muted hover:text-solder"
                  >
                    reveal
                  </button>
                </div>
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function Archives() {
  const qc = useQueryClient();
  const { push } = useToasts();
  const { data: archives } = useQuery({ queryKey: ["archives"], queryFn: api.listArchives });

  const restore = useMutation({
    mutationFn: api.restoreArchive,
    onSuccess: () => {
      push("Project restored from archive");
      qc.invalidateQueries();
    },
    onError: (e) => push(String(e), "error"),
  });

  return (
    <div className="flex-1 overflow-y-auto px-6 pb-10">
      {(archives ?? []).length === 0 ? (
        <p className="mt-8 text-center text-muted">
          No archives yet. Archive a project from its header — it becomes a zip
          in <span className="font-mono">_Archive/</span> and the folder moves
          to Trash.
        </p>
      ) : (
        <div className="overflow-hidden rounded-panel border border-line">
          {(archives ?? []).map((a) => (
            <div
              key={a.path}
              className="flex items-center gap-3 border-b border-line/50 bg-panel px-4 py-2.5 last:border-b-0"
            >
              <span>🗜️</span>
              <span className="min-w-0 flex-1 truncate text-[12.5px]">{a.name}</span>
              <span className="font-mono text-[11px] text-muted">{formatBytes(a.size)}</span>
              <span className="font-mono text-[11px] text-muted">{formatAgo(a.created_ms)}</span>
              <button
                onClick={() => restore.mutate(a.path)}
                disabled={restore.isPending}
                className="rounded-md border border-line px-2.5 py-1 text-[11px] text-muted hover:border-solder hover:text-solder disabled:opacity-50"
              >
                Restore
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function Parts() {
  const [query, setQuery] = useState("");
  const [mpn, setMpn] = useState("");
  const [lcsc, setLcsc] = useState("");
  const [desc, setDesc] = useState("");
  const [pkg, setPkg] = useState("");
  const qc = useQueryClient();

  const { data: parts } = useQuery({
    queryKey: ["components", query],
    queryFn: () => api.listComponents(query || undefined),
  });

  const add = async () => {
    if (!mpn.trim()) return;
    await api.saveComponent({
      mpn: mpn.trim(),
      lcsc: lcsc.trim() || undefined,
      description: desc.trim() || undefined,
      package: pkg.trim() || undefined,
    });
    setMpn(""); setLcsc(""); setDesc(""); setPkg("");
    qc.invalidateQueries({ queryKey: ["components"] });
  };

  const input =
    "rounded-md border border-line bg-panel-2 px-2.5 py-1.5 text-[12px] placeholder:text-muted focus:border-solder focus:outline-none";

  return (
    <div className="flex-1 overflow-y-auto px-6 pb-10">
      <div className="mb-3 flex gap-2">
        <input
          className={`${input} w-64`}
          placeholder="Search MPN, LCSC, value…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>
      <div className="mb-4 flex gap-2">
        <input className={`${input} w-44`} placeholder="MPN *" value={mpn} onChange={(e) => setMpn(e.target.value)} />
        <input className={`${input} w-28`} placeholder="LCSC (C123…)" value={lcsc} onChange={(e) => setLcsc(e.target.value)} />
        <input className={`${input} w-28`} placeholder="Package" value={pkg} onChange={(e) => setPkg(e.target.value)} />
        <input className={`${input} flex-1`} placeholder="Description" value={desc} onChange={(e) => setDesc(e.target.value)} />
        <button
          onClick={add}
          disabled={!mpn.trim()}
          className="rounded-md bg-solder px-3 py-1.5 text-[12px] font-semibold text-ink disabled:opacity-40"
        >
          Add part
        </button>
      </div>
      <div className="overflow-hidden rounded-panel border border-line">
        {(parts ?? []).length === 0 ? (
          <p className="bg-panel px-4 py-6 text-center text-[12px] text-muted">
            Your personal parts library — MPNs and LCSC numbers you actually
            use, reusable across every board.
          </p>
        ) : (
          (parts ?? []).map((c) => (
            <div
              key={c.id}
              className="group flex items-center gap-3 border-b border-line/50 bg-panel px-4 py-2.5 last:border-b-0"
            >
              <span className="w-40 truncate font-mono text-[12px]">{c.mpn}</span>
              {c.lcsc && <span className="font-mono text-[11px] text-solder">{c.lcsc}</span>}
              {c.package && <span className="font-mono text-[11px] text-muted">{c.package}</span>}
              <span className="min-w-0 flex-1 truncate text-[12px] text-muted">
                {c.description}
              </span>
              {c.used_in.length > 0 && (
                <span className="text-[10px] text-muted" title={c.used_in.join(", ")}>
                  used in {c.used_in.length}
                </span>
              )}
              <button
                onClick={async () => {
                  await api.deleteComponent(c.id);
                  qc.invalidateQueries({ queryKey: ["components"] });
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
  );
}
