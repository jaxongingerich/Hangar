import { useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, FileRow, ProjectDetail } from "../../lib/api";
import { formatAgo, formatBytes } from "../../lib/format";
import { binIcon, fileIcon } from "../../lib/icons";
import { useToasts } from "../../lib/store";
import { SnapshotsPanel } from "./SnapshotsPanel";

type Scope =
  | { kind: "all" }
  | { kind: "root" }
  | { kind: "bin"; binId: number }
  | { kind: "snapshots" };

export function FilesTab({ project }: { project: ProjectDetail }) {
  const [scope, setScope] = useState<Scope>({ kind: "all" });
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [renaming, setRenaming] = useState<number | null>(null);
  const [newBin, setNewBin] = useState<string | null>(null);
  const qc = useQueryClient();
  const { push } = useToasts();

  const binId = scope.kind === "bin" ? scope.binId : null;
  const { data: files } = useQuery({
    queryKey: ["files", project.id, scope],
    queryFn: () =>
      api.listFiles(project.id, binId, scope.kind === "root" ? true : undefined),
    enabled: scope.kind !== "snapshots",
  });
  const { data: snapshots } = useQuery({
    queryKey: ["snapshots", project.id],
    queryFn: () => api.listSnapshots(project.id),
  });
  const { data: notedIds } = useQuery({
    queryKey: ["noted", project.id],
    queryFn: () => api.notedFileIds(project.id),
  });

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ["files", project.id] });
    qc.invalidateQueries({ queryKey: ["project", project.id] });
    qc.invalidateQueries({ queryKey: ["projects"] });
  };

  const moveTo = useMutation({
    mutationFn: ({ ids, dest }: { ids: number[]; dest: number | null }) =>
      api.moveFiles(ids, dest),
    onSuccess: (n) => {
      push(`Moved ${n} file${n === 1 ? "" : "s"}`);
      setSelected(new Set());
      invalidate();
    },
    onError: (e) => push(String(e), "error"),
  });

  const trash = useMutation({
    mutationFn: (ids: number[]) => api.trashFiles(ids),
    onSuccess: (n) => {
      push(`${n} file${n === 1 ? "" : "s"} → Trash`);
      setSelected(new Set());
      invalidate();
    },
    onError: (e) => push(String(e), "error"),
  });

  const rename = useMutation({
    mutationFn: ({ id, name }: { id: number; name: string }) =>
      api.renameFile(id, name),
    onSuccess: invalidate,
    onError: (e) => push(String(e), "error"),
  });

  const createBin = useMutation({
    mutationFn: (name: string) => api.createBin(project.id, name),
    onSuccess: () => {
      setNewBin(null);
      invalidate();
    },
    onError: (e) => push(String(e), "error"),
  });

  const rows = files ?? [];
  const selectedRows = rows.filter((f) => selected.has(f.id));

  const handleRowClick = (f: FileRow, e: React.MouseEvent) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (e.metaKey) {
        if (next.has(f.id)) next.delete(f.id);
        else next.add(f.id);
      } else {
        next.clear();
        next.add(f.id);
      }
      return next;
    });
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === " " && selectedRows.length === 1) {
      e.preventDefault();
      api.quickLook(selectedRows[0].abs_path);
    }
    if (e.key === "Backspace" && e.metaKey && selectedRows.length > 0) {
      trash.mutate(selectedRows.map((f) => f.id));
    }
    if (e.key === "Enter" && selectedRows.length === 1) {
      setRenaming(selectedRows[0].id);
    }
    // ⌘⇧V — paste clipboard (image or text) as a file into the current bin.
    if (e.metaKey && e.shiftKey && e.key.toLowerCase() === "v") {
      e.preventDefault();
      pasteClipboard();
    }
  };

  const pasteClipboard = async () => {
    const binIdForPaste = scope.kind === "bin" ? scope.binId : null;
    try {
      const items = await navigator.clipboard.read();
      for (const item of items) {
        if (item.types.includes("image/png")) {
          const blob = await item.getType("image/png");
          const buf = await blob.arrayBuffer();
          let binary = "";
          const bytes = new Uint8Array(buf);
          for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
          const name = await api.saveClipboardFile(
            project.id, binIdForPaste, "png", btoa(binary),
          );
          push(`Pasted → ${name}`);
          invalidate();
          return;
        }
      }
      const text = await navigator.clipboard.readText();
      if (text.trim()) {
        const name = await api.saveClipboardFile(
          project.id, binIdForPaste, "txt", undefined, text,
        );
        push(`Pasted → ${name}`);
        invalidate();
      } else {
        push("Clipboard is empty", "error");
      }
    } catch (err) {
      push(`Paste failed: ${err}`, "error");
    }
  };

  return (
    <div className="flex flex-1 overflow-hidden" onKeyDown={onKeyDown} tabIndex={-1}>
      {/* Bins rail */}
      <aside className="flex w-52 shrink-0 flex-col gap-0.5 overflow-y-auto border-r border-line p-2">
        <RailItem
          label="All files"
          icon="🗂️"
          count={project.bins.reduce((n, b) => n + b.file_count, 0) + project.root_file_count}
          active={scope.kind === "all"}
          onClick={() => setScope({ kind: "all" })}
        />
        <RailItem
          label="Loose files"
          icon="📥"
          count={project.root_file_count}
          active={scope.kind === "root"}
          onClick={() => setScope({ kind: "root" })}
          onDropIds={(ids) => moveTo.mutate({ ids, dest: null })}
        />
        <div className="mx-2 my-1.5 border-t border-line" />
        {project.bins.map((b) => (
          <RailItem
            key={b.id}
            label={b.name}
            icon={binIcon(b.name)}
            count={b.file_count}
            active={scope.kind === "bin" && scope.binId === b.id}
            onClick={() => setScope({ kind: "bin", binId: b.id })}
            onDropIds={(ids) => moveTo.mutate({ ids, dest: b.id })}
          />
        ))}
        <div className="mx-2 my-1.5 border-t border-line" />
        <RailItem
          label="Snapshots"
          icon="📸"
          count={snapshots?.length ?? 0}
          active={scope.kind === "snapshots"}
          onClick={() => setScope({ kind: "snapshots" })}
        />
        {newBin === null ? (
          <button
            onClick={() => setNewBin("")}
            className="mt-1 rounded-md px-2.5 py-1.5 text-left text-[12px] text-muted hover:bg-panel hover:text-text"
          >
            + New bin
          </button>
        ) : (
          <input
            autoFocus
            value={newBin}
            onChange={(e) => setNewBin(e.target.value)}
            onBlur={() => setNewBin(null)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && newBin.trim()) createBin.mutate(newBin.trim());
              if (e.key === "Escape") setNewBin(null);
            }}
            placeholder="Bin name"
            className="mt-1 rounded-md border border-solder bg-panel-2 px-2 py-1.5 text-[12px] focus:outline-none"
          />
        )}
      </aside>

      {/* File list */}
      <div className="flex flex-1 flex-col overflow-hidden">
        <div className="flex h-10 shrink-0 items-center gap-1 border-b border-line px-3">
          {selectedRows.length > 0 ? (
            <>
              <span className="mr-2 font-mono text-[11px] text-muted">
                {selectedRows.length} selected
              </span>
              <ToolbarBtn
                label="Quick Look"
                onClick={() => api.quickLook(selectedRows[0].abs_path)}
              />
              <ToolbarBtn
                label="Reveal"
                onClick={() => revealItemInDir(selectedRows[0].abs_path)}
              />
              <ToolbarBtn
                label="Rename"
                onClick={() => setRenaming(selectedRows[0].id)}
                disabled={selectedRows.length !== 1}
              />
              {selectedRows.length === 1 && (
                <>
                  <ToolbarBtn
                    label={notedIds?.includes(selectedRows[0].id) ? "Note ●" : "Note"}
                    onClick={async () => {
                      const existing = await api.getFileNote(selectedRows[0].id);
                      const body = prompt("Note for this file (markdown):", existing ?? "");
                      if (body === null) return;
                      await api.setFileNote(selectedRows[0].id, body);
                      qc.invalidateQueries({ queryKey: ["noted", project.id] });
                      push(body.trim() ? "Note saved" : "Note removed");
                    }}
                  />
                  <ToolbarBtn
                    label="Tags"
                    onClick={async () => {
                      const current = await api.getFinderTags(selectedRows[0].abs_path);
                      const input = prompt(
                        "Finder tags (comma separated) — synced with macOS:",
                        current.join(", "),
                      );
                      if (input === null) return;
                      const tags = input.split(",").map((t) => t.trim()).filter(Boolean);
                      await api.setFinderTags(selectedRows[0].abs_path, tags);
                      push(tags.length ? `Tagged: ${tags.join(", ")}` : "Tags cleared");
                    }}
                  />
                </>
              )}
              <MoveMenu
                bins={project.bins}
                onMove={(dest) =>
                  moveTo.mutate({ ids: selectedRows.map((f) => f.id), dest })
                }
              />
              <ToolbarBtn
                label="Pin"
                onClick={async () => {
                  await Promise.all(selectedRows.map((f) => api.togglePinFile(f.id)));
                  invalidate();
                }}
              />
              <ToolbarBtn
                label="Trash"
                danger
                onClick={() => trash.mutate(selectedRows.map((f) => f.id))}
              />
            </>
          ) : (
            <>
              <span className="font-mono text-[11px] text-muted">
                {rows.length} files · drag rows onto a bin to move them
              </span>
              {scope.kind === "bin" && (
                <BinActions
                  project={project}
                  binId={scope.binId}
                  onChange={() => {
                    invalidate();
                    qc.invalidateQueries({ queryKey: ["snapshots", project.id] });
                  }}
                />
              )}
            </>
          )}
          {selectedRows.length === 1 &&
            selectedRows[0].ext === "csv" && (
              <ToolbarBtn
                label="Normalize BOM → JLC"
                onClick={async () => {
                  try {
                    const out = await api.normalizeBom(selectedRows[0].id);
                    push(`Wrote ${out.split("/").pop()}`);
                    invalidate();
                  } catch (e) {
                    push(String(e), "error");
                  }
                }}
              />
            )}
        </div>
        {scope.kind === "snapshots" ? (
          <SnapshotsPanel project={project} snapshots={snapshots ?? []} />
        ) : (
        <FileList
          rows={rows}
          selected={selected}
          renaming={renaming}
          onRowClick={handleRowClick}
          onRename={(id, name) => {
            setRenaming(null);
            rename.mutate({ id, name });
          }}
          onCancelRename={() => setRenaming(null)}
          onQuickLook={(f) => api.quickLook(f.abs_path)}
        />
        )}
      </div>
    </div>
  );
}

