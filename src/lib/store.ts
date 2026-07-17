import { create } from "zustand";

export type View =
  | "dashboard"
  | "today"
  | "inbox"
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
