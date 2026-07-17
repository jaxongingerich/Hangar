import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { AnimatePresence, motion } from "framer-motion";
import { api } from "../lib/api";
import { useUi } from "../lib/store";

const TEMPLATES = [
  { id: "hardware", label: "Hardware", desc: "Gerbers · JLCPCB · Firmware · CAD · BOM…" },
  { id: "software", label: "Software", desc: "Design · Assets · Research · Exports · Docs" },
  { id: "mixed", label: "Mixed", desc: "Everything — hardware + software bins" },
];

export function NewProjectModal() {
  const { newProjectOpen, setNewProjectOpen } = useUi();
  const [name, setName] = useState("");
  const [template, setTemplate] = useState("hardware");
  const [error, setError] = useState<string | null>(null);
  const qc = useQueryClient();

  const create = useMutation({
    mutationFn: () => api.createProject(name.trim(), template),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["projects"] });
      setName("");
      setError(null);
      setNewProjectOpen(false);
    },
    onError: (e) => setError(String(e)),
  });

  return (
    <AnimatePresence>
      {newProjectOpen && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.15 }}
          className="absolute inset-0 z-50 flex items-start justify-center bg-black/50 pt-[18vh]"
          onClick={() => setNewProjectOpen(false)}
        >
          <motion.div
            initial={{ opacity: 0, y: -8, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -8, scale: 0.98 }}
            transition={{ duration: 0.15, ease: "easeOut" }}
            className="w-[440px] rounded-panel border border-line bg-panel p-5 shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="mb-4 text-[16px] font-semibold">New project</div>
            <input
              autoFocus
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && name.trim()) create.mutate();
                if (e.key === "Escape") setNewProjectOpen(false);
              }}
              placeholder="Project name"
              className="mb-3 w-full rounded-lg border border-line bg-panel-2 px-3 py-2.5 text-[14px] placeholder:text-muted focus:border-solder focus:outline-none"
            />
            <div className="mb-4 flex flex-col gap-1.5">
              {TEMPLATES.map((t) => (
                <button
                  key={t.id}
                  onClick={() => setTemplate(t.id)}
                  className={`flex items-baseline gap-2 rounded-lg border px-3 py-2 text-left transition-colors ${
                    template === t.id
                      ? "border-solder bg-solder/5"
                      : "border-line hover:border-[#2E3A4E]"
                  }`}
                >
                  <span className="text-[13px] font-medium">{t.label}</span>
                  <span className="truncate text-[11px] text-muted">{t.desc}</span>
                </button>
              ))}
            </div>
            {error && <p className="mb-3 text-[12px] text-st-late">{error}</p>}
            <div className="flex justify-end gap-2">
              <button
                onClick={() => setNewProjectOpen(false)}
                className="rounded-lg px-3.5 py-2 text-[13px] text-muted hover:bg-panel-2"
              >
                Cancel
              </button>
              <button
                onClick={() => create.mutate()}
                disabled={!name.trim() || create.isPending}
                className="rounded-lg bg-solder px-3.5 py-2 text-[13px] font-semibold text-ink transition-opacity hover:opacity-90 disabled:opacity-40"
              >
                {create.isPending ? "Creating…" : "Create project"}
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
