import { useEffect, useState } from "react";
import { Command } from "cmdk";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../lib/api";
import { useToasts, useUi, View } from "../lib/store";

const VIEWS: { view: View; label: string }[] = [
  { view: "dashboard", label: "Go to Dashboard" },
  { view: "today", label: "Go to Today" },
  { view: "inbox", label: "Go to Inbox" },
  { view: "progress", label: "Go to Progress" },
  { view: "space", label: "Go to Space" },
  { view: "settings", label: "Go to Settings" },
];

export function CommandPalette() {
  const { paletteOpen, setPaletteOpen, setView, openProject, setNewProjectOpen, projectId, view } =
    useUi();
  const [query, setQuery] = useState("");
  const qc = useQueryClient();
  const { push } = useToasts();

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.metaKey && e.key === "k") {
        e.preventDefault();
        setPaletteOpen(!paletteOpen);
      }
      if (e.key === "Escape") setPaletteOpen(false);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [paletteOpen, setPaletteOpen]);

  const { data: hits } = useQuery({
    queryKey: ["search", query],
    queryFn: () => api.search(query),
    enabled: paletteOpen && query.trim().length > 0,
  });

  const close = () => {
    setPaletteOpen(false);
    setQuery("");
  };

  // "set progress 60" command when inside a project.
  const progressMatch = query.match(/^set progress (\d{1,3})$/i);

  if (!paletteOpen) return null;

  return (
    <div
      className="absolute inset-0 z-50 flex items-start justify-center bg-black/40 pt-[14vh]"
      onClick={close}
    >
      <Command
        label="Command palette"
        shouldFilter={query.trim().length === 0 || !hits}
        className="w-[560px] overflow-hidden rounded-panel border border-line bg-panel shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <Command.Input
          autoFocus
          value={query}
          onValueChange={setQuery}
          placeholder="Search projects, files, logs — or type a command…"
          className="w-full border-b border-line bg-transparent px-4 py-3.5 text-[14px] placeholder:text-muted focus:outline-none"
        />
        <Command.List className="max-h-[380px] overflow-y-auto p-1.5">
          <Command.Empty className="px-3 py-6 text-center text-[12px] text-muted">
            No results.
          </Command.Empty>

          {progressMatch && projectId !== null && view === "project" && (
            <Item
              onSelect={async () => {
                await api.setProgress(projectId, Number(progressMatch[1]));
                qc.invalidateQueries();
                push(`Progress → ${progressMatch[1]}%`);
                close();
              }}
            >
              <span className="text-solder">▸</span> Set progress to{" "}
              {progressMatch[1]}%
            </Item>
          )}

          {(hits ?? []).map((h) => (
            <Item
              key={`${h.kind}-${h.id}`}
              value={`${h.kind}-${h.id}-${h.title}`}
              onSelect={() => {
                openProject(h.project_id);
                close();
              }}
            >
              <span className="w-4 text-center">
                {h.kind === "project" ? "📦" : h.kind === "file" ? "📄" : "📝"}
              </span>
              <span className="truncate">{h.title}</span>
              <span className="ml-auto max-w-[220px] truncate font-mono text-[10px] text-muted">
                {h.subtitle.replace(/^\/Users\/[^/]+/, "~")}
              </span>
            </Item>
          ))}

          {query.trim().length === 0 && (
            <>
              <Command.Group
                heading="Commands"
                className="px-1.5 pb-1 pt-2 text-[10px] font-medium uppercase tracking-wide text-muted"
              >
                <Item
                  onSelect={() => {
                    setNewProjectOpen(true);
                    close();
                  }}
                >
                  ＋ New project
                </Item>
                <Item
                  onSelect={async () => {
                    const stats = await api.rescan();
                    qc.invalidateQueries();
                    push(`Rescanned ${stats.projects} projects, ${stats.files} files in ${stats.elapsed_ms}ms`);
                    close();
                  }}
                >
                  ⟳ Rebuild index from disk
                </Item>
              </Command.Group>
              <Command.Group
                heading="Views"
                className="px-1.5 pb-1 pt-2 text-[10px] font-medium uppercase tracking-wide text-muted"
              >
                {VIEWS.map((v) => (
                  <Item
                    key={v.view}
                    onSelect={() => {
                      setView(v.view);
                      close();
                    }}
                  >
                    {v.label}
                  </Item>
                ))}
              </Command.Group>
            </>
          )}
        </Command.List>
      </Command>
    </div>
  );
}

function Item({
  children,
  onSelect,
  value,
}: {
  children: React.ReactNode;
  onSelect: () => void;
  value?: string;
}) {
  return (
    <Command.Item
      value={value}
      onSelect={onSelect}
      className="flex cursor-default items-center gap-2 rounded-md px-2.5 py-2 text-[13px] data-[selected=true]:bg-panel-2"
    >
      {children}
    </Command.Item>
  );
}
