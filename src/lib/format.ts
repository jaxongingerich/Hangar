export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v >= 100 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

export function formatAgo(ms: number | null): string {
  if (!ms) return "—";
  const diff = Date.now() - ms;
  const min = Math.floor(diff / 60_000);
  if (min < 1) return "just now";
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const d = Math.floor(hr / 24);
  if (d < 30) return `${d}d ago`;
  const mo = Math.floor(d / 30);
  return `${mo}mo ago`;
}

/** Theme-aware status colors (CSS variables switch with light/dark). */
export const STATUS_COLORS: Record<string, string> = {
  idea: "var(--muted)",
  active: "var(--accent)",
  paused: "var(--muted)",
  shipped: "var(--ok)",
  archived: "var(--muted)",
};

export const STATUS_LABELS: Record<string, string> = {
  idea: "Idea",
  active: "Active",
  paused: "Paused",
  shipped: "Shipped",
  archived: "Archived",
};

export const HEALTH_COLORS: Record<string, string> = {
  on_track: "var(--ok)",
  at_risk: "var(--warn)",
  late: "var(--danger)",
};

export const HEALTH_LABELS: Record<string, string> = {
  on_track: "On track",
  at_risk: "At risk",
  late: "Late",
};

/** `color` at `pct`% opacity — works with CSS variables. */
export function tint(color: string, pct: number): string {
  return `color-mix(in srgb, ${color} ${pct}%, transparent)`;
}
