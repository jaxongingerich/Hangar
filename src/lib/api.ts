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

export interface MilestoneRow {
  id: number;
  title: string;
  state: "todo" | "doing" | "done";
  weight: number;
  sort_order: number;
  task_count: number;
  done_task_count: number;
}

export interface TaskRow {
  id: number;
  project_id: number;
  project_name: string;
  project_emoji: string;
  milestone_id: number | null;
  title: string;
  done: boolean;
  due: string | null;
  priority: "low" | "med" | "high";
  blocked: boolean;
  blocked_reason: string | null;
  recurrence: string | null;
}

export interface HistoryPoint {
  ts: string;
  value: number;
}

export interface ProgressStats {
  history: HistoryPoint[];
  velocity_per_week: number;
  projected_finish: string | null;
  health: "on_track" | "at_risk" | "late";
  days_since_touch: number | null;
  hours_this_week: number;
  heatmap: number[];
  blocked_count: number;
}

export interface OrderRow {
  id: number;
  project_id: number;
  project_name: string;
  vendor: string;
  ref: string | null;
  items: string | null;
  cost_cents: number;
  currency: string;
  ordered_at: string;
  eta: string | null;
  status: "ordered" | "shipped" | "arrived" | "issue";
  tracking_url: string | null;
  notes: string | null;
}

export interface SpendSummary {
  total_cents: number;
  in_flight_cents: number;
  by_month: [string, number][];
}

export interface LinkRow {
  id: number;
  title: string;
  url: string;
  kind: string;
}

export interface GitBadge {
  branch: string;
  dirty: boolean;
}

export interface ActiveTimer {
  project_id: number;
  project_name: string;
  started_at: string;
}

export interface TodayData {
  overdue: TaskRow[];
  due_today: TaskRow[];
  high_priority: TaskRow[];
  arriving: OrderRow[];
  suggestions: [number, string, string, string][];
}

export interface PortfolioRow {
  id: number;
  emoji: string;
  name: string;
  color: string;
  progress: number;
  health: "on_track" | "at_risk" | "late";
  velocity_per_week: number;
  target_date: string | null;
  projected_finish: string | null;
  days_since_touch: number | null;
  blocked_count: number;
  history: HistoryPoint[];
}

export interface HealthRollup {
  active: number;
  at_risk: number;
  late: number;
  open_orders: number;
  in_flight_cents: number;
  disk_free_bytes: number;
  hours_this_week: number;
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

  listMilestones: (projectId: number) =>
    invoke<MilestoneRow[]>("list_milestones", { projectId }),
  addMilestone: (projectId: number, title: string, weight?: number) =>
    invoke<number>("add_milestone", { projectId, title, weight }),
  setMilestoneState: (milestoneId: number, newState: string) =>
    invoke<void>("set_milestone_state", { milestoneId, newState }),
  updateMilestone: (milestoneId: number, title?: string, weight?: number) =>
    invoke<void>("update_milestone", { milestoneId, title, weight }),
  deleteMilestone: (milestoneId: number) =>
    invoke<void>("delete_milestone", { milestoneId }),
  applyMilestoneTemplate: (projectId: number, template: string) =>
    invoke<void>("apply_milestone_template", { projectId, template }),
  setProgressMode: (projectId: number, mode: string) =>
    invoke<void>("set_progress_mode", { projectId, mode }),

  listTasks: (projectId: number, includeDone?: boolean) =>
    invoke<TaskRow[]>("list_tasks", { projectId, includeDone }),
  addTask: (task: {
    project_id: number;
    title: string;
    due?: string | null;
    priority?: string;
    milestone_id?: number | null;
    recurrence?: string | null;
  }) => invoke<number>("add_task", { task }),
  toggleTask: (taskId: number) => invoke<void>("toggle_task", { taskId }),
  updateTask: (
    taskId: number,
    patch: Partial<{
      title: string;
      due: string | null;
      priority: string;
      blocked: boolean;
      blocked_reason: string | null;
      milestone_id: number | null;
      recurrence: string | null;
    }>,
  ) => invoke<void>("update_task", { taskId, patch }),
  deleteTask: (taskId: number) => invoke<void>("delete_task", { taskId }),

  getProgressStats: (projectId: number) =>
    invoke<ProgressStats>("get_progress_stats", { projectId }),
  draftStatusReport: (projectId: number) =>
    invoke<string>("draft_status_report", { projectId }),

