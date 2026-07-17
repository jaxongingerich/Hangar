import { useQuery } from "@tanstack/react-query";
import { Line, LineChart, ResponsiveContainer, YAxis } from "recharts";
import { api } from "../lib/api";
import { useUi } from "../lib/store";
import { ProgressRing } from "../components/ProgressRing";

const HEALTH_COLOR: Record<string, string> = {
  on_track: "#22D3A6",
  at_risk: "#F5A524",
  late: "#F5556D",
};
const HEALTH_LABEL: Record<string, string> = {
  on_track: "On track",
  at_risk: "At risk",
  late: "Late",
};

export function GlobalProgress() {
  const { openProject } = useUi();
  const { data: rows } = useQuery({ queryKey: ["portfolio"], queryFn: api.portfolio });

  const avg =
    rows && rows.length > 0
      ? Math.round(rows.reduce((s, r) => s + r.progress, 0) / rows.length)
      : 0;
  const blocked = rows?.reduce((s, r) => s + r.blocked_count, 0) ?? 0;
  const stale = rows?.filter((r) => (r.days_since_touch ?? 0) > 14).length ?? 0;

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[860px] px-6 pb-10 pt-5">
        <div className="mb-5 flex items-center gap-6">
          <h1 className="text-[20px] font-semibold">Progress</h1>
          <div className="ml-auto flex items-center gap-5 rounded-panel border border-line bg-panel px-5 py-2.5">
            <div className="flex items-center gap-2.5">
              <ProgressRing value={avg} color="#22D3A6" size={34} stroke={3} />
              <span className="text-[11px] text-muted">portfolio avg</span>
            </div>
            <span className="font-mono text-[11px] text-muted">
              {rows?.length ?? 0} active
            </span>
            {blocked > 0 && (
              <span className="font-mono text-[11px] text-st-late">{blocked} blocked</span>
            )}
            {stale > 0 && (
              <span className="font-mono text-[11px] text-st-risk">{stale} stale</span>
            )}
          </div>
        </div>

        <div className="flex flex-col gap-2">
          {(rows ?? []).map((r) => (
            <button
              key={r.id}
              onClick={() => openProject(r.id)}
              className="flex items-center gap-4 rounded-panel border border-line bg-panel px-4 py-3 text-left transition-colors hover:border-[#2E3A4E]"
            >
              <span className="text-[16px]">{r.emoji}</span>
              <span className="w-44 truncate text-[13px] font-medium">{r.name}</span>
              <ProgressRing value={r.progress} color={HEALTH_COLOR[r.health]} size={34} stroke={3} />
              <div className="h-9 w-28">
                {r.history.length > 1 && (
                  <ResponsiveContainer width="100%" height="100%">
                    <LineChart data={r.history.map((h) => ({ v: h.value }))}>
                      <YAxis domain={[0, 100]} hide />
                      <Line type="monotone" dataKey="v" stroke={r.color} strokeWidth={1.5} dot={false} />
                    </LineChart>
                  </ResponsiveContainer>
                )}
              </div>
              <span
                className="rounded-full px-2 py-0.5 text-[10px] font-semibold"
                style={{
                  color: HEALTH_COLOR[r.health],
                  background: `${HEALTH_COLOR[r.health]}1A`,
                }}
              >
                {HEALTH_LABEL[r.health]}
              </span>
              <span className="font-mono text-[11px] text-muted">
                {r.velocity_per_week > 0 ? "+" : ""}
                {r.velocity_per_week}%/wk
              </span>
              {r.target_date && (
                <span className="font-mono text-[11px] text-muted">→ {r.target_date.slice(5)}</span>
              )}
              <span className="ml-auto font-mono text-[11px] text-muted">
                {r.days_since_touch != null ? `${r.days_since_touch}d` : "—"}
              </span>
              {r.blocked_count > 0 && (
                <span className="font-mono text-[11px] text-st-late">⛔ {r.blocked_count}</span>
              )}
            </button>
          ))}
          {(rows ?? []).length === 0 && (
            <p className="mt-12 text-center text-muted">
              No active projects. Set a project's status to Active and it shows
              up here with velocity and health.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
