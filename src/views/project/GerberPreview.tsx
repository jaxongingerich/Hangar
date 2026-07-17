import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { api } from "../../lib/api";

/** Renders the Gerbers bin to top/bottom board previews via pcb-stackup. */
export function GerberPreview({
  binId,
  onClose,
}: {
  binId: number;
  onClose: () => void;
}) {
  const [svgs, setSvgs] = useState<{ top: string; bottom: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [side, setSide] = useState<"top" | "bottom">("top");

  useEffect(() => {
    (async () => {
      try {
        const files = await api.readBinGerbers(binId);
        if (files.length === 0) {
          setError("No gerber files in this bin.");
          return;
        }
        const { default: pcbStackup } = await import("pcb-stackup");
        const stackup = await pcbStackup(
          files.map((f) => ({ filename: f.filename, gerber: f.content })),
          { color: { fr4: "#1A2130", cu: "#8A97AC", cf: "#22D3A6", sm: "#0B0E14cc", ss: "#E6EBF2", sp: "#666666", out: "#232C3B" } },
        );
        setSvgs({ top: stackup.top.svg as string, bottom: stackup.bottom.svg as string });
      } catch (e) {
        setError(`Couldn't render board: ${e}`);
      }
    })();
  }, [binId]);

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.15 }}
        className="absolute inset-0 z-50 flex items-center justify-center bg-black/60"
        onClick={onClose}
      >
        <motion.div
          initial={{ opacity: 0, scale: 0.98 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ duration: 0.15, ease: "easeOut" }}
          className="flex max-h-[80vh] w-[640px] flex-col rounded-panel border border-line bg-panel shadow-2xl"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="flex items-center gap-2 border-b border-line px-5 py-3.5">
            <span className="text-[14px] font-semibold">Board preview</span>
            <div className="ml-2 flex rounded-lg border border-line p-0.5">
              {(["top", "bottom"] as const).map((s) => (
                <button
                  key={s}
                  onClick={() => setSide(s)}
                  className={`rounded-md px-2.5 py-1 text-[11px] font-medium ${
                    side === s ? "bg-panel-2 text-text" : "text-muted hover:text-text"
                  }`}
                >
                  {s === "top" ? "Top" : "Bottom"}
                </button>
              ))}
            </div>
            <button onClick={onClose} className="ml-auto text-[12px] text-muted hover:text-text">
              ✕
            </button>
          </div>
          <div className="flex flex-1 items-center justify-center overflow-auto p-6">
            {error ? (
              <p className="max-w-[400px] text-center text-[12px] text-muted">{error}</p>
            ) : !svgs ? (
              <p className="font-mono text-[12px] text-muted">rendering…</p>
            ) : (
              <div
                className="w-full [&_svg]:h-auto [&_svg]:w-full"
                dangerouslySetInnerHTML={{ __html: side === "top" ? svgs.top : svgs.bottom }}
              />
            )}
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}
