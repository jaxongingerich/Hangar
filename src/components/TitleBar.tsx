import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../lib/api";

export function TitleBar({ root }: { root: string | null }) {
  const qc = useQueryClient();
  const rescan = useMutation({
    mutationFn: api.rescan,
    onSuccess: () => qc.invalidateQueries({ queryKey: ["projects"] }),
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
