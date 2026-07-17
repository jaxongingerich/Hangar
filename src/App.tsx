import { useQuery } from "@tanstack/react-query";
import { api } from "./lib/api";
import { useUi } from "./lib/store";
import { TitleBar } from "./components/TitleBar";
import { IconRail } from "./components/IconRail";
import { NewProjectModal } from "./components/NewProjectModal";
import { Dashboard } from "./views/Dashboard";
import { RootPicker } from "./views/RootPicker";
import { Placeholder } from "./views/Placeholder";

export default function App() {
  const { view } = useUi();
  const { data: root, isLoading } = useQuery({
    queryKey: ["root"],
    queryFn: api.getRoot,
  });

  return (
    <div className="relative flex h-full flex-col bg-ink text-text">
      <TitleBar root={root ?? null} />
      {isLoading ? null : root ? (
        <div className="flex flex-1 overflow-hidden">
          <IconRail />
          {view === "dashboard" ? <Dashboard /> : <Placeholder view={view} />}
        </div>
      ) : (
        <RootPicker />
      )}
      <NewProjectModal />
    </div>
  );
}
