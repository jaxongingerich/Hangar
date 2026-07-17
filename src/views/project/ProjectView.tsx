import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../lib/api";
import { useToasts, useUi } from "../../lib/store";
import { STATUS_COLORS, STATUS_LABELS } from "../../lib/format";
import { ProgressRing } from "../../components/ProgressRing";
import { FilesTab } from "./FilesTab";
import { LogTab } from "./LogTab";

const TABS = ["Files", "Progress", "Log", "Orders", "Links"] as const;
type Tab = (typeof TABS)[number];

export function ProjectView({ projectId }: { projectId: number }) {
  const [tab, setTab] = useState<Tab>("Files");
  const qc = useQueryClient();
  const { setView } = useUi();
  const { push } = useToasts();

  const { data: project } = useQuery({
    queryKey: ["project", projectId],
    queryFn: () => api.getProject(projectId),
  });

  const update = useMutation({
    mutationFn: (patch: Parameters<typeof api.updateProject>[1]) =>
      api.updateProject(projectId, patch),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["project", projectId] });
      qc.invalidateQueries({ queryKey: ["projects"] });
    },
    onError: (e) => push(String(e), "error"),
  });

  const setProgress = useMutation({
    mutationFn: (value: number) => api.setProgress(projectId, value),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["project", projectId] });
      qc.invalidateQueries({ queryKey: ["projects"] });
    },
  });

  if (!project) return null;
  const ringColor = STATUS_COLORS[project.status] ?? "var(--color-solder)";

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div className="flex items-center gap-3 border-b border-line px-6 py-4">
        <button
          onClick={() => setView("dashboard")}
          className="rounded-md px-1.5 py-1 text-muted hover:bg-panel hover:text-text"
          title="Back to dashboard"
        >
          ←
        </button>
        <span
          className="flex h-9 w-9 items-center justify-center rounded-lg text-[18px]"
          style={{ background: `${project.color}1F` }}
        >
          {project.emoji}
        </span>
        <InlineName
          value={project.name}
          onRename={(name) => update.mutate({ name })}
        />
        <select
          value={project.status}
          onChange={(e) => update.mutate({ status: e.target.value })}
          className="rounded-md border border-line bg-panel px-2 py-1 text-[12px]"
          style={{ color: STATUS_COLORS[project.status] }}
        >
          {Object.entries(STATUS_LABELS)
            .filter(([s]) => s !== "archived")
            .map(([s, label]) => (
              <option key={s} value={s}>
                {label}
              </option>
            ))}
        </select>
        <button
          onClick={() => update.mutate({ pinned: !project.pinned })}
          className={`rounded-md px-2 py-1 text-[12px] ${
            project.pinned ? "text-solder" : "text-muted hover:text-text"
          }`}
          title={project.pinned ? "Unpin" : "Pin to top"}
        >
          {project.pinned ? "Pinned" : "Pin"}
        </button>

        <div className="ml-auto flex items-center gap-3">
          <input
            type="range"
            min={0}
            max={100}
            value={project.progress}
            onChange={(e) => setProgress.mutate(Number(e.target.value))}
            className="w-28 accent-(--color-solder)"
            title="Progress"
          />
          <ProgressRing value={project.progress} color={ringColor} size={36} stroke={3} />
        </div>
      </div>

      <div className="flex gap-1 border-b border-line px-6 pt-2">
        {TABS.map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`rounded-t-md px-3 py-2 text-[13px] font-medium transition-colors ${
              tab === t
                ? "border-b-2 border-solder text-text"
                : "text-muted hover:text-text"
            }`}
          >
            {t}
          </button>
        ))}
      </div>

      {tab === "Files" && <FilesTab project={project} />}
      {tab === "Log" && <LogTab projectId={projectId} />}
      {(tab === "Progress" || tab === "Orders" || tab === "Links") && (
        <div className="flex flex-1 items-center justify-center text-muted">
          <p>
            {tab} arrives in the next build phase.
          </p>
        </div>
      )}
    </div>
  );
}

function InlineName({
  value,
  onRename,
}: {
  value: string;
  onRename: (name: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);

  if (!editing) {
    return (
      <button
        onDoubleClick={() => {
          setDraft(value);
          setEditing(true);
        }}
        className="truncate text-[16px] font-semibold"
        title="Double-click to rename"
      >
        {value}
      </button>
    );
  }
  return (
    <input
      autoFocus
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => {
        setEditing(false);
        if (draft.trim() && draft.trim() !== value) onRename(draft.trim());
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        if (e.key === "Escape") setEditing(false);
      }}
      className="rounded-md border border-solder bg-panel-2 px-2 py-1 text-[15px] font-semibold focus:outline-none"
    />
  );
}
