import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, ProjectDetail, SnapshotRow } from "../../lib/api";
import { useToasts } from "../../lib/store";

export function SnapshotsPanel({
  project,
  snapshots,
}: {
  project: ProjectDetail;
  snapshots: SnapshotRow[];
}) {
  const [selected, setSelected] = useState<number[]>([]);
  const { push } = useToasts();

  const toggle = (id: number) =>
    setSelected((prev) =>
      prev.includes(id)
        ? prev.filter((x) => x !== id)
        : [...prev.slice(-1), id],
    );

  const [a, b] = selected;
  const { data: diff } = useQuery({
    queryKey: ["snapdiff", a, b],
    queryFn: () => api.diffSnapshots(a, b),
    enabled: selected.length === 2,
  });

  if (snapshots.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <p className="max-w-[320px] text-center text-muted">
          No snapshots yet. Open a bin (Gerbers, say) and hit Snapshot to
          freeze a labelled revision — then diff any two, and export exactly
          what you fabbed.
        </p>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto p-4">
      <p className="mb-3 font-mono text-[11px] text-muted">
        select two snapshots to diff
      </p>
      <div className="mb-4 overflow-hidden rounded-panel border border-line">
        {snapshots.map((s) => (
          <div
            key={s.id}
            className={`flex items-center gap-3 border-b border-line/50 px-4 py-2.5 last:border-b-0 ${
              selected.includes(s.id) ? "bg-panel-2" : "bg-panel"
            }`}
          >
            <input
              type="checkbox"
              checked={selected.includes(s.id)}
              onChange={() => toggle(s.id)}
              className="accent-(--color-solder)"
            />
            <span className="font-mono text-[12px] font-medium text-solder">
              {s.label}
            </span>
            <span className="text-[11px] text-muted">{s.bin_name ?? "bin gone"}</span>
            <span className="font-mono text-[11px] text-muted">{s.file_count} files</span>
            <span className="ml-auto font-mono text-[10px] text-muted">
              {s.created_at.slice(0, 16)}
            </span>
            <button
              onClick={() => revealItemInDir(s.zip_path)}
              className="text-[11px] text-muted hover:text-solder"
            >
              reveal
            </button>
            <button
              onClick={async () => {
                try {
                  const check = await api.exportJlcpcb(project.id, {
                    snapshotId: s.id,
                    dryRun: true,
                  });
                  const proceed =
                    check.missing.length === 0 ||
                    confirm(
                      `Missing layers:\n• ${check.missing.join("\n• ")}\n\nExport anyway?`,
                    );
                  if (!proceed) return;
                  const result = await api.exportJlcpcb(project.id, {
                    snapshotId: s.id,
                    dryRun: false,
                  });
                  push(`JLC package from "${s.label}" → ${result.zip_path?.split("/").pop()}`);
                } catch (e) {
                  push(String(e), "error");
                }
              }}
              className="rounded-md border border-line px-2 py-0.5 text-[11px] text-muted hover:border-solder hover:text-solder"
            >
              Export JLC
            </button>
          </div>
        ))}
      </div>

      {selected.length === 2 && diff && (
        <div className="rounded-panel border border-line bg-panel p-4">
          <div className="mb-2 font-mono text-[11px] text-muted">
            {snapshots.find((s) => s.id === a)?.label} → {snapshots.find((s) => s.id === b)?.label}
          </div>
          {diff.added.length + diff.removed.length + diff.changed.length === 0 ? (
            <p className="text-[12px] text-muted">Identical.</p>
          ) : (
            <div className="flex flex-col gap-0.5 font-mono text-[12px]">
              {diff.added.map((p) => (
                <span key={`a${p}`} className="text-solder">+ {p}</span>
              ))}
              {diff.removed.map((p) => (
                <span key={`r${p}`} className="text-st-late">− {p}</span>
              ))}
              {diff.changed.map((p) => (
                <span key={`c${p}`} className="text-st-risk">~ {p}</span>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
