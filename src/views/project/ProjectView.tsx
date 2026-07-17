import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../lib/api";
import { useToasts, useUi } from "../../lib/store";
import { STATUS_COLORS, STATUS_LABELS } from "../../lib/format";
import { ProgressRing } from "../../components/ProgressRing";
import { FilesTab } from "./FilesTab";
import { LogTab } from "./LogTab";
import { ProgressTab } from "./ProgressTab";
import { OrdersTab } from "./OrdersTab";
import { LinksTab } from "./LinksTab";
import { ChatDrawer } from "./ChatDrawer";

const TABS = ["Files", "Progress", "Log", "Orders", "Links"] as const;
type Tab = (typeof TABS)[number];

export function ProjectView({ projectId }: { projectId: number }) {
  const [tab, setTab] = useState<Tab>("Files");
  const [chatOpen, setChatOpen] = useState(false);
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
          <button
            onClick={() => setChatOpen(!chatOpen)}
            className={`rounded-md border px-2.5 py-1 text-[12px] transition-colors ${
              chatOpen
                ? "border-solder bg-solder/10 text-solder"
                : "border-line text-muted hover:border-solder hover:text-solder"
            }`}
            title="AI project chat"
          >
            Chat
          </button>
          <HeaderMenu projectId={projectId} projectName={project.name} />
          <TimerButton projectId={projectId} />
          <input
            type="date"
            value={project.target_date ?? ""}
            onChange={(e) => update.mutate({ target_date: e.target.value || null })}
            className="rounded-md border border-line bg-panel px-2 py-1 font-mono text-[11px] text-muted"
            title="Target date"
          />
          {project.progress_mode === "manual" && (
            <input
              type="range"
              min={0}
              max={100}
              value={project.progress}
              onChange={(e) => setProgress.mutate(Number(e.target.value))}
              className="w-24 accent-(--color-solder)"
              title="Progress"
            />
          )}
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

      <div className="flex flex-1 overflow-hidden">
        <div className="flex flex-1 flex-col overflow-hidden">
          {tab === "Files" && <FilesTab project={project} />}
          {tab === "Progress" && <ProgressTab project={project} />}
          {tab === "Log" && <LogTab projectId={projectId} />}
          {tab === "Orders" && <OrdersTab projectId={projectId} />}
          {tab === "Links" && <LinksTab projectId={projectId} />}
        </div>
        {chatOpen && (
          <ChatDrawer projectId={projectId} onClose={() => setChatOpen(false)} />
        )}
      </div>
    </div>
  );
}

function HeaderMenu({
  projectId,
  projectName,
}: {
  projectId: number;
  projectName: string;
}) {
  const [open, setOpen] = useState(false);
  const qc = useQueryClient();
  const { setView } = useUi();
  const { push } = useToasts();

  return (
    <div className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="rounded-md border border-line px-2 py-1 text-[12px] text-muted hover:border-solder hover:text-solder"
        title="Project actions"
      >
        ⋯
      </button>
      {open && (
        <div className="absolute right-0 top-8 z-40 w-48 overflow-hidden rounded-lg border border-line bg-panel-2 shadow-xl">
          <MenuItem
            label="Export one-pager"
            onClick={async () => {
              setOpen(false);
              const path = await api.exportOnePager(projectId);
              const { openPath } = await import("@tauri-apps/plugin-opener");
              await openPath(path);
              push("One-pager opened — ⌘P to save as PDF");
            }}
          />
          <MenuItem
            label="Archive project…"
            danger
            onClick={async () => {
              setOpen(false);
              if (
                !confirm(
                  `Archive "${projectName}"?\n\nThe folder is zipped into _Archive/ and the original moves to Trash. Restore any time from Space → Archives.`,
                )
              )
                return;
              try {
                await api.archiveProject(projectId);
                push(`${projectName} archived`);
                qc.invalidateQueries();
                setView("dashboard");
              } catch (e) {
                push(String(e), "error");
              }
            }}
          />
        </div>
      )}
    </div>
  );
}

function MenuItem({
  label,
  onClick,
  danger,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      className={`block w-full px-3 py-2 text-left text-[12px] transition-colors hover:bg-panel ${
        danger ? "text-st-late" : "text-text"
      }`}
    >
      {label}
    </button>
  );
}

function TimerButton({ projectId }: { projectId: number }) {
  const qc = useQueryClient();
  const { data: timer } = useQuery({
    queryKey: ["timer"],
    queryFn: api.activeTimer,
    refetchInterval: 30_000,
  });
  const runningHere = timer?.project_id === projectId;
  return (
    <button
      onClick={async () => {
        if (runningHere) await api.stopTimer();
        else await api.startTimer(projectId);
        qc.invalidateQueries({ queryKey: ["timer"] });
      }}
      className={`rounded-md border px-2.5 py-1 font-mono text-[11px] transition-colors ${
        runningHere
          ? "border-solder bg-solder/10 text-solder"
          : "border-line text-muted hover:border-solder hover:text-solder"
      }`}
      title={runningHere ? "Stop timer" : "Start timer"}
    >
      {runningHere ? "■ stop" : "▶ start"}
    </button>
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
