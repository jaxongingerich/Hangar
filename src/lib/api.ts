import { invoke } from "@tauri-apps/api/core";

export interface ScanStats {
  projects: number;
  files: number;
  elapsed_ms: number;
}

export interface ProjectCard {
  id: number;
  slug: string;
  name: string;
  path: string;
  emoji: string;
  color: string;
  status: "idea" | "active" | "paused" | "shipped" | "archived";
  progress: number;
  pinned: boolean;
  target_date: string | null;
  file_count: number;
  size_bytes: number;
  last_touch_ms: number | null;
  spine: number[];
}

export const api = {
  getRoot: () => invoke<string | null>("get_root"),
  defaultRoot: () => invoke<string>("default_root"),
  setRoot: (path: string) => invoke<ScanStats>("set_root", { path }),
  rescan: () => invoke<ScanStats>("rescan"),
  listProjects: () => invoke<ProjectCard[]>("list_projects"),
  createProject: (name: string, template?: string) =>
    invoke<ScanStats>("create_project", { name, template }),
};
