import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../lib/api";
import { useUi } from "../lib/store";

export function TitleBar({ root }: { root: string | null }) {
  const qc = useQueryClient();
  const { openProject, setPaletteOpen } = useUi();
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
      <svg
        data-tauri-drag-region
        width="16"
        height="16"
        viewBox="0 0 24 24"
        fill="none"
        stroke="var(--accent)"
        strokeWidth="2.4"
        strokeLinecap="round"
        className="mr-2 shrink-0"
        aria-hidden
      >
        <path d="M4 19v-6a8 8 0 0 1 16 0v6" />
        <path d="M3 19h18" />
      </svg>
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
      <div className="ml-auto flex items-center gap-1.5">
        {root && (
          <button
            onClick={() => setPaletteOpen(true)}
            className="flex w-48 items-center gap-2 rounded-lg border border-line bg-panel px-3 py-1.5 text-left text-[12px] text-muted transition-colors hover:border-line-strong"
            title="Search everything"
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
              <circle cx="11" cy="11" r="7" />
              <path d="m21 21-4.35-4.35" />
            </svg>
            <span className="flex-1">Search</span>
            <span className="font-mono text-[10px]">⌘K</span>
          </button>
        )}
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
