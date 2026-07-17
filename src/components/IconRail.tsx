import { useUi, View } from "../lib/store";

type RailView = Exclude<View, "project">;

const ICONS: Record<RailView, { label: string; path: string }> = {
  dashboard: {
    label: "Dashboard",
    path: "M3 3h7v9H3zM14 3h7v5h-7zM14 12h7v9h-7zM3 16h7v5H3z",
  },
  today: {
    label: "Today",
    path: "M8 2v3M16 2v3M3.5 9h17M5 5h14a1.5 1.5 0 0 1 1.5 1.5v13A1.5 1.5 0 0 1 19 21H5a1.5 1.5 0 0 1-1.5-1.5v-13A1.5 1.5 0 0 1 5 5Z",
  },
  inbox: {
    label: "Inbox",
    path: "M3 13h4l2 3h6l2-3h4M5 6h14l2 7v6a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-6l2-7Z",
  },
  progress: {
    label: "Progress",
    path: "M4 20V10M10 20V4M16 20v-8M22 20H2",
  },
  space: {
    label: "Space",
    path: "M12 2a10 10 0 1 0 10 10H12V2Z M15 2.5A10 10 0 0 1 21.5 9H15V2.5Z",
  },
  settings: {
    label: "Settings",
    path: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1 1.55V21a2 2 0 1 1-4 0v-.09A1.7 1.7 0 0 0 9 19.36a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.55-1H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.64 9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34H9a1.7 1.7 0 0 0 1-1.55V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1 1.55 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87V9a1.7 1.7 0 0 0 1.55 1H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.55 1Z",
  },
};

const ORDER: RailView[] = ["dashboard", "today", "inbox", "progress", "space", "settings"];

export function IconRail() {
  const { view, setView } = useUi();
  return (
    <nav className="flex w-14 shrink-0 flex-col items-center gap-1 border-r border-line py-3">
      {ORDER.map((v) => {
        const active = view === v;
        return (
          <button
            key={v}
            onClick={() => setView(v)}
            title={ICONS[v].label}
            aria-label={ICONS[v].label}
            className={`flex h-10 w-10 items-center justify-center rounded-lg transition-colors ${
              active ? "bg-panel-2 text-solder" : "text-muted hover:bg-panel hover:text-text"
            }`}
          >
            <svg
              width="18"
              height="18"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.8"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d={ICONS[v].path} />
            </svg>
          </button>
        );
      })}
    </nav>
  );
}
