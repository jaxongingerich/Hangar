import { create } from "zustand";

export type Theme = "system" | "light" | "dark";

interface ThemeState {
  theme: Theme;
  setTheme: (t: Theme) => void;
}

export const useTheme = create<ThemeState>((set) => ({
  theme: (localStorage.getItem("hangar-theme") as Theme) || "system",
  setTheme: (theme) => {
    localStorage.setItem("hangar-theme", theme);
    set({ theme });
    applyTheme(theme);
  },
}));

export function applyTheme(theme: Theme) {
  const dark =
    theme === "dark" ||
    (theme === "system" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.classList.toggle("dark", dark);
}

// Apply on load and track OS changes while in system mode.
applyTheme(useTheme.getState().theme);
window
  .matchMedia("(prefers-color-scheme: dark)")
  .addEventListener("change", () => applyTheme(useTheme.getState().theme));

export type View =
  | "dashboard"
  | "today"
  | "inbox"
  | "assistant"
  | "progress"
  | "space"
  | "settings"
  | "project";

export type SortMode = "recent" | "progress" | "size" | "deadline" | "name";

interface UiState {
  view: View;
  setView: (v: View) => void;
  projectId: number | null;
  openProject: (id: number) => void;
  sort: SortMode;
  setSort: (s: SortMode) => void;
  newProjectOpen: boolean;
  setNewProjectOpen: (open: boolean) => void;
  paletteOpen: boolean;
  setPaletteOpen: (open: boolean) => void;
  aiResult: { title: string; text: string } | null;
  setAiResult: (r: { title: string; text: string } | null) => void;
  /** Bin currently open in the project Files tab — drop target for imports. */
  activeBinId: number | null;
  setActiveBinId: (id: number | null) => void;
}

export const useUi = create<UiState>((set) => ({
  view: "dashboard",
  setView: (view) => set({ view }),
  projectId: null,
  openProject: (id) => set({ view: "project", projectId: id }),
  sort: "recent",
  setSort: (sort) => set({ sort }),
  newProjectOpen: false,
  setNewProjectOpen: (newProjectOpen) => set({ newProjectOpen }),
  paletteOpen: false,
  setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
  aiResult: null,
  setAiResult: (aiResult) => set({ aiResult }),
  activeBinId: null,
  setActiveBinId: (activeBinId) => set({ activeBinId }),
}));

// Lightweight toast bus.
export interface Toast {
  id: number;
  message: string;
  kind: "info" | "error";
}

interface ToastState {
  toasts: Toast[];
  push: (message: string, kind?: Toast["kind"]) => void;
  dismiss: (id: number) => void;
}

let toastId = 0;

export const useToasts = create<ToastState>((set) => ({
  toasts: [],
  push: (message, kind = "info") => {
    const id = ++toastId;
    set((s) => ({ toasts: [...s.toasts, { id, message, kind }] }));
    setTimeout(
      () => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
      4000,
    );
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));
