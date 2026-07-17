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
  const { view, projectId, setNewProjectOpen } = useUi();
  const { push } = useToasts();
  const qc = useQueryClient();
  const { data: root, isLoading } = useQuery({
    queryKey: ["root"],
    queryFn: api.getRoot,
  });

  // Disk is truth: when the watcher sees changes, refetch everything visible.
  useEffect(() => {
    const unlistenFs = listen("fs-changed", () => {
      qc.invalidateQueries();
    });
    const unlistenSweep = listen<string>("swept", (e) => {
      push(`Swept ${e.payload} → Inbox`);
      qc.invalidateQueries({ queryKey: ["inbox"] });
    });
    return () => {
      unlistenFs.then((fn) => fn());
      unlistenSweep.then((fn) => fn());
    };
  }, [qc, push]);

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
      <NewProjectModal />
      <CommandPalette />
      <AiResultModal />
      <Toasts />
    </div>
  );
}