function BinActions({
  project,
  binId,
  onChange,
}: {
  project: ProjectDetail;
  binId: number;
  onChange: () => void;
}) {
  const { push } = useToasts();
  const bin = project.bins.find((b) => b.id === binId);
  const isGerbers = bin?.name.toLowerCase().includes("gerber") ?? false;

  return (
    <div className="ml-auto flex items-center gap-1">
      <ToolbarBtn
        label="📸 Snapshot"
        onClick={async () => {
          const label = prompt("Snapshot label (e.g. Rev A):");
          if (!label?.trim()) return;
          try {
            await api.snapshotBin(binId, label.trim());
            push(`Snapshot "${label.trim()}" saved`);
            onChange();
          } catch (e) {
            push(String(e), "error");
          }
        }}
      />
      {isGerbers && (
        <ToolbarBtn
          label="Export JLC"
          onClick={async () => {
            try {
              const check = await api.exportJlcpcb(project.id, { binId, dryRun: true });
              const proceed =
                check.missing.length === 0 ||
                confirm(
                  `Missing layers:\n• ${check.missing.join("\n• ")}\n\nExport anyway?`,
                );
              if (!proceed) return;
              const result = await api.exportJlcpcb(project.id, { binId, dryRun: false });
              push(`JLC package → ${result.zip_path?.split("/").pop()}`);
              onChange();
            } catch (e) {
              push(String(e), "error");
            }
          }}
        />
      )}
    </div>
  );
}