  listOrders: (projectId?: number) =>
    invoke<OrderRow[]>("list_orders", { projectId }),
  addOrder: (order: {
    project_id: number;
    vendor: string;
    ref?: string | null;
    items?: string | null;
    cost_cents: number;
    eta?: string | null;
    tracking_url?: string | null;
    notes?: string | null;
  }) => invoke<number>("add_order", { order }),
  updateOrderStatus: (orderId: number, status: string) =>
    invoke<void>("update_order_status", { orderId, status }),
  deleteOrder: (orderId: number) => invoke<void>("delete_order", { orderId }),
  spendSummary: (projectId?: number) =>
    invoke<SpendSummary>("spend_summary", { projectId }),

  listLinks: (projectId: number) => invoke<LinkRow[]>("list_links", { projectId }),
  addLink: (projectId: number, title: string, url: string, kind: string) =>
    invoke<number>("add_link", { projectId, title, url, kind }),
  deleteLink: (linkId: number) => invoke<void>("delete_link", { linkId }),
  gitBadge: (projectId: number) =>
    invoke<GitBadge | null>("git_badge", { projectId }),

  startTimer: (projectId: number) => invoke<void>("start_timer", { projectId }),
  stopTimer: () => invoke<void>("stop_timer"),
  activeTimer: () => invoke<ActiveTimer | null>("active_timer"),

  todayData: () => invoke<TodayData>("today_data"),
  portfolio: () => invoke<PortfolioRow[]>("portfolio"),
  healthRollup: () => invoke<HealthRollup>("health_rollup"),

  spaceReport: () => invoke<SpaceReport>("space_report"),
  findDuplicates: () => invoke<DupeGroup[]>("find_duplicates"),
  archiveProject: (projectId: number) =>
    invoke<string>("archive_project", { projectId }),
  listArchives: () => invoke<ArchiveEntry[]>("list_archives"),
  restoreArchive: (zipPath: string) =>
    invoke<void>("restore_archive", { zipPath }),
  snapshotBin: (binId: number, label: string) =>
    invoke<number>("snapshot_bin", { binId, label }),
  listSnapshots: (projectId: number) =>
    invoke<SnapshotRow[]>("list_snapshots", { projectId }),
  diffSnapshots: (aId: number, bId: number) =>
    invoke<SnapshotDiff>("diff_snapshots", { aId, bId }),
  exportJlcpcb: (
    projectId: number,
    opts: { binId?: number; snapshotId?: number; dryRun: boolean },
  ) =>
    invoke<JlcValidation>("export_jlcpcb", {
      projectId,
      binId: opts.binId,
      snapshotId: opts.snapshotId,
      dryRun: opts.dryRun,
    }),
  normalizeBom: (fileId: number) => invoke<string>("normalize_bom", { fileId }),
  listComponents: (query?: string) =>
    invoke<ComponentRow[]>("list_components", { query }),
  saveComponent: (c: {
    id?: number;
    mpn: string;
    lcsc?: string;
    description?: string;
    package?: string;
    value?: string;
  }) => invoke<number>("save_component", c),
  deleteComponent: (id: number) => invoke<void>("delete_component", { id }),
  useComponent: (componentId: number, projectId: number, qty: number, refDes?: string) =>
    invoke<void>("use_component", { componentId, projectId, qty, refDes }),
  undoLastOp: () => invoke<string | null>("undo_last_op"),
  exportOnePager: (projectId: number) =>
    invoke<string>("export_one_pager", { projectId }),

  getFileNote: (fileId: number) =>
    invoke<string | null>("get_file_note", { fileId }),
  setFileNote: (fileId: number, body: string) =>
    invoke<void>("set_file_note", { fileId, body }),
  notedFileIds: (projectId: number) =>
    invoke<number[]>("noted_file_ids", { projectId }),
  saveClipboardFile: (
    projectId: number,
    binId: number | null,
    kind: "png" | "txt",
    dataBase64?: string,
    text?: string,
  ) =>
    invoke<string>("save_clipboard_file", {
      projectId,
      binId,
      kind,
      dataBase64,
      text,
    }),
  listCollections: () => invoke<CollectionRow[]>("list_collections"),
  saveCollection: (name: string, query: string) =>
    invoke<number>("save_collection", { name, query }),
  deleteCollection: (id: number) => invoke<void>("delete_collection", { id }),
  runCollection: (query: string) =>
    invoke<BigFile[]>("run_collection", { query }),
  getWatchedDirs: () => invoke<string[]>("get_watched_dirs"),
  setWatchedDirs: (dirs: string[]) => invoke<void>("set_watched_dirs", { dirs }),
  getSweepPatterns: () => invoke<string>("get_sweep_patterns"),
  setSweepPatterns: (patterns: string) =>
    invoke<void>("set_sweep_patterns", { patterns }),
  getFinderTags: (path: string) => invoke<string[]>("get_finder_tags", { path }),
  setFinderTags: (path: string, tags: string[]) =>
    invoke<void>("set_finder_tags", { path, tags }),
  backupStatus: () => invoke<BackupStatus>("backup_status"),
  setBackupDir: (dir: string | null) => invoke<void>("set_backup_dir", { dir }),
  runBackup: () => invoke<string>("run_backup"),
  globalTimeline: (limit?: number) =>
    invoke<TimelineRow[]>("global_timeline", { limit }),

