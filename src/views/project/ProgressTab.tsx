import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as chrono from "chrono-node";
import {
  Line,
  LineChart,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { api, MilestoneRow, ProgressEvaluation, ProjectDetail, TaskRow } from "../../lib/api";
import { HEALTH_COLORS as HEALTH_COLOR, HEALTH_LABELS as HEALTH_LABEL, STATUS_COLORS, tint } from "../../lib/format";
import { EditableRing } from "../../components/EditableRing";
import { useToasts } from "../../lib/store";



export function ProgressTab({ project }: { project: ProjectDetail }) {
  const qc = useQueryClient();
  const { push } = useToasts();
  const { data: stats } = useQuery({
    queryKey: ["stats", project.id],
    queryFn: () => api.getProgressStats(project.id),
  });
  const { data: milestones } = useQuery({
    queryKey: ["milestones", project.id],
    queryFn: () => api.listMilestones(project.id),
  });
  const { data: tasks } = useQuery({
    queryKey: ["tasks", project.id],
    queryFn: () => api.listTasks(project.id, true),
  });

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ["stats", project.id] });
    qc.invalidateQueries({ queryKey: ["milestones", project.id] });
    qc.invalidateQueries({ queryKey: ["tasks", project.id] });
    qc.invalidateQueries({ queryKey: ["project", project.id] });
    qc.invalidateQueries({ queryKey: ["projects"] });
  };

  const report = useMutation({
    // AI drafts the report when a provider is configured; the built-in
    // template takes over otherwise.
    mutationFn: async () => {
      try {
        return await api.aiStatusReport(project.id);
      } catch {
        return await api.draftStatusReport(project.id);
      }
    },
    onSuccess: (text) => {
      navigator.clipboard.writeText(text);
      push("Status report saved to Log and copied");
      qc.invalidateQueries({ queryKey: ["logs", project.id] });
    },
  });

  const ringColor = STATUS_COLORS[project.status] ?? "var(--color-solder)";
  const blocked = (tasks ?? []).filter((t) => t.blocked && !t.done);

  const [evaluation, setEvaluation] = useState<ProgressEvaluation | null>(null);

  const evaluate = useMutation({
    mutationFn: () => api.aiEvaluateProgress(project.id),
    onSuccess: setEvaluation,
    onError: (e) => push(String(e), "error"),
  });

  /** Write a percentage and make it stick: milestone mode would otherwise
   *  recompute the ring the next time a milestone moved. */
  const applyProgress = async (value: number) => {
    await api.setProgress(project.id, value);
    if (project.progress_mode === "milestones") {
      await api.setProgressMode(project.id, "manual");
      push(`Progress set to ${value}% — switched off milestone tracking`);
    } else {
      push(`Progress set to ${value}%`);
    }
    invalidate();
  };

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[860px] px-6 py-5">
        {/* Header stats */}
        <div className="mb-5 flex items-center gap-6 rounded-panel border border-line bg-panel p-5">
          <EditableRing project={project} color={ringColor} />
          <Stat label="velocity" value={`${stats?.velocity_per_week ?? 0 > 0 ? "+" : ""}${stats?.velocity_per_week ?? 0}%/wk`} />
          <Stat
            label="projected"
            value={stats?.projected_finish ?? "—"}
            sub={project.target_date ? `target ${project.target_date}` : "no target"}
          />
          <div className="flex flex-col gap-1">
            <span className="text-[10px] uppercase tracking-wide text-muted">health</span>
            <span
              className="rounded-full px-2.5 py-0.5 text-[12px] font-semibold"
              style={{
                color: HEALTH_COLOR[stats?.health ?? "on_track"],
                background: tint(HEALTH_COLOR[stats?.health ?? "on_track"], 12),
              }}
            >
              {HEALTH_LABEL[stats?.health ?? "on_track"]}
            </span>
          </div>
          <Stat label="last touch" value={stats?.days_since_touch != null ? `${stats.days_since_touch}d ago` : "—"} />
          <Stat label="this week" value={`${stats?.hours_this_week ?? 0}h`} />
          <div className="ml-auto flex flex-col items-end gap-1.5">
            <button
              onClick={() => evaluate.mutate()}
              disabled={evaluate.isPending}
              className="rounded-lg border border-line px-3 py-1.5 text-[12px] text-muted transition-colors hover:border-solder hover:text-solder disabled:opacity-50"
              title="Have your AI read the milestones, tasks and log, then estimate where this project stands"
            >
              {evaluate.isPending ? "Evaluating…" : "✳️ AI evaluate progress"}
            </button>
            <button
              onClick={() => report.mutate()}
              className="rounded-lg border border-line px-3 py-1.5 text-[12px] text-muted transition-colors hover:border-solder hover:text-solder"
            >
              Draft status report
            </button>
          </div>
        </div>

        {evaluation && (
          <Evaluation
            evaluation={evaluation}
            onApply={async () => {
              await applyProgress(evaluation.percent);
              setEvaluation(null);
            }}
            onDismiss={() => setEvaluation(null)}
          />
        )}

        {/* History chart */}
        {(stats?.history.length ?? 0) > 1 && (
          <div className="mb-5 rounded-panel border border-line bg-panel p-4">
            <ResponsiveContainer width="100%" height={140}>
              <LineChart data={stats!.history.map((h) => ({ ts: h.ts.slice(5, 10), value: h.value }))}>
                <XAxis dataKey="ts" tick={{ fontSize: 10, fill: "var(--muted)", fontFamily: "JetBrains Mono Variable" }} axisLine={{ stroke: "var(--line)" }} tickLine={false} />
                <YAxis domain={[0, 100]} width={30} tick={{ fontSize: 10, fill: "var(--muted)", fontFamily: "JetBrains Mono Variable" }} axisLine={false} tickLine={false} />
                <Tooltip
                  contentStyle={{ background: "var(--bg2)", border: "1px solid var(--line)", borderRadius: 8, fontSize: 11, color: "var(--text)" }}
                  labelStyle={{ color: "var(--muted)" }}
                />
                {project.target_date && (
                  <ReferenceLine x={project.target_date.slice(5, 10)} stroke="var(--warn)" strokeDasharray="4 3" />
                )}
                <Line type="monotone" dataKey="value" stroke="var(--accent)" strokeWidth={2} dot={false} />
              </LineChart>
            </ResponsiveContainer>
          </div>
        )}

        {/* Blockers strip */}
        {blocked.length > 0 && (
          <div className="mb-5 rounded-panel border border-st-late/40 bg-st-late/5 p-3">
            <div className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-st-late">
              Blocked ({blocked.length})
            </div>
            {blocked.map((t) => (
              <div key={t.id} className="flex items-center gap-2 py-1 text-[12.5px]">
                <span className="font-mono text-[10px] font-semibold uppercase text-st-late">block</span>
                <span>{t.title}</span>
                {t.blocked_reason && <span className="text-muted">— {t.blocked_reason}</span>}
                <button
                  onClick={async () => {
                    await api.updateTask(t.id, { blocked: false, blocked_reason: null });
                    invalidate();
                  }}
                  className="ml-auto rounded-md border border-line px-2 py-0.5 text-[11px] text-muted hover:border-solder hover:text-solder"
                >
                  Unblock
                </button>
              </div>
            ))}
          </div>
        )}

        {/* Milestone kanban */}
        <MilestoneKanban project={project} milestones={milestones ?? []} onChange={invalidate} />

        {/* Tasks */}
        <TaskSection project={project} tasks={tasks ?? []} milestones={milestones ?? []} onChange={invalidate} />

        {/* Heatmap */}
        {stats && <Heatmap data={stats.heatmap} />}
      </div>
    </div>
  );
}

