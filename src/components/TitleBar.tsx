import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../lib/api";
import { useUi } from "../lib/store";

export function TitleBar({ root }: { root: string | null }) {
  const qc = useQueryClient();
  const { openProject } = useUi();
  const rescan = useMutation({
    mutationFn: api.rescan,
    onSuccess: () => qc.invalidateQueries({ queryKey: ["projects"] }),
  });
  const { data: timer } = useQuery({
    queryKey: ["timer"],
    queryFn: api.activeTimer,
    refetchInterval: 30_000,
  });

  return (
    <header
      data-tauri-drag-region
      className="flex h-12 shrink-0 items-center border-b border-line bg-ink pl-[84px] pr-3"
    >
      <span data-tauri-drag-region className="text-[13px] font-semibold">
        Hangar
      </span>
      {root && (
        <span
          data-tauri-drag-region
          className="ml-3 truncate font-mono text-[11px] text-muted"
        >
          {root.replace(/^\/Users\/[^/]+/, "~")}
        </span>
      )}
      <div className="ml-auto flex items-center gap-1">
        {timer && (
          <button
            onClick={() => openProject(timer.project_id)}
            className="mr-1 flex items-center gap-1.5 rounded-md border border-solder/40 bg-solder/10 px-2.5 py-1 font-mono text-[11px] text-solder"
            title="Timer running — click to open project"
          >
            <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-solder" />
            {timer.project_name}
          </button>
        )}
        {root && (
          <button
            onClick={() => rescan.mutate()}
            disabled={rescan.isPending}
            className="rounded-md px-2.5 py-1 text-[12px] text-muted transition-colors hover:bg-panel hover:text-text disabled:opacity-50"
            title="Rebuild index from disk"
          >
            {rescan.isPending ? "Scanning…" : "Rescan"}
          </button>
        )}
      </div>
    </header>
  );
}
