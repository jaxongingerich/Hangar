const COPY: Record<string, { title: string; body: string }> = {
  today: {
    title: "Today",
    body: "Your morning screen — overdue and due-today tasks, arriving orders, and a suggested next action per project.",
  },
  inbox: {
    title: "Inbox",
    body: "Drop files into _Inbox and file them into project bins with rules or AI.",
  },
  progress: {
    title: "Progress",
    body: "All active projects ranked, with velocity, deadlines, and blockers.",
  },
  space: {
    title: "Space",
    body: "Treemaps, duplicate finder, stale projects, and disk health.",
  },
  settings: {
    title: "Settings",
    body: "Roots, rules, AI providers, notifications, and backups.",
  },
};

export function Placeholder({ view }: { view: string }) {
  const copy = COPY[view] ?? { title: view, body: "" };
  return (
    <div className="flex flex-1 items-center justify-center">
      <div className="max-w-[360px] text-center">
        <div className="mb-1 text-[16px] font-semibold">{copy.title}</div>
        <p className="leading-relaxed text-muted">{copy.body}</p>
        <p className="mt-3 font-mono text-[11px] text-muted">
          Coming in the next build phase
        </p>
      </div>
    </div>
  );
}
