import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, LogRow } from "../../lib/api";

const KIND_LABEL: Record<LogRow["kind"], string> = {
  note: "note",
  auto: "auto",
  status_report: "status",
  digest: "digest",
};

export function LogTab({ projectId }: { projectId: number }) {
  const [draft, setDraft] = useState("");
  const [filter, setFilter] = useState<"all" | "note" | "auto">("all");
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const qc = useQueryClient();

  const { data: logs } = useQuery({
    queryKey: ["logs", projectId],
    queryFn: () => api.listLogs(projectId),
  });

  const add = useMutation({
    mutationFn: (body: string) => api.addLog(projectId, body),
    onSuccess: () => {
      setDraft("");
      qc.invalidateQueries({ queryKey: ["logs", projectId] });
    },
  });

  // ⌘L focuses the composer.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.metaKey && e.key === "l") {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const rows = (logs ?? []).filter((l) => filter === "all" || l.kind === filter);

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div className="border-b border-line p-4">
        <textarea
          ref={inputRef}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && e.metaKey && draft.trim()) {
              add.mutate(draft.trim());
            }
          }}
          placeholder="Log a note… (⌘L to focus, ⌘↩ to save)"
          rows={2}
          className="w-full resize-none rounded-lg border border-line bg-panel-2 px-3 py-2.5 text-[13px] placeholder:text-muted focus:border-solder focus:outline-none"
        />
        <div className="mt-2 flex items-center gap-1">
          {(["all", "note", "auto"] as const).map((f) => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={`rounded-md px-2 py-1 text-[11px] font-medium ${
                filter === f ? "bg-panel-2 text-text" : "text-muted hover:text-text"
              }`}
            >
              {f === "all" ? "All" : f === "note" ? "Notes" : "Auto"}
            </button>
          ))}
          <button
            onClick={() => draft.trim() && add.mutate(draft.trim())}
            disabled={!draft.trim() || add.isPending}
            className="ml-auto rounded-md bg-solder px-3 py-1.5 text-[12px] font-semibold text-ink disabled:opacity-40"
          >
            Save note
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-3">
        {rows.length === 0 ? (
          <p className="mt-8 text-center text-muted">
            No log entries yet. Notes you write and everything Hangar does land
            here — and in <span className="font-mono">.hangar/log.md</span>.
          </p>
        ) : (
          <div className="flex flex-col">
            {rows.map((l) => (
              <div
                key={l.id}
                className="flex gap-3 border-b border-line/50 py-2.5"
              >
                <span className="w-28 shrink-0 font-mono text-[11px] text-muted">
                  {l.ts.slice(0, 16)}
                </span>
                <span
                  className={`w-12 shrink-0 font-mono text-[10px] uppercase ${
                    l.kind === "note" ? "text-solder" : "text-muted"
                  }`}
                >
                  {KIND_LABEL[l.kind]}
                </span>
                <span className="flex-1 whitespace-pre-wrap text-[12.5px] leading-relaxed select-text">
                  {l.body_md}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