  aiGetConfig: () => invoke<AiConfig>("ai_get_config"),
  aiSetConfig: (provider: string, model: string, baseUrl: string) =>
    invoke<void>("ai_set_config", { provider, model, baseUrl }),
  aiSetKey: (key: string) => invoke<void>("ai_set_key", { key }),
  aiTest: () => invoke<string>("ai_test"),
  aiOllamaModels: () => invoke<string[]>("ai_ollama_models"),
  aiUsage: () => invoke<AiUsage>("ai_usage"),
  aiOrganizeInbox: () => invoke<InboxPlanItem[]>("ai_organize_inbox"),
  aiSummarize: (projectId: number) =>
    invoke<string>("ai_summarize", { projectId }),
  aiAutoMilestones: (projectId: number, description: string) =>
    invoke<string[]>("ai_auto_milestones", { projectId, description }),
  aiStatusReport: (projectId: number) =>
    invoke<string>("ai_status_report", { projectId }),
  aiWeeklyDigest: () => invoke<string>("ai_weekly_digest"),
  aiSmartRename: (binId: number) =>
    invoke<RenamePlanItem[]>("ai_smart_rename", { binId }),
  aiProjectChat: (projectId: number, messages: { role: string; content: string }[]) =>
    invoke<string>("ai_project_chat", { projectId, messages }),

  aiListProfiles: () => invoke<AiProfile[]>("ai_list_profiles"),
  aiSaveProfile: (profile: {
    id: string;
    name: string;
    provider: string;
    model: string;
    base_url: string;
    command?: string;
    args?: string;
  }) =>
    invoke<string>("ai_save_profile", {
      profile: { command: "", args: "", ...profile },
    }),
  aiDeleteProfile: (id: string) => invoke<void>("ai_delete_profile", { id }),
  aiActivateProfile: (id: string) => invoke<void>("ai_activate_profile", { id }),
  aiSetProfileKey: (id: string, key: string) =>
    invoke<void>("ai_set_profile_key", { id, key }),
  aiDetectProviders: () => invoke<DetectedProvider[]>("ai_detect_providers"),

  aiListChats: () => invoke<ChatRow[]>("ai_list_chats"),
  aiNewChat: (projectId?: number | null) =>
    invoke<number>("ai_new_chat", { projectId }),
  aiUpdateChat: (
    chatId: number,
    patch: { title?: string; projectId?: number | null; clearProject?: boolean },
  ) =>
    invoke<void>("ai_update_chat", {
      chatId,
      title: patch.title,
      projectId: patch.projectId,
      clearProject: patch.clearProject,
    }),
  aiDeleteChat: (chatId: number) => invoke<void>("ai_delete_chat", { chatId }),
  aiChatHistory: (chatId: number) =>
    invoke<ChatMessageRow[]>("ai_chat_history", { chatId }),
  aiChatSend: (chatId: number, content: string, profileId: string) =>
    invoke<ChatMessageRow>("ai_chat_send", { chatId, content, profileId }),
  aiDiscoverSessions: () =>
    invoke<DiscoveredSession[]>("ai_discover_sessions"),
  aiImportSessions: (sessions: DiscoveredSession[], profileId: string | null) =>
    invoke<ImportSummary>("ai_import_sessions", { sessions, profileId }),
  aiImportExportFile: (path: string, profileId: string | null) =>
    invoke<ImportSummary>("ai_import_export_file", { path, profileId }),
  aiCliBridgeStatus: () => invoke<CliBridgeStatus[]>("ai_cli_bridge_status"),
  aiInstallCliBridge: (command: string) =>
    invoke<string>("ai_install_cli_bridge", { command }),
  readTextFile: (path: string) =>
    invoke<{ name: string; content: string; truncated: boolean }>(
      "read_text_file",
      { path },
    ),

