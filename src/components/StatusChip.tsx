import { STATUS_COLORS, STATUS_LABELS } from "../lib/format";

export function StatusChip({ status }: { status: string }) {
  const color = STATUS_COLORS[status] ?? "var(--color-muted)";
  return (
    <span
      className="inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[11px] font-medium"
      style={{ color, background: `color-mix(in srgb, ${color} 12%, transparent)` }}
    >
      <span
        className="h-1.5 w-1.5 rounded-full"
        style={{ background: color }}
      />
      {STATUS_LABELS[status] ?? status}
    </span>
  );
}
