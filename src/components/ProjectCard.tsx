import { motion } from "framer-motion";
import { ProjectCard as Card } from "../lib/api";
import { formatAgo, formatBytes, STATUS_COLORS } from "../lib/format";
import { ProgressRing } from "./ProgressRing";
import { Spine } from "./Spine";
import { StatusChip } from "./StatusChip";

export function ProjectCard({
  project,
  onClick,
}: {
  project: Card;
  onClick?: () => void;
}) {
  const ringColor = STATUS_COLORS[project.status] ?? "var(--color-solder)";
  return (
    <motion.button
      layout
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.15, ease: "easeOut" }}
      onClick={onClick}
      className="group flex w-full items-stretch gap-3 rounded-panel border border-line bg-panel p-4 text-left transition-colors hover:border-line-strong"
    >
      <div className="flex min-w-0 flex-1 flex-col gap-3">
        <div className="flex items-center gap-2.5">
          <span
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-[16px]"
            style={{ background: `color-mix(in srgb, ${project.color} 14%, transparent)` }}
          >
            {project.emoji}
          </span>
          <span className="truncate text-[14px] font-semibold">
            {project.name}
          </span>
          {project.pinned && (
            <span className="text-[10px] text-solder" title="Pinned">
              ●
            </span>
          )}
        </div>

        <div className="flex items-center gap-3.5">
          <ProgressRing value={project.progress} color={ringColor} />
          <div className="flex min-w-0 flex-col gap-1">
            <StatusChip status={project.status} />
            <div className="flex gap-3 font-mono text-[11px] text-muted">
              <span>{project.file_count} files</span>
              <span>{formatBytes(project.size_bytes)}</span>
            </div>
            <span className="font-mono text-[11px] text-muted">
              {formatAgo(project.last_touch_ms)}
            </span>
          </div>
        </div>
      </div>
      <Spine data={project.spine} color={project.color} />
    </motion.button>
  );
}
