import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../lib/api";
import { formatAgo, formatBytes } from "../lib/format";
import { fileIcon } from "../lib/icons";
import { useToasts } from "../lib/store";

export function Inbox() {
  const [targetProject, setTargetProject] = useState<number | null>(null);
  // Per-item bin overrides (path → bin id or null for project root).
  const [overrides, setOverrides] = useState<Record<string, number | null>>({});
  const qc = useQueryClient();
  const { push } = useToasts();

  const { data: projects } = useQuery({
    queryKey: ["projects"],
    queryFn: api.listProjects,
  });
  const { data: items } = useQuery({
    queryKey: ["inbox", targetProject],
    queryFn: () => api.listInbox(targetProject),
  });
  const { data: detail } = useQuery({
    queryKey: ["project", targetProject],
    queryFn: () => api.getProject(targetProject!),
    enabled: targetProject !== null,
  });

  // Default the target to the first project.
  useEffect(() => {
    if (targetProject === null && projects && projects.length > 0) {
      setTargetProject(projects[0].id);
    }
  }, [projects, targetProject]);

  const file = useMutation({
    mutationFn: (
      filings: { path: string; project_id: number; bin_id: number | null }[],
    ) => api.fileInbox(filings),
    onSuccess: (n) => {
      push(`Filed ${n} item${n === 1 ? "" : "s"}`);
      setOverrides({});
      qc.invalidateQueries();
    },
    onError: (e) => push(String(e), "error"),
  });

  const binFor = (item: NonNullable<typeof items>[number]) =>
    item.path in overrides ? overrides[item.path] : item.suggested_bin_id;

  const fileAll = () => {
    if (targetProject === null || !items) return;
    file.mutate(
      items.map((i) => ({
        path: i.path,
        project_id: targetProject,
        bin_id: binFor(i),
      })),
    );
  };

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div className="flex items-center gap-3 px-6 pb-4 pt-5">
        <h1 className="text-[20px] font-semibold">Inbox</h1>
        {items && (
          <span className="font-mono text-[11px] text-muted">
            {items.length} items in _Inbox
          </span>
        )}
        <div className="ml-auto flex items-center gap-2">
          <span className="text-[12px] text-muted">File into</span>
          <select
            value={targetProject ?? ""}
            onChange={(e) => setTargetProject(Number(e.target.value))}
            className="rounded-md border border-line bg-panel px-2 py-1.5 text-[12px]"
          >
            {(projects ?? []).map((p) => (
              <option key={p.id} value={p.id}>
                {p.emoji} {p.name}
              </option>
            ))}
          </select>
          <button
            onClick={fileAll}
            disabled={!items || items.length === 0 || file.isPending}
            className="rounded-lg bg-solder px-3.5 py-1.5 text-[12px] font-semibold text-ink transition-opacity hover:opacity-90 disabled:opacity-40"
          >
            File all
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-6 pb-6">
        {!items || items.length === 0 ? (
          <div className="mx-auto mt-16 max-w-[380px] text-center">
            <div className="mb-2 text-[28px]">📥</div>
            <div className="mb-1 text-[16px] font-semibold">Inbox zero</div>
            <p className="leading-relaxed text-muted">
              Drop files into the <span className="font-mono">_Inbox</span>{" "}
              folder in your root (or onto this window) and file them into
              project bins from here. Rules suggest the right bin
              automatically.
            </p>
          </div>
        ) : (
          <div className="overflow-hidden rounded-panel border border-line">
            {items.map((item) => (
              <div
                key={item.path}
                className="flex items-center gap-3 border-b border-line/50 bg-panel px-4 py-2.5 last:border-b-0"
              >
                <span className="text-[14px]">
                  {fileIcon(item.name.split(".").pop() ?? null)}
                </span>
                <span className="min-w-0 flex-1 truncate text-[12.5px]">
                  {item.name}
                </span>
                <span className="font-mono text-[11px] text-muted">
                  {formatBytes(item.size)}
                </span>
                <span className="w-16 text-right font-mono text-[11px] text-muted">
                  {formatAgo(item.mtime)}
                </span>
                <select
                  value={binFor(item) ?? "root"}
                  onChange={(e) =>
                    setOverrides((o) => ({
                      ...o,
                      [item.path]:
                        e.target.value === "root" ? null : Number(e.target.value),
                    }))
                  }
                  className={`w-36 rounded-md border border-line bg-panel-2 px-2 py-1 text-[11px] ${
                    binFor(item) !== null ? "text-solder" : "text-muted"
                  }`}
                >
                  <option value="root">Project root</option>
                  {(detail?.bins ?? []).map((b) => (
                    <option key={b.id} value={b.id}>
                      {b.name}
                      {item.suggested_bin_id === b.id ? " · suggested" : ""}
                    </option>
                  ))}
                </select>
                <button
                  onClick={() =>
                    targetProject !== null &&
                    file.mutate([
                      {
                        path: item.path,
                        project_id: targetProject,
                        bin_id: binFor(item),
                      },
                    ])
                  }
                  className="rounded-md border border-line px-2.5 py-1 text-[11px] font-medium text-muted transition-colors hover:border-solder hover:text-solder"
                >
                  File it
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
