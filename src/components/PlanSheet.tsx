import { useState } from "react";
import { AnimatePresence, motion } from "framer-motion";

export interface PlanRow {
  key: string;
  left: string;
  right: string;
  detail?: string;
}

/** Review sheet for AI-proposed operations: nothing runs until approved. */
export function PlanSheet({
  title,
  rows,
  onApprove,
  onCancel,
  busy,
}: {
  title: string;
  rows: PlanRow[];
  onApprove: (keys: string[]) => void;
  onCancel: () => void;
  busy?: boolean;
}) {
  const [checked, setChecked] = useState<Set<string>>(new Set(rows.map((r) => r.key)));

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.15 }}
        className="absolute inset-0 z-50 flex items-start justify-center bg-black/50 pt-[12vh]"
        onClick={onCancel}
      >
        <motion.div
          initial={{ opacity: 0, y: -8, scale: 0.98 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          transition={{ duration: 0.15, ease: "easeOut" }}
          className="flex max-h-[70vh] w-[560px] flex-col rounded-panel border border-line bg-panel shadow-2xl"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="flex items-center gap-2 border-b border-line px-5 py-3.5">
            <span className="text-[14px] font-semibold">{title}</span>
            <span className="font-mono text-[11px] text-muted">
              {checked.size}/{rows.length} selected
            </span>
            <button
              onClick={() =>
                setChecked(
                  checked.size === rows.length
                    ? new Set()
                    : new Set(rows.map((r) => r.key)),
                )
              }
              className="ml-auto text-[11px] text-muted hover:text-text"
            >
              {checked.size === rows.length ? "None" : "All"}
            </button>
          </div>
          <div className="flex-1 overflow-y-auto p-2">
            {rows.map((r) => (
              <label
                key={r.key}
                className="flex cursor-pointer items-center gap-2.5 rounded-md px-2.5 py-2 hover:bg-panel-2"
              >
                <input
                  type="checkbox"
                  checked={checked.has(r.key)}
                  onChange={() =>
                    setChecked((prev) => {
                      const next = new Set(prev);
                      if (next.has(r.key)) next.delete(r.key);
                      else next.add(r.key);
                      return next;
                    })
                  }
                  className="accent-(--color-solder)"
                />
                <span className="min-w-0 flex-1 truncate text-[12.5px]">{r.left}</span>
                <span className="text-muted">→</span>
                <span className="max-w-[200px] truncate text-[12.5px] text-solder">
                  {r.right}
                </span>
                {r.detail && (
                  <span
                    className="max-w-[120px] truncate text-[10px] text-muted"
                    title={r.detail}
                  >
                    {r.detail}
                  </span>
                )}
              </label>
            ))}
          </div>
          <div className="flex justify-end gap-2 border-t border-line px-5 py-3">
            <button
              onClick={onCancel}
              className="rounded-lg px-3.5 py-2 text-[13px] text-muted hover:bg-panel-2"
            >
              Cancel
            </button>
            <button
              onClick={() => onApprove([...checked])}
              disabled={checked.size === 0 || busy}
              className="rounded-lg bg-solder px-3.5 py-2 text-[13px] font-semibold text-ink disabled:opacity-40"
            >
              {busy ? "Applying…" : `Apply ${checked.size}`}
            </button>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}
