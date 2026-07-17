import { create } from "zustand";

export type View =
  | "dashboard"
  | "today"
  | "inbox"
  | "progress"
  | "space"
  | "settings";

export type SortMode = "recent" | "progress" | "size" | "deadline" | "name";

interface UiState {
  view: View;
  setView: (v: View) => void;
  sort: SortMode;
  setSort: (s: SortMode) => void;
  newProjectOpen: boolean;
  setNewProjectOpen: (open: boolean) => void;
}

export const useUi = create<UiState>((set) => ({
  view: "dashboard",
  setView: (view) => set({ view }),
  sort: "recent",
  setSort: (sort) => set({ sort }),
  newProjectOpen: false,
  setNewProjectOpen: (newProjectOpen) => set({ newProjectOpen }),
}));
