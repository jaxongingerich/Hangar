import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { api } from "./lib/api";
import { useUi } from "./lib/store";
import { TitleBar } from "./components/TitleBar";
import { IconRail } from "./components/IconRail";
import { NewProjectModal } from "./components/NewProjectModal";
import { CommandPalette } from "./components/CommandPalette";
import { Toasts } from "./components/Toasts";
import { AiResultModal } from "./components/AiResultModal";
import { Dashboard } from "./views/Dashboard";
import { RootPicker } from "./views/RootPicker";
import { Placeholder } from "./views/Placeholder";
import { Inbox } from "./views/Inbox";
import { Settings } from "./views/Settings";
import { ProjectView } from "./views/project/ProjectView";
import { Today } from "./views/Today";
import { GlobalProgress } from "./views/GlobalProgress";
import { Space } from "./views/Space";
import { useToasts } from "./lib/store";

export default function App() {
  const { view, projectId, setNewProjectOpen, activeBinId } = useUi();
  const { push } = useToasts();
  const qc = useQueryClient();
  const [dropActive, setDropActive] = useState(false);
  const { data: root, isLoading } = useQuery({
    queryKey: ["root"],
    queryFn: api.getRoot,
  });

  // Universal import: drag any files from Finder onto the window.
  // In a project they land in the open bin; anywhere else, in _Inbox.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      const { getCurrentWebview } = await import("@tauri-apps/api/webview");
      unlisten = await getCurrentWebview().onDragDropEvent(async (event) => {
        const t = event.payload.type;
        if (t === "over" || t === "enter") setDropActive(true);
        if (t === "leave") setDropActive(false);
        if (t === "drop") {
          setDropActive(false);
          const paths = (event.payload as { paths: string[] }).paths ?? [];
          if (paths.length === 0) return;
          try {
            const toProject = view === "project" && projectId !== null;
            const n = await api.importFiles(
              paths,
              toProject ? projectId : null,
              toProject ? activeBinId : null,
            );
            push(
              toProject
                ? `Imported ${n} file${n === 1 ? "" : "s"} (copied)`
                : `Imported ${n} file${n === 1 ? "" : "s"} → Inbox`,
            );
            qc.invalidateQueries();
          } catch (e) {
            push(String(e), "error");
          }
        }
      });
    })();
    return () => unlisten?.();
  }, [view, projectId, activeBinId, push, qc]);

  // Disk is truth: when the watcher sees changes, refetch everything visible.
  useEffect(() => {
    const unlistenFs = listen("fs-changed", () => {
      qc.invalidateQueries();
    });
    const unlistenSweep = listen<string>("swept", (e) => {
      push(`Swept ${e.payload} → Inbox`);
      qc.invalidateQueries({ queryKey: ["inbox"] });
    });
    const unlistenIdea = listen("tray-new-idea", async () => {
      const name = prompt("New idea:");
      if (name?.trim()) {
        await api.createIdea(name.trim());
        push("Idea captured");
        qc.invalidateQueries({ queryKey: ["ideas"] });
      }
    });
    const unlistenProject = listen("tray-new-project", () => {
      setNewProjectOpen(true);
    });
    return () => {
      unlistenFs.then((fn) => fn());
      unlistenSweep.then((fn) => fn());
      unlistenIdea.then((fn) => fn());
      unlistenProject.then((fn) => fn());
    };
  }, [qc, push, setNewProjectOpen]);

  // Global hotkey: ⌘⇧H raises Hangar from anywhere.
  useEffect(() => {
    let cleanup = () => {};
    (async () => {
      try {
        const { register, unregister } = await import("@tauri-apps/plugin-global-shortcut");
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        await register("CmdOrCtrl+Shift+H", async (event) => {
          if (event.state === "Pressed") {
            const win = getCurrentWindow();
            await win.show();
            await win.setFocus();
          }
        });
        cleanup = () => {
          unregister("CmdOrCtrl+Shift+H").catch(() => {});
        };
      } catch {
        // Shortcut may already be registered by another instance.
      }
    })();
    return () => cleanup();
  }, []);

  // One notification per session for overdue work.
  useEffect(() => {
    if (!root) return;
    (async () => {
      try {
        const data = await api.todayData();
        const overdue = data.overdue.length;
        if (overdue === 0) return;
        const { isPermissionGranted, requestPermission, sendNotification } =
          await import("@tauri-apps/plugin-notification");
        let granted = await isPermissionGranted();
        if (!granted) granted = (await requestPermission()) === "granted";
        if (granted) {
          sendNotification({
            title: "Hangar",
            body: `${overdue} task${overdue === 1 ? "" : "s"} overdue — open Today to triage.`,
          });
        }
      } catch {
        // Notifications are best-effort.
      }
    })();
  }, [root]);

  // Global shortcuts that aren't view-specific.
  useEffect(() => {
    const handler = async (e: KeyboardEvent) => {
      if (e.metaKey && e.key === "n") {
        e.preventDefault();
        setNewProjectOpen(true);
      }
      // ⌘Z outside a text field = undo the last file operation.
      if (
        e.metaKey &&
        e.key === "z" &&
        !(e.target instanceof HTMLInputElement) &&
        !(e.target instanceof HTMLTextAreaElement)
      ) {
        e.preventDefault();
        const undone = await api.undoLastOp();
        if (undone) {
          push(`Undone: ${undone}`);
          qc.invalidateQueries();
        } else {
          push("Nothing to undo");
        }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [setNewProjectOpen, push, qc]);

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
      case "space":
        return <Space />;
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
      {dropActive && (
        <div className="pointer-events-none absolute inset-2 z-[70] flex items-center justify-center rounded-xl border-2 border-dashed border-solder bg-ink/70">
          <div className="rounded-lg bg-panel px-5 py-3 text-[14px] font-medium shadow-xl">
            Drop to import{" "}
            {view === "project" ? "into this project" : "into the Inbox"} —
            files are copied, never moved
          </div>
        </div>
      )}
      <NewProjectModal />
      <CommandPalette />
      <AiResultModal />
      <Toasts />
    </div>
  );
}
