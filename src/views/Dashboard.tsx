import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, ProjectCard as Card } from "../lib/api";
import { SortMode, useToasts, useUi } from "../lib/store";
import { ProjectCard } from "../components/ProjectCard";
import { formatBytes } from "../lib/format";

const SORTS: { id: SortMode; label: string }[] = [
  { id: "recent", label: "Recent" },
  { id: "progress", label: "Progress" },
  { id: "size", label: "Size" },
  { id: "deadline", label: "Deadline" },
  { id: "name", label: "Name" },
];

function sortCards(cards: Card[], mode: SortMode): Card[] {
  const sorted = [...cards];
  const cmp: Record<SortMode, (a: Card, b: Card) => number> = {
    recent: (a, b) => (b.last_touch_ms ?? 0) - (a.last_touch_ms ?? 0),
    progress: (a, b) => b.progress - a.progress,
    size: (a, b) => b.size_bytes - a.size_bytes,
    deadline: (a, b) =>
      (a.target_date ?? "9999").localeCompare(b.target_date ?? "9999"),
    name: (a, b) => a.name.localeCompare(b.name),
  };
  sorted.sort((a, b) => {
    if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
    return cmp[mode](a, b);
  });
  return sorted;
}

export function Dashboard() {
  const { sort, setSort, setNewProjectOpen, openProject } = useUi();
  const { data: projects, isLoading } = useQuery({
    queryKey: ["projects"],
    queryFn: api.listProjects,
  });

  const { data: rollup } = useQuery({
    queryKey: ["rollup"],
    queryFn: api.healthRollup,
  });

  const cards = sortCards(projects ?? [], sort);

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div className="flex items-center gap-3 px-6 pb-4 pt-5">
        <h1 className="text-[20px] font-semibold">Dashboard</h1>
        {rollup && (
          <div className="flex items-center gap-3 font-mono text-[11px] text-muted">
            <span>{rollup.active} active</span>
            {rollup.at_risk > 0 && (
              <span className="text-st-risk">{rollup.at_risk} at risk</span>
            )}
            {rollup.late > 0 && <span className="text-st-late">{rollup.late} late</span>}
            {rollup.open_orders > 0 && (
              <span>
                {rollup.open_orders} orders · ${(rollup.in_flight_cents / 100).toFixed(0)} in flight
              </span>
            )}
            {rollup.hours_this_week > 0 && <span>{rollup.hours_this_week}h this week</span>}
            <span>{formatBytes(rollup.disk_free_bytes)} free</span>
          </div>
        )}
        <div className="ml-auto flex items-center gap-2">
          <div className="flex rounded-lg border border-line p-0.5">
            {SORTS.map((s) => (
              <button
                key={s.id}
                onClick={() => setSort(s.id)}
                className={`rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors ${
                  sort === s.id
                    ? "bg-panel-2 text-text"
                    : "text-muted hover:text-text"
                }`}
              >
                {s.label}
              </button>
            ))}
          </div>
          <button
            onClick={() => setNewProjectOpen(true)}
            className="rounded-lg bg-solder px-3.5 py-1.5 text-[12px] font-semibold text-ink transition-opacity hover:opacity-90"
          >
            New project
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-6 pb-6">
        {isLoading ? (
          <p className="mt-12 text-center font-mono text-[12px] text-muted">
            Loading…
          </p>
        ) : cards.length === 0 ? (
          <div className="mx-auto mt-16 max-w-[360px] text-center">
            <div className="mb-2 text-[28px]">🛩️</div>
            <div className="mb-1 text-[16px] font-semibold">
              The hangar is empty
            </div>
            <p className="mb-5 leading-relaxed text-muted">
              Create your first project and Hangar will lay out its folders on
              disk — Gerbers, Firmware, CAD and the rest — ready for files.
            </p>
            <button
              onClick={() => setNewProjectOpen(true)}
              className="rounded-lg bg-solder px-4 py-2 text-[13px] font-semibold text-ink transition-opacity hover:opacity-90"
            >
              New project
            </button>
          </div>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-3">
            {cards.map((p) => (
              <ProjectCard
                key={p.id}
                project={p}
                onClick={() => openProject(p.id)}
              />
            ))}
          </div>
        )}
        <IdeaBacklog />
      </div>
    </div>
  );
}

function IdeaBacklog() {
  const [draft, setDraft] = useState("");
  const qc = useQueryClient();
  const { push } = useToasts();
  const { data: ideas } = useQuery({ queryKey: ["ideas"], queryFn: api.listIdeas });

  const create = useMutation({
    mutationFn: () => api.createIdea(draft.trim()),
    onSuccess: () => {
      setDraft("");
      qc.invalidateQueries({ queryKey: ["ideas"] });
    },
  });

  const promote = useMutation({
    mutationFn: async (idea: { id: number; name: string }) => {
      await api.createProject(idea.name, "hardware");
      await api.deleteIdea(idea.id);
    },
    onSuccess: () => {
      push("Idea promoted to a project");
      qc.invalidateQueries();
    },
    onError: (e) => push(String(e), "error"),
  });

  const remove = useMutation({
    mutationFn: (id: number) => api.deleteIdea(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["ideas"] }),
  });

  return (
    <div className="mt-8">
      <div className="mb-2 flex items-baseline gap-2">
        <h2 className="text-[14px] font-semibold">Idea backlog</h2>
        <span className="font-mono text-[11px] text-muted">
          {ideas?.length ?? 0}
        </span>
      </div>
      <div className="flex flex-col gap-1.5">
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && draft.trim()) create.mutate();
          }}
          placeholder="Capture an idea — no folder until you promote it"
          className="w-full max-w-[480px] rounded-lg border border-line bg-panel px-3 py-2 text-[12.5px] placeholder:text-muted focus:border-solder focus:outline-none"
        />
        {(ideas ?? []).map((idea) => (
          <div
            key={idea.id}
            className="flex max-w-[480px] items-center gap-2 rounded-lg border border-line/60 bg-panel px-3 py-2"
          >
            <span className="text-st-idea">◌</span>
            <span className="flex-1 truncate text-[12.5px]">{idea.name}</span>
            <button
              onClick={() => promote.mutate(idea)}
              className="rounded-md border border-line px-2 py-0.5 text-[11px] text-muted transition-colors hover:border-solder hover:text-solder"
            >
              Promote
            </button>
            <button
              onClick={() => remove.mutate(idea.id)}
              className="rounded-md px-1.5 py-0.5 text-[11px] text-muted hover:text-st-late"
              title="Delete idea"
            >
              ✕
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
