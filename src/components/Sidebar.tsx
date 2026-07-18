import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api } from "../lib/api";
import { useToasts, useUi, View } from "../lib/store";

type NavView = Exclude<View, "project">;

const SECTIONS: { heading: string | null; items: { view: NavView; label: string }[] }[] = [
  {
    heading: null,
    items: [
      { view: "dashboard", label: "Projects" },
      { view: "today", label: "Today" },
      { view: "inbox", label: "Inbox" },
    ],
  },
  {
    heading: "Assist",
    items: [{ view: "assistant", label: "AI" }],
  },
  {
    heading: "Insight",
    items: [
      { view: "progress", label: "Progress" },
      { view: "space", label: "Space" },
    ],
  },
];

const ORDER: NavView[] = [
  "dashboard",
  "today",
  "inbox",
  "assistant",
  "progress",
  "space",
  "settings",
];

export function Sidebar() {
  const { view, setView, projectId, activeBinId, setNewProjectOpen } = useUi();
  const { push } = useToasts();
  const qc = useQueryClient();

  // ⌘1–⌘7 jump straight to a section.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!e.metaKey || e.altKey || e.ctrlKey || e.shiftKey) return;
      const idx = Number(e.key) - 1;
      if (idx >= 0 && idx < ORDER.length) {
        e.preventDefault();
        setView(ORDER[idx]);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [setView]);

  const importFiles = async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({ multiple: true, title: "Import files" });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    const toProject = view === "project" && projectId !== null;
    const n = await api.importFiles(
      paths,
      toProject ? projectId : null,
      toProject ? activeBinId : null,
    );
    push(
      toProject
        ? `Imported ${n} file${n === 1 ? "" : "s"} into this project`
        : `Imported ${n} file${n === 1 ? "" : "s"} into the Inbox`,
    );
    qc.invalidateQueries();
    if (!toProject) setView("inbox");
  };

  const navButton = (v: NavView, label: string) => {
    const active = view === v || (view === "project" && v === "dashboard");
    return (
      <button
        key={v}
        onClick={() => setView(v)}
        title={`${label} ⌘${ORDER.indexOf(v) + 1}`}
        aria-current={active ? "page" : undefined}
        className={`w-full rounded-md px-3 py-1.5 text-left text-[12.5px] transition-colors ${
          active
            ? "bg-panel-2 font-semibold text-text"
            : "font-medium text-muted hover:bg-panel hover:text-text"
        }`}
      >
        {label}
      </button>
    );
  };

  return (
    <nav className="flex w-[164px] shrink-0 flex-col border-r border-line px-2.5 py-3">
      <div className="mb-3 flex flex-col gap-1.5">
        <button
          onClick={() => setNewProjectOpen(true)}
          title="New project ⌘N"
          className="w-full rounded-md bg-solder px-3 py-1.5 text-left text-[12.5px] font-semibold text-on-accent transition-opacity hover:opacity-90"
        >
          New project
        </button>
        <button
          onClick={importFiles}
          title="Copy files into Hangar"
          className="w-full rounded-md border border-line px-3 py-1.5 text-left text-[12.5px] font-medium text-muted transition-colors hover:border-line-strong hover:text-text"
        >
          Import files
        </button>
      </div>

      {SECTIONS.map((section, i) => (
        <div key={i} className="mb-1.5 flex flex-col gap-0.5">
          {section.heading && (
            <div className="px-3 pb-0.5 pt-2 text-[10px] font-medium uppercase tracking-wide text-muted">
              {section.heading}
            </div>
          )}
          {section.items.map((item) => navButton(item.view, item.label))}
        </div>
      ))}

      <div className="mt-auto flex flex-col gap-0.5 border-t border-line pt-2">
        {navButton("settings", "Settings")}
      </div>
    </nav>
  );
}
