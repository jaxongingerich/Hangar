const EXT_ICONS: [string[], string][] = [
  [["gbr", "gtl", "gbl", "gto", "gbo", "gts", "gbs", "gko", "gm1", "drl", "xln"], "🟩"],
  [["step", "stp", "stl", "f3d", "dxf", "3mf", "scad"], "🧊"],
  [["c", "cpp", "h", "hpp", "ino", "rs", "py", "ts", "tsx", "js", "jsx"], "📟"],
  [["bin", "elf", "hex", "uf2"], "🔧"],
  [["pdf"], "📕"],
  [["csv", "xlsx", "xls", "numbers"], "📊"],
  [["png", "jpg", "jpeg", "gif", "heic", "webp", "svg"], "🖼️"],
  [["mp4", "mov", "avi", "mkv"], "🎬"],
  [["zip", "gz", "tar", "7z", "rar"], "🗜️"],
  [["md", "txt", "doc", "docx", "pages"], "📄"],
  [["sch", "kicad_sch", "kicad_pcb", "brd"], "⚡"],
];

export function fileIcon(ext: string | null): string {
  if (!ext) return "📄";
  const e = ext.toLowerCase();
  for (const [exts, icon] of EXT_ICONS) {
    if (exts.includes(e)) return icon;
  }
  return "📄";
}

export function binIcon(name: string): string {
  const n = name.toLowerCase();
  if (n.includes("gerber")) return "🟩";
  if (n.includes("jlc")) return "🏭";
  if (n.includes("firmware")) return "📟";
  if (n.includes("cad")) return "🧊";
  if (n.includes("datasheet")) return "📕";
  if (n.includes("bom")) return "📊";
  if (n.includes("photo") || n.includes("image")) return "🖼️";
  if (n.includes("doc")) return "📄";
  if (n.includes("design")) return "🎨";
  if (n.includes("asset")) return "🧩";
  if (n.includes("research")) return "🔬";
  if (n.includes("export")) return "📤";
  if (n.includes("ai") || n.includes("chat")) return "✳️";
  return "📁";
}
