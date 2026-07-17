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

export const STATUS_COLORS: Record<string, string> = {
  idea: "#7C8AA5",
  active: "#22D3A6",
  paused: "#8A97AC",
  shipped: "#8B5CF6",
  archived: "#5A657A",
};

export const STATUS_LABELS: Record<string, string> = {
  idea: "Idea",
  active: "Active",
  paused: "Paused",
  shipped: "Shipped",
  archived: "Archived",
};
