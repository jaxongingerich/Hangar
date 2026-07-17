import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, TaskRow } from "../lib/api";
import { useUi } from "../lib/store";

export function Today() {
  const qc = useQueryClient();
  const { openProject } = useUi();
  const { data } = useQuery({ queryKey: ["today"], queryFn: api.todayData });

  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["today"] });
    qc.invalidateQueries({ queryKey: ["projects"] });
  };

  const total =
    (data?.overdue.length ?? 0) +
    (data?.due_today.length ?? 0) +
    (data?.high_priority.length ?? 0);

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[720px] px-6 pb-10 pt-5">
        <div className="mb-1 flex items-baseline gap-3">
          <h1 className="text-[20px] font-semibold">Today</h1>
          <span className="font-mono text-[11px] text-muted">
            {new Date().toLocaleDateString(undefined, {
              weekday: "long",
              month: "long",
              day: "numeric",
            })}
          </span>
        </div>
        <p className="mb-6 text-[12.5px] text-muted">
          {total === 0
            ? "Clear runway — nothing due, nothing on fire."
            : `${total} task${total === 1 ? "" : "s"} need attention.`}
        </p>

        {data && data.overdue.length > 0 && (
          <TaskGroup title="Overdue" tone="late" tasks={data.overdue} onChange={refresh} onOpen={openProject} />
        )}
        {data && data.due_today.length > 0 && (
          <TaskGroup title="Due today" tone="risk" tasks={data.due_today} onChange={refresh} onOpen={openProject} />
        )}
        {data && data.high_priority.length > 0 && (
          <TaskGroup title="High priority" tone="normal" tasks={data.high_priority} onChange={refresh} onOpen={openProject} />
        )}

        {data && data.arriving.length > 0 && (
          <section className="mb-6">
            <h2 className="mb-2 text-[13px] font-semibold text-muted">Arriving soon</h2>
            <div className="overflow-hidden rounded-panel border border-line">
              {data.arriving.map((o) => (
                <div
                  key={o.id}
                  className="flex items-center gap-3 border-b border-line/50 bg-panel px-4 py-2.5 last:border-b-0"
                >
                  <span>📦</span>
                  <span className="text-[12.5px]">
                    {o.vendor}
                    {o.items ? ` — ${o.items}` : ""}
                  </span>
                  <button
                    onClick={() => openProject(o.project_id)}
                    className="text-[11px] text-muted hover:text-solder"
                  >
                    {o.project_name}
                  </button>
                  <span className="ml-auto font-mono text-[11px] text-st-risk">
                    ETA {o.eta?.slice(5)}
                  </span>
                </div>
              ))}
            </div>
          </section>
        )}

        {data && data.suggestions.length > 0 && (
          <section>
            <h2 className="mb-2 text-[13px] font-semibold text-muted">Next best action</h2>
            <div className="overflow-hidden rounded-panel border border-line">
              {data.suggestions.map(([pid, emoji, name, suggestion]) => (
                <button
                  key={pid}
                  onClick={() => openProject(pid)}
                  className="flex w-full items-center gap-3 border-b border-line/50 bg-panel px-4 py-2.5 text-left transition-colors last:border-b-0 hover:bg-panel-2"
                >
                  <span>{emoji}</span>
                  <span className="w-36 shrink-0 truncate text-[12px] text-muted">{name}</span>
                  <span className="truncate text-[12.5px]">{suggestion}</span>
                </button>
              ))}
            </div>
          </section>
        )}
      </div>
    </div>
  );
}

function TaskGroup({
  title,
  tone,
  tasks,
  onChange,
  onOpen,
}: {
  title: string;
  tone: "late" | "risk" | "normal";
  tasks: TaskRow[];
  onChange: () => void;
  onOpen: (id: number) => void;
}) {
  const color =
    tone === "late" ? "text-st-late" : tone === "risk" ? "text-st-risk" : "text-muted";
  return (
    <section className="mb-6">
      <h2 className={`mb-2 text-[13px] font-semibold ${color}`}>
        {title} · {tasks.length}
      </h2>
      <div className="overflow-hidden rounded-panel border border-line">
        {tasks.map((t) => (
          <div
            key={t.id}
            className="flex items-center gap-2.5 border-b border-line/50 bg-panel px-4 py-2.5 last:border-b-0"
          >
            <button
              onClick={async () => {
                await api.toggleTask(t.id);
                onChange();
              }}
              className="flex h-4 w-4 items-center justify-center rounded border border-line text-[10px] hover:border-solder"
            />
            <span className="flex-1 truncate text-[12.5px]">
              {t.blocked && <span className="mr-1">⛔</span>}
              {t.title}
            </span>
            <button
              onClick={() => onOpen(t.project_id)}
              className="text-[11px] text-muted hover:text-solder"
            >
              {t.project_emoji} {t.project_name}
            </button>
            {t.due && <span className="font-mono text-[11px] text-muted">{t.due.slice(5)}</span>}
          </div>
        ))}
      </div>
    </section>
  );
}
