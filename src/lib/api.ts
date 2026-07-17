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

export interface BinInfo {
  id: number;
  name: string;
  rel_path: string;
  file_count: number;
  size_bytes: number;
}

export interface ProjectDetail {
  id: number;
  name: string;
  path: string;
  emoji: string;
  color: string;
  status: ProjectCard["status"];
  progress: number;
  progress_mode: "manual" | "milestones";
  target_date: string | null;
  pinned: boolean;
  bins: BinInfo[];
  root_file_count: number;
}

export interface FileRow {
  id: number;
  bin_id: number | null;
  rel_path: string;
  name: string;
  ext: string | null;
  size: number;
  mtime: number;
  pinned: boolean;
  abs_path: string;
}

export interface InboxItem {
  path: string;
  name: string;
  size: number;
  mtime: number;
  suggested_bin_id: number | null;
  suggested_bin_name: string | null;
}

export interface RuleRow {
  id: number;
  project_id: number | null;
  pattern: string;
  match_kind: "ext" | "glob" | "regex";
  dest_bin_id: number | null;
  dest_bin_name: string | null;
  enabled: boolean;
}

export interface LogRow {
  id: number;
  ts: string;
  kind: "note" | "auto" | "status_report" | "digest";
  body_md: string;
}

export interface SearchHit {
  kind: "project" | "file" | "log";
  id: number;
  project_id: number;
  title: string;
  subtitle: string;
}

export interface IdeaRow {
  id: number;
  name: string;
  note: string | null;
  created_at: string;
}

export interface ProjectPatch {
  name?: string;
  emoji?: string;
  color?: string;
  status?: string;
  target_date?: string | null;
  pinned?: boolean;
}

export const api = {
  getRoot: () => invoke<string | null>("get_root"),
  defaultRoot: () => invoke<string>("default_root"),
  setRoot: (path: string) => invoke<ScanStats>("set_root", { path }),
  rescan: () => invoke<ScanStats>("rescan"),
  listProjects: () => invoke<ProjectCard[]>("list_projects"),
  createProject: (name: string, template?: string) =>
    invoke<ScanStats>("create_project", { name, template }),

  getProject: (id: number) => invoke<ProjectDetail>("get_project", { id }),
  listFiles: (projectId: number, binId?: number | null, rootOnly?: boolean) =>
    invoke<FileRow[]>("list_files", { projectId, binId, rootOnly }),
  renameFile: (fileId: number, newName: string) =>
    invoke<void>("rename_file", { fileId, newName }),
  moveFiles: (fileIds: number[], destBinId: number | null) =>
    invoke<number>("move_files", { fileIds, destBinId }),
  trashFiles: (fileIds: number[]) => invoke<number>("trash_files", { fileIds }),
  togglePinFile: (fileId: number) => invoke<void>("toggle_pin_file", { fileId }),
  quickLook: (path: string) => invoke<void>("quick_look", { path }),

  createBin: (projectId: number, name: string) =>
    invoke<number>("create_bin", { projectId, name }),
  renameBin: (binId: number, newName: string) =>
    invoke<void>("rename_bin", { binId, newName }),
  trashBin: (binId: number) => invoke<void>("trash_bin", { binId }),

  listInbox: (projectId: number | null) =>
    invoke<InboxItem[]>("list_inbox", { projectId }),
  fileInbox: (items: { path: string; project_id: number; bin_id: number | null }[]) =>
    invoke<number>("file_inbox", { items }),

  listRules: () => invoke<RuleRow[]>("list_rules"),
  saveRule: (rule: {
    id?: number;
    projectId?: number | null;
    pattern: string;
    matchKind: string;
    destBinId?: number | null;
    enabled: boolean;
  }) => invoke<number>("save_rule", rule),
  deleteRule: (id: number) => invoke<void>("delete_rule", { id }),
  testRule: (pattern: string, matchKind: string, samples: string[]) =>
    invoke<boolean[]>("test_rule", { pattern, matchKind, samples }),

  listLogs: (projectId: number) => invoke<LogRow[]>("list_logs", { projectId }),
  addLog: (projectId: number, body: string) =>
    invoke<void>("add_log", { projectId, body }),

  setProgress: (projectId: number, value: number) =>
    invoke<void>("set_progress", { projectId, value }),
  updateProject: (projectId: number, patch: ProjectPatch) =>
    invoke<void>("update_project", { projectId, patch }),

  search: (query: string) => invoke<SearchHit[]>("search", { query }),

  listIdeas: () => invoke<IdeaRow[]>("list_ideas"),
  createIdea: (name: string, note?: string) =>
    invoke<number>("create_idea", { name, note }),
  deleteIdea: (id: number) => invoke<void>("delete_idea", { id }),
};