function RailItem({
  label,
  icon,
  count,
  active,
  onClick,
  onDropIds,
}: {
  label: string;
  icon: string;
  count: number;
  active: boolean;
  onClick: () => void;
  onDropIds?: (ids: number[]) => void;
}) {
  const [over, setOver] = useState(false);
  return (
    <button
      onClick={onClick}
      onDragOver={(e) => {
        if (onDropIds) {
          e.preventDefault();
          setOver(true);
        }
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        setOver(false);
        if (!onDropIds) return;
        const raw = e.dataTransfer.getData("application/x-hangar-files");
        if (raw) onDropIds(JSON.parse(raw));
      }}
      className={`flex items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-[12px] transition-colors ${
        active ? "bg-panel-2 text-text" : "text-muted hover:bg-panel hover:text-text"
      } ${over ? "ring-1 ring-solder" : ""}`}
    >
      <span className="text-[13px]">{icon}</span>
      <span className="flex-1 truncate">{label}</span>
      <span className="font-mono text-[10px]">{count}</span>
    </button>
  );
}

function ToolbarBtn({
  label,
  onClick,
  disabled,
  danger,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  danger?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`rounded-md px-2 py-1 text-[12px] transition-colors disabled:opacity-40 ${
        danger ? "text-st-late hover:bg-st-late/10" : "text-muted hover:bg-panel hover:text-text"
      }`}
    >
      {label}
    </button>
  );
}

