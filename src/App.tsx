import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { api } from "./lib/api";
import { useUi } from "./lib/store";
import { TitleBar } from "./components/TitleBar";
import { IconRail } from "./components/IconRail";
import { NewProjectModal } from "./components/NewProjectModal";
import { CommandPalette } from "./components/CommandPalette";
import { Toasts } from "./components/Toasts";
import { Dashboard } from "./views/Dashboard";
import { RootPicker } from "./views/RootPicker";
import { Placeholder } from "./views/Placeholder";
import { Inbox } from "./views/Inbox";
import { Settings } from "./views/Settings";
import { ProjectView } from "./views/project/ProjectView";
import { Today } from "./views/Today";
import { GlobalProgress } from "./views/GlobalProgress";

export default function App() {
  const { view, projectId, setNewProjectOpen } = useUi();
  const qc = useQueryClient();
  const { data: root, isLoading } = useQuery({
    queryKey: ["root"],
    queryFn: api.getRoot,
  });

  // Disk is truth: when the watcher sees changes, refetch everything visible.
  useEffect(() => {
    const unlisten = listen("fs-changed", () => {
      qc.invalidateQueries();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [qc]);

  // Global shortcuts that aren't view-specific.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.metaKey && e.key === "n") {
        e.preventDefault();
        setNewProjectOpen(true);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [setNewProjectOpen]);

  const content = () => {
    switch (view) {
      case "dashboard":
        return <Dashboard />;
      case "project":
        return projectId !== null ? (
          <ProjectView key={projectId} projectId={projectId} />
        ) : (
          <Dashboard />
        );
      case "inbox":
        return <Inbox />;
      case "today":
        return <Today />;
      case "progress":
        return <GlobalProgress />;
      case "settings":
        return <Settings root={root ?? null} />;
      default:
        return <Placeholder view={view} />;
    }
  };

  return (
    <div className="relative flex h-full flex-col bg-ink text-text">
      <TitleBar root={root ?? null} />
      {isLoading ? null : root ? (
        <div className="flex flex-1 overflow-hidden">
          <IconRail />
          {content()}
        </div>
      ) : (
        <RootPicker />
      )}
      <NewProjectModal />
      <CommandPalette />
      <Toasts />
    </div>
  );
}
