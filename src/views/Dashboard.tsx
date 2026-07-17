import { useQuery } from "@tanstack/react-query";
import { api, ProjectCard as Card } from "../lib/api";
import { SortMode, useUi } from "../lib/store";
import { ProjectCard } from "../components/ProjectCard";

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
  const { sort, setSort, setNewProjectOpen } = useUi();
  const { data: projects, isLoading } = useQuery({
    queryKey: ["projects"],
    queryFn: api.listProjects,
  });

  const cards = sortCards(projects ?? [], sort);
  const active = cards.filter((c) => c.status === "active").length;

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div className="flex items-center gap-3 px-6 pb-4 pt-5">
        <h1 className="text-[20px] font-semibold">Dashboard</h1>
        {projects && (
          <span className="font-mono text-[11px] text-muted">
            {cards.length} projects · {active} active
          </span>
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
              <ProjectCard key={p.id} project={p} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