/** An AI's read on progress. Advisory only — nothing is written until the
 *  user hits Apply. */
function Evaluation({
  evaluation,
  onApply,
  onDismiss,
}: {
  evaluation: ProgressEvaluation;
  onApply: () => void;
  onDismiss: () => void;
}) {
  const delta = evaluation.percent - evaluation.current;
  return (
    <div className="mb-5 rounded-panel border border-solder/40 bg-solder/5 p-4">
      <div className="flex items-center gap-2.5">
        <span className="font-mono text-[20px] font-medium">{evaluation.percent}%</span>
        {delta !== 0 && (
          <span
            className="font-mono text-[11px]"
            style={{ color: delta > 0 ? "var(--color-ok, var(--accent))" : "var(--color-st-late, var(--danger))" }}
          >
            {delta > 0 ? "+" : ""}
            {delta} vs the ring
          </span>
        )}
        <span className="text-[11px] uppercase tracking-wide text-muted">AI estimate</span>
        <button
          onClick={onApply}
          className="ml-auto rounded-lg bg-solder px-3 py-1.5 text-[12px] font-semibold text-ink"
        >
          Use {evaluation.percent}%
        </button>
        <button
          onClick={onDismiss}
          className="rounded-lg border border-line px-3 py-1.5 text-[12px] text-muted hover:border-solder hover:text-solder"
        >
          Dismiss
        </button>
      </div>
      {evaluation.summary && (
        <p className="mt-2 text-[12.5px] leading-relaxed">{evaluation.summary}</p>
      )}
      {evaluation.reasons.length > 0 && (
        <ul className="mt-2 flex flex-col gap-1">
          {evaluation.reasons.map((r, i) => (
            <li key={i} className="text-[12px] leading-relaxed text-muted">
              — {r}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function Stat({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[10px] uppercase tracking-wide text-muted">{label}</span>
      <span className="font-mono text-[15px] font-medium">{value}</span>
      {sub && <span className="font-mono text-[10px] text-muted">{sub}</span>}
    </div>
  );
}

function MilestoneKanban({
  project,
  milestones,
  onChange,
}: {
  project: ProjectDetail;
  milestones: MilestoneRow[];
  onChange: () => void;
}) {
  const [adding, setAdding] = useState<string | null>(null);
  const { push } = useToasts();

  if (milestones.length === 0) {
    return (
      <div className="mb-5 rounded-panel border border-line bg-panel p-5 text-center">
        <p className="mb-3 text-muted">
          No milestones yet. Milestones drive the progress ring — check them
          off and the % takes care of itself.
        </p>
        <div className="flex justify-center gap-2">
          <button
            onClick={async () => {
              await api.applyMilestoneTemplate(project.id, "hardware");
              push("Hardware milestones added");
              onChange();
            }}
            className="rounded-lg bg-solder px-3.5 py-1.5 text-[12px] font-semibold text-ink"
          >
            Hardware template
          </button>
          <button
            onClick={async () => {
              await api.applyMilestoneTemplate(project.id, "software");
              push("Software milestones added");
              onChange();
            }}
            className="rounded-lg border border-line px-3.5 py-1.5 text-[12px] text-muted hover:border-solder hover:text-solder"
          >
            Software template
          </button>
          <button
            onClick={async () => {
              const desc = prompt(
                "Describe the project in a sentence — AI drafts the milestones:",
              );
              if (!desc?.trim()) return;
              try {
                const titles = await api.aiAutoMilestones(project.id, desc.trim());
                if (titles.length === 0) {
                  push("AI returned no milestones", "error");
                  return;
                }
                for (const t of titles) await api.addMilestone(project.id, t);
                await api.setProgressMode(project.id, "milestones");
                push(`${titles.length} milestones drafted`);
                onChange();
              } catch (e) {
                push(String(e), "error");
              }
            }}
            className="rounded-lg border border-line px-3.5 py-1.5 text-[12px] text-muted hover:border-solder hover:text-solder"
          >
            ✳️ AI milestones
          </button>
        </div>
      </div>
    );
  }

  const cols: { state: MilestoneRow["state"]; label: string }[] = [
    { state: "todo", label: "Todo" },
    { state: "doing", label: "Doing" },
    { state: "done", label: "Done" },
  ];

  return (
    <div className="mb-5">
      <div className="mb-2 flex items-center gap-2">
        <h3 className="text-[14px] font-semibold">Milestones</h3>
        <span className="font-mono text-[11px] text-muted">
          {milestones.filter((m) => m.state === "done").length}/{milestones.length}
        </span>
        <label className="ml-auto flex items-center gap-1.5 text-[11px] text-muted">
          <input
            type="checkbox"
            checked={project.progress_mode === "milestones"}
            onChange={async (e) => {
              await api.setProgressMode(project.id, e.target.checked ? "milestones" : "manual");
              onChange();
            }}
            className="accent-(--color-solder)"
          />
          Milestones drive progress
        </label>
      </div>
      <div className="grid grid-cols-3 gap-3">
        {cols.map((col) => (
          <div
            key={col.state}
            onDragOver={(e) => e.preventDefault()}
            onDrop={async (e) => {
              const id = e.dataTransfer.getData("application/x-hangar-milestone");
              if (id) {
                await api.setMilestoneState(Number(id), col.state);
                onChange();
              }
            }}
            className="flex min-h-[120px] flex-col gap-1.5 rounded-panel border border-line bg-panel p-2.5"
          >
            <div className="flex items-center px-1 text-[10px] font-medium uppercase tracking-wide text-muted">
              {col.label}
              <span className="ml-auto font-mono">
                {milestones.filter((m) => m.state === col.state).length}
              </span>
            </div>
            {milestones
              .filter((m) => m.state === col.state)
              .map((m) => (
                <div
                  key={m.id}
                  draggable
                  onDragStart={(e) =>
                    e.dataTransfer.setData("application/x-hangar-milestone", String(m.id))
                  }
                  className={`group cursor-grab rounded-lg border border-line bg-panel-2 px-2.5 py-2 text-[12px] ${
                    m.state === "done" ? "opacity-60" : ""
                  }`}
                >
                  <div className="flex items-center gap-1.5">
                    <span className="flex-1">{m.title}</span>
                    <button
                      onClick={async () => {
                        await api.deleteMilestone(m.id);
                        onChange();
                      }}
                      className="hidden text-muted hover:text-st-late group-hover:block"
                      title="Delete milestone"
                    >
                      ✕
                    </button>
                  </div>
                  <div className="mt-1 flex items-center gap-2 font-mono text-[10px] text-muted">
                    <span>w{m.weight}</span>
                    {m.task_count > 0 && (
                      <span>
                        {m.done_task_count}/{m.task_count} tasks
                      </span>
                    )}
                    {m.state !== "done" && (
                      <button
                        onClick={async () => {
                          await api.setMilestoneState(m.id, m.state === "todo" ? "doing" : "done");
                          onChange();
                        }}
                        className="ml-auto hidden text-solder group-hover:block"
                      >
                        {m.state === "todo" ? "start →" : "done ✓"}
                      </button>
                    )}
                  </div>
                </div>
              ))}
            {col.state === "todo" &&
              (adding !== null ? (
                <input
                  autoFocus
                  value={adding}
                  onChange={(e) => setAdding(e.target.value)}
                  onBlur={() => setAdding(null)}
                  onKeyDown={async (e) => {
                    if (e.key === "Enter" && adding.trim()) {
                      await api.addMilestone(project.id, adding.trim());
                      setAdding(null);
                      onChange();
                    }
                    if (e.key === "Escape") setAdding(null);
                  }}
                  placeholder="Milestone title"
                  className="rounded-lg border border-solder bg-panel-2 px-2.5 py-2 text-[12px] focus:outline-none"
                />
              ) : (
                <button
                  onClick={() => setAdding("")}
                  className="rounded-lg px-2.5 py-1.5 text-left text-[11px] text-muted hover:bg-panel-2 hover:text-text"
                >
                  + Add
                </button>
              ))}
          </div>
        ))}
      </div>
    </div>
  );
}

const PRIORITY_COLOR: Record<string, string> = {
  high: "var(--danger)",
  med: "var(--muted)",
  low: "var(--line-strong)",
};

function TaskSection({
  project,
  tasks,
  milestones,
  onChange,
}: {
  project: ProjectDetail;
  tasks: TaskRow[];
  milestones: MilestoneRow[];
  onChange: () => void;
}) {
  const [draft, setDraft] = useState("");
  const [showDone, setShowDone] = useState(false);

  const addTask = async () => {
    const text = draft.trim();
    if (!text) return;
    // Natural dates: "panelize gerbers fri" → due Friday.
    const parsed = chrono.parse(text, new Date(), { forwardDate: true });
    let title = text;
    let due: string | null = null;
    let priority = "med";
    if (parsed.length > 0) {
      const p = parsed[parsed.length - 1];
      // Only strip the date text when it sits at the end.
      if (p.index + p.text.length >= text.length - 1) {
        title = text.slice(0, p.index).trim();
        const d = p.start.date();
        due = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
      }
    }
    if (title.endsWith("!")) {
      priority = "high";
      title = title.replace(/!+$/, "").trim();
    }
    if (!title) title = text;
    await api.addTask({ project_id: project.id, title, due, priority });
    setDraft("");
    onChange();
  };

  const visible = tasks.filter((t) => showDone || !t.done);
  const today = new Date().toISOString().slice(0, 10);

  return (
    <div className="mb-5">
      <div className="mb-2 flex items-center gap-2">
        <h3 className="text-[14px] font-semibold">Tasks</h3>
        <span className="font-mono text-[11px] text-muted">
          {tasks.filter((t) => !t.done).length} open
        </span>
        <button
          onClick={() => setShowDone(!showDone)}
          className="ml-auto text-[11px] text-muted hover:text-text"
        >
          {showDone ? "Hide done" : "Show done"}
        </button>
      </div>
      <input
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && addTask()}
        placeholder='Quick add — "panelize gerbers fri", "order stencils tomorrow", "fix DFU mode!"'
        className="mb-2 w-full rounded-lg border border-line bg-panel px-3 py-2.5 text-[12.5px] placeholder:text-muted focus:border-solder focus:outline-none"
      />
      <div className="overflow-hidden rounded-panel border border-line">
        {visible.length === 0 ? (
          <p className="bg-panel px-4 py-3 text-[12px] text-muted">
            No open tasks. Type one above — a trailing date like “fri” sets the
            due date, a trailing “!” sets high priority.
          </p>
        ) : (
          visible.map((t) => (
            <div
              key={t.id}
              className={`group flex items-center gap-2.5 border-b border-line/50 bg-panel px-3 py-2 last:border-b-0 ${
                t.done ? "opacity-50" : ""
              }`}
            >
              <button
                onClick={async () => {
                  await api.toggleTask(t.id);
                  onChange();
                }}
                className={`flex h-4 w-4 items-center justify-center rounded border text-[10px] ${
                  t.done ? "border-solder bg-solder text-ink" : "border-line hover:border-solder"
                }`}
              >
                {t.done ? "✓" : ""}
              </button>
              <span
                className="h-1.5 w-1.5 shrink-0 rounded-full"
                style={{ background: PRIORITY_COLOR[t.priority] }}
                title={`${t.priority} priority`}
              />
              <span className={`flex-1 truncate text-[12.5px] ${t.done ? "line-through" : ""}`}>
                {t.blocked && <span className="mr-1.5 font-mono text-[10px] font-semibold uppercase text-st-late">block</span>}
                {t.title}
              </span>
              {t.recurrence && (
                <span className="font-mono text-[10px] text-muted">↻ {t.recurrence}</span>
              )}
              {t.due && (
                <span
                  className={`font-mono text-[11px] ${
                    !t.done && t.due < today ? "text-st-late" : "text-muted"
                  }`}
                >
                  {t.due.slice(5)}
                </span>
              )}
              <select
                value={t.milestone_id ?? ""}
                onChange={async (e) => {
                  await api.updateTask(t.id, {
                    milestone_id: e.target.value === "" ? null : Number(e.target.value),
                  });
                  onChange();
                }}
                className="hidden w-24 rounded-md border border-line bg-panel-2 px-1 py-0.5 text-[10px] text-muted group-hover:block"
                title="Attach to milestone"
              >
                <option value="">no milestone</option>
                {milestones.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.title}
                  </option>
                ))}
              </select>
              {!t.done && (
                <button
                  onClick={async () => {
                    if (t.blocked) {
                      await api.updateTask(t.id, { blocked: false, blocked_reason: null });
                    } else {
                      const reason = prompt("Blocked on…?") ?? "";
                      await api.updateTask(t.id, { blocked: true, blocked_reason: reason || null });
                    }
                    onChange();
                  }}
                  className="hidden text-[11px] text-muted hover:text-st-risk group-hover:block"
                >
                  {t.blocked ? "unblock" : "block"}
                </button>
              )}
              <button
                onClick={async () => {
                  await api.deleteTask(t.id);
                  onChange();
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

function Heatmap({ data }: { data: number[] }) {
  // 26 weeks × 7 days, oldest first.
  const max = Math.max(1, ...data);
  const weeks: number[][] = [];
  for (let w = 0; w < 26; w++) {
    weeks.push(data.slice(w * 7, w * 7 + 7));
  }
  return (
    <div className="rounded-panel border border-line bg-panel p-4">
      <div className="mb-2 text-[11px] font-medium uppercase tracking-wide text-muted">
        Activity · 26 weeks
      </div>
      <div className="flex gap-[3px]">
        {weeks.map((week, wi) => (
          <div key={wi} className="flex flex-col gap-[3px]">
            {week.map((count, di) => (
              <div
                key={di}
                className="h-[10px] w-[10px] rounded-[2px]"
                style={{
                  background:
                    count > 0
                      ? tint("var(--accent)", Math.round(25 + 75 * (count / max)))
                      : "var(--bg2)",
                }}
                title={`${count} events`}
              />
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
