import { AnimatePresence, motion } from "framer-motion";
import { useToasts } from "../lib/store";

export function Toasts() {
  const { toasts, dismiss } = useToasts();
  return (
    <div className="pointer-events-none absolute bottom-4 left-1/2 z-[60] flex -translate-x-1/2 flex-col items-center gap-2">
      <AnimatePresence>
        {toasts.map((t) => (
          <motion.button
            key={t.id}
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 8 }}
            transition={{ duration: 0.15, ease: "easeOut" }}
            onClick={() => dismiss(t.id)}
            className={`pointer-events-auto rounded-lg border px-4 py-2 text-[12px] shadow-lg ${
              t.kind === "error"
                ? "border-st-late/40 bg-panel text-st-late"
                : "border-line bg-panel text-text"
            }`}
          >
            {t.message}
          </motion.button>
        ))}
      </AnimatePresence>
    </div>
  );
}