function MoveMenu({
  bins,
  onMove,
}: {
  bins: ProjectDetail["bins"];
  onMove: (dest: number | null) => void;
}) {
  return (
    <select
      value=""
      onChange={(e) => {
        const v = e.target.value;
        if (v === "") return;
        onMove(v === "root" ? null : Number(v));
      }}
      className="rounded-md border border-line bg-panel px-2 py-1 text-[12px] text-muted"
    >
      <option value="">Move to…</option>
      <option value="root">Loose files</option>
      {bins.map((b) => (
        <option key={b.id} value={b.id}>
          {b.name}
        </option>
      ))}
    </select>
  );
}

function FileList({
  rows,
  selected,
  renaming,
  onRowClick,
  onRename,
  onCancelRename,
  onQuickLook,
}: {
  rows: FileRow[];
  selected: Set<number>;
  renaming: number | null;
  onRowClick: (f: FileRow, e: React.MouseEvent) => void;
  onRename: (id: number, name: string) => void;
  onCancelRename: () => void;
  onQuickLook: (f: FileRow) => void;
}) {
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 34,
    overscan: 12,
  });

  const items = virtualizer.getVirtualItems();
  const empty = useMemo(() => rows.length === 0, [rows]);

  if (empty) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <p className="max-w-[300px] text-center text-muted">
          Nothing here yet. Drop files in from Finder — this bin is a real
          folder on disk.
        </p>
      </div>
    );
  }

  return (
    <div ref={parentRef} className="flex-1 overflow-y-auto">
      <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {items.map((vi) => {
          const f = rows[vi.index];
          const isSel = selected.has(f.id);
          return (
            <div
              key={f.id}
              draggable
              onDragStart={(e) => {
                const ids = isSel ? [...selected] : [f.id];
                e.dataTransfer.setData(
                  "application/x-hangar-files",
                  JSON.stringify(ids),
                );
              }}
              onClick={(e) => onRowClick(f, e)}
              onDoubleClick={() => onQuickLook(f)}
              className={`absolute left-0 right-0 flex cursor-default items-center gap-2.5 border-b border-line/50 px-3 ${
                isSel ? "bg-panel-2" : "hover:bg-panel"
              }`}
              style={{ top: vi.start, height: vi.size }}
            >
              <span className="w-5 text-center text-[13px]">{fileIcon(f.ext)}</span>
              {renaming === f.id ? (
                <RenameInput
                  initial={f.name}
                  onCommit={(name) => onRename(f.id, name)}
                  onCancel={onCancelRename}
                />
              ) : (
                <span className="flex-1 truncate text-[12px]">
                  {f.pinned && <span className="mr-1 text-solder">●</span>}
                  {f.name}
                </span>
              )}
              <span className="w-20 text-right font-mono text-[11px] text-muted">
                {formatBytes(f.size)}
              </span>
              <span className="w-20 text-right font-mono text-[11px] text-muted">
                {formatAgo(f.mtime)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function RenameInput({
  initial,
  onCommit,
  onCancel,
}: {
  initial: string;
  onCommit: (name: string) => void;
  onCancel: () => void;
}) {
  const [draft, setDraft] = useState(initial);
  return (
    <input
      autoFocus
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onClick={(e) => e.stopPropagation()}
      onBlur={onCancel}
      onKeyDown={(e) => {
        if (e.key === "Enter" && draft.trim()) onCommit(draft.trim());
        if (e.key === "Escape") onCancel();
      }}
      className="flex-1 rounded-md border border-solder bg-panel-2 px-2 py-0.5 text-[12px] focus:outline-none"
    />
  );
}