  suggestImports: () => invoke<SuggestedImport[]>("suggest_imports"),
  mcpGetEnabled: () => invoke<boolean>("mcp_get_enabled"),
  mcpSetEnabled: (enabled: boolean) =>
    invoke<void>("mcp_set_enabled", { enabled }),

  importFiles: (paths: string[], projectId?: number | null, binId?: number | null) =>
    invoke<number>("import_files", { paths, projectId, binId }),
  readBinGerbers: (binId: number) =>
    invoke<{ filename: string; content: string }[]>("read_bin_gerbers", { binId }),
  mcpInfo: () =>
    invoke<{ url: string; token: string; install_cmd: string }>("mcp_info"),
};

export interface AiConfig {
  provider: string;
  model: string;
  base_url: string;
  has_key: boolean;
}

export interface AiProfile {
  id: string;
  name: string;
  provider: string;
  model: string;
  base_url: string;
  command: string;
  has_key: boolean;
  needs_key: boolean;
  active: boolean;
}

export interface DetectedProvider {
  id: string;
  name: string;
  provider: string;
  model: string;
  base_url: string;
  command: string;
  args: string;
  note: string;
  connectable: boolean;
}

export interface ChatRow {
  id: number;
  title: string;
  profile_id: string | null;
  project_id: number | null;
  project_name: string | null;
  message_count: number;
  updated_at: string;
  /** "hangar" for chats started here, else the importer id it came from. */
  source?: string;
}

/** A past conversation found on this Mac that Hangar can pull in. */
export interface DiscoveredSession {
  id: string;
  source: string;
  title: string;
  path: string;
  cwd: string | null;
  message_count: number;
  started_at: string;
  imported: boolean;
}

export interface ImportSummary {
  imported: number;
  skipped: number;
  messages: number;
  errors: string[];
}

export interface CliBridgeStatus {
  command: string;
  installed: boolean;
  /** The tool's data directory exists, so installing the CLI unlocks history. */
  has_history: boolean;
  install_hint: string;
}

export interface ChatMessageRow {
  id: number;
  role: "user" | "assistant";
  content: string;
  provider: string | null;
  model: string | null;
  ts: string;
}

export interface SuggestedImport {
  path: string;
  name: string;
  size: number;
  mtime: number;
  source: string;
}

export interface AiUsage {
  month_runs: number;
  month_tokens_in: number;
  month_tokens_out: number;
}

export interface InboxPlanItem {
  path: string;
  name: string;
  project_id: number;
  project_name: string;
  bin_id: number | null;
  bin_name: string;
  reason: string;
}

export interface RenamePlanItem {
  file_id: number;
  old_name: string;
  new_name: string;
}

export interface CollectionRow {
  id: number;
  name: string;
  query: string;
  icon: string | null;
}

export interface BackupStatus {
  backup_dir: string | null;
  keep: number;
  last_backup: string | null;
  backups: [string, number][];
}

export interface TimelineRow {
  id: number;
  project_id: number;
  project_name: string;
  project_emoji: string;
  ts: string;
  kind: string;
  body_md: string;
}

export interface SpaceProject {
  id: number;
  name: string;
  emoji: string;
  color: string;
  size_bytes: number;
  file_count: number;
  bins: [string, number][];
  days_since_touch: number | null;
  empty_bins: string[];
}

export interface BigFile {
  id: number;
  project_name: string;
  name: string;
  rel_path: string;
  size: number;
  abs_path: string;
}

export interface SpaceReport {
  projects: SpaceProject[];
  largest: BigFile[];
  loose_root_files: number;
  disk_free_bytes: number;
  total_bytes: number;
}

export interface DupeGroup {
  hash: string;
  size: number;
  files: BigFile[];
}

export interface ArchiveEntry {
  name: string;
  path: string;
  size: number;
  created_ms: number;
}

export interface SnapshotRow {
  id: number;
  bin_id: number | null;
  bin_name: string | null;
  label: string;
  zip_path: string;
  file_count: number;
  created_at: string;
}

export interface SnapshotDiff {
  added: string[];
  removed: string[];
  changed: string[];
}

export interface JlcValidation {
  present: string[];
  missing: string[];
  zip_path: string | null;
}

export interface ComponentRow {
  id: number;
  mpn: string;
  lcsc: string | null;
  description: string | null;
  package: string | null;
  value: string | null;
  used_in: string[];
}
