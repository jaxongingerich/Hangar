import { AnimatePresence, motion } from "framer-motion";
import { useToasts, useUi } from "../lib/store";

export function AiResultModal() {
  const { aiResult, setAiResult } = useUi();
  const { push } = useToasts();

  return (
    <AnimatePresence>
      {aiResult && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.15 }}
          className="absolute inset-0 z-50 flex items-start justify-center bg-black/50 pt-[12vh]"
          onClick={() => setAiResult(null)}
        >
          <motion.div
            initial={{ opacity: 0, y: -8, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            transition={{ duration: 0.15, ease: "easeOut" }}
            className="flex max-h-[70vh] w-[560px] flex-col rounded-panel border border-line bg-panel shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center gap-2 border-b border-line px-5 py-3.5">
              <span className="text-[14px] font-semibold">{aiResult.title}</span>
              <button
                onClick={() => {
                  navigator.clipboard.writeText(aiResult.text);
                  push("Copied");
                }}
                className="ml-auto rounded-md border border-line px-2.5 py-1 text-[11px] text-muted hover:border-solder hover:text-solder"
              >
                Copy
              </button>
              <button
                onClick={() => setAiResult(null)}
                className="text-[12px] text-muted hover:text-text"
              >
                ✕
              </button>
            </div>
            <div className="flex-1 overflow-y-auto whitespace-pre-wrap px-5 py-4 text-[12.5px] leading-relaxed select-text">
              {aiResult.text}
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
