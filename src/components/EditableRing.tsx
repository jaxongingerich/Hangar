import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api, ProjectDetail } from "../lib/api";
import { useToasts } from "../lib/store";
import { ProgressRing } from "./ProgressRing";

/**
 * The progress ring, with the number itself as the edit affordance: click it,
 * type a percentage, press Enter.
 *
 * Shared by the project header (small) and the Progress workspace (large) so
 * the number behaves the same everywhere it appears.
 */
export function EditableRing({
  project,
  size = 72,
  stroke = 5,
  color,
}: {
  project: Pick<ProjectDetail, "id" | "progress" | "progress_mode">;
  size?: number;
  stroke?: number;
  color: string;
}) {
  const qc = useQueryClient();
  const { push } = useToasts();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");

  const commit = async () => {
    const n = Number(draft.trim().replace(/%$/, ""));
    setEditing(false);
    if (!Number.isFinite(n)) return;
    const value = Math.round(Math.min(100, Math.max(0, n)));
    if (value === project.progress) return;

    await api.setProgress(project.id, value);
    // Milestone mode would recompute the ring the next time a milestone moved,
    // silently discarding what was just typed — so typing a number opts out.
    if (project.progress_mode === "milestones") {
      await api.setProgressMode(project.id, "manual");
      push(`Progress set to ${value}% — switched off milestone tracking`);
    } else {
      push(`Progress set to ${value}%`);
    }
    for (const key of ["project", "stats", "milestones"]) {
      qc.invalidateQueries({ queryKey: [key, project.id] });
    }
    qc.invalidateQueries({ queryKey: ["projects"] });
  };

  if (editing) {
    return (
      <div
        className="flex shrink-0 items-center justify-center rounded-full border-2 border-solder"
        style={{ width: size, height: size }}
      >
        <input
          autoFocus
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
            if (e.key === "Escape") setEditing(false);
          }}
          inputMode="numeric"
          aria-label="Progress percentage"
          className="w-full min-w-0 bg-transparent text-center font-mono focus:outline-none"
          style={{ fontSize: Math.max(11, size / 4.2) }}
        />
      </div>
    );
  }

  return (
    <button
      onClick={() => {
        setDraft(String(project.progress));
        setEditing(true);
      }}
      title="Click to type a percentage"
      className="shrink-0 rounded-full transition-opacity hover:opacity-75"
    >
      <ProgressRing value={project.progress} color={color} size={size} stroke={stroke} />
    </button>
  );
}
