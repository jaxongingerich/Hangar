/** Monochrome line icons — quiet, direct, inherit currentColor. */

type Glyph = string[];

const GLYPHS: Record<string, Glyph> = {
  file: [
    "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z",
    "M14 2v6h6",
  ],
  code: ["m16 18 6-6-6-6", "m8 6-6 6 6 6"],
  chip: [
    "M6 6h12v12H6z",
    "M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4",
  ],
  board: ["M4 4h16v16H4z", "M9 9h2v2H9zM14 13h2v2h-2z", "M4 12h5M15 8h5M12 20v-5"],
  cube: [
    "M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z",
    "m3.3 7 8.7 5 8.7-5",
    "M12 22V12",
  ],
  image: [
    "M3 5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
    "M8.5 10a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z",
    "m21 15-5-5L5 21",
  ],
  video: ["M2 6a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2z", "m22 8-4 3 4 3z"],
  archive: ["M21 8v13H3V8", "M1 3h22v5H1z", "M10 12h4"],
  sheet: ["M3 5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z", "M3 9h18M3 15h18M9 3v18"],
  text: [
    "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z",
    "M14 2v6h6",
    "M8 13h8M8 17h5",
  ],
  folder: [
    "M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z",
  ],
  book: [
    "M4 19.5A2.5 2.5 0 0 1 6.5 17H20",
    "M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z",
  ],
  camera: [
    "M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z",
    "M12 17a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z",
  ],
  factory: ["M2 20h20", "M4 20V8l6 4V8l6 4V4h4v16"],
  export: ["M12 3v12", "m7 8 5-5 5 5", "M5 21h14"],
  flask: ["M9 3h6", "M10 3v6L4.5 19a1.5 1.5 0 0 0 1.4 2h12.2a1.5 1.5 0 0 0 1.4-2L14 9V3"],
  palette: [
    "M12 22a10 10 0 1 1 10-10c0 1.66-1.34 3-3 3h-2a2 2 0 0 0-2 2c0 .5.2 1 .5 1.3.3.4.5.8.5 1.2a2.5 2.5 0 0 1-2.5 2.5Z",
    "M7.5 11a1 1 0 1 0 0-2 1 1 0 0 0 0 2ZM11 7.5a1 1 0 1 0 0-2 1 1 0 0 0 0 2ZM15.5 8a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z",
  ],
  sparkle: ["M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M18.4 5.6l-2.1 2.1M7.7 16.3l-2.1 2.1"],
  inbox: ["M3 13h4l2 3h6l2-3h4", "M5 6h14l2 7v6a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-6l2-7Z"],
};

const EXT_GLYPH: [string[], string][] = [
  [["gbr", "gtl", "gbl", "gto", "gbo", "gts", "gbs", "gko", "gm1", "drl", "xln"], "board"],
  [["step", "stp", "stl", "f3d", "dxf", "3mf", "scad"], "cube"],
  [["c", "cpp", "h", "hpp", "ino", "rs", "py", "ts", "tsx", "js", "jsx", "json", "sh"], "code"],
  [["bin", "elf", "hex", "uf2"], "chip"],
  [["sch", "kicad_sch", "kicad_pcb", "brd"], "chip"],
  [["pdf"], "book"],
  [["csv", "xlsx", "xls", "numbers"], "sheet"],
  [["png", "jpg", "jpeg", "heic", "webp", "svg", "gif"], "image"],
  [["mp4", "mov", "avi", "mkv"], "video"],
  [["zip", "gz", "tar", "7z", "rar"], "archive"],
  [["md", "txt", "doc", "docx", "pages"], "text"],
];

export function Icon({
  glyph,
  size = 14,
  className,
}: {
  glyph: string;
  size?: number;
  className?: string;
}) {
  const paths = GLYPHS[glyph] ?? GLYPHS.file;
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden
    >
      {paths.map((d, i) => (
        <path key={i} d={d} />
      ))}
    </svg>
  );
}

export function fileGlyph(ext: string | null): string {
  if (!ext) return "file";
  const e = ext.toLowerCase();
  for (const [exts, glyph] of EXT_GLYPH) {
    if (exts.includes(e)) return glyph;
  }
  return "file";
}

export function binGlyph(name: string): string {
  const n = name.toLowerCase();
  if (n.includes("gerber")) return "board";
  if (n.includes("jlc")) return "factory";
  if (n.includes("firmware")) return "chip";
  if (n.includes("cad")) return "cube";
  if (n.includes("datasheet")) return "book";
  if (n.includes("bom")) return "sheet";
  if (n.includes("photo") || n.includes("image")) return "camera";
  if (n.includes("design")) return "palette";
  if (n.includes("asset")) return "archive";
  if (n.includes("research")) return "flask";
  if (n.includes("export")) return "export";
  if (n.includes("ai") || n.includes("chat")) return "sparkle";
  if (n.includes("doc")) return "text";
  return "folder";
}

export function FileTypeIcon({ ext, size = 14 }: { ext: string | null; size?: number }) {
  return <Icon glyph={fileGlyph(ext)} size={size} />;
}

export function BinTypeIcon({ name, size = 14 }: { name: string; size?: number }) {
  return <Icon glyph={binGlyph(name)} size={size} />;
}
