import { useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, ChatAction, FileRow } from "../../lib/api";

interface Msg {
  role: "user" | "assistant";
  content: string;
  /** Changes the AI made to the project on this turn, if any. */
  actions?: ChatAction[];
  /** Files the user attached to this message, by name. */
  attached?: string[];
}

/** Right-panel chat scoped to one project.
 *
 *  Context is built by the backend from the file tree, logs, milestones and
 *  tasks. Beyond answering, the AI can change the project — add tasks, move
 *  milestones, organise files — and every change it makes is listed under its
 *  reply so nothing happens invisibly. That can be switched off per chat.
 */
export function ChatDrawer({
  projectId,
  onClose,
}: {
  projectId: number;
  onClose: () => void;
}) {
  const qc = useQueryClient();
  const [messages, setMessages] = useState<Msg[]>([]);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [profileId, setProfileId] = useState<string>("");
  const [allowActions, setAllowActions] = useState(true);
  const [attached, setAttached] = useState<FileRow[]>([]);
  const [picking, setPicking] = useState(false);
  const scroller = useRef<HTMLDivElement>(null);

  const { data: profiles } = useQuery({
    queryKey: ["aiProfiles"],
    queryFn: api.aiListProfiles,
  });
  const { data: files } = useQuery({
    queryKey: ["files", projectId],
    queryFn: () => api.listFiles(projectId),
  });

  const send = async () => {
    const content = draft.trim();
    if (!content || busy) return;
    const outgoing = attached;
    const next: Msg[] = [
      ...messages,
      { role: "user", content, attached: outgoing.map((f) => f.name) },
    ];
    setMessages(next);
    setDraft("");
    setAttached([]);
    setBusy(true);
    try {
      const reply = await api.aiProjectChat(
        projectId,
        // Attachment names are UI-only; the backend reads the files itself.
        next.map((m) => ({ role: m.role, content: m.content })),
        {
          profileId: profileId || null,
          attachments: outgoing.map((f) => ({ file_id: f.id })),
          allowActions,
        },
      );
      setMessages([
        ...next,
        { role: "assistant", content: reply.text, actions: reply.actions },
      ]);
      // The AI may have added tasks, moved files or set progress — refresh
      // everything this project's views read.
      if (reply.actions.length > 0) {
        for (const key of ["project", "files", "tasks", "milestones", "logs", "stats"]) {
          qc.invalidateQueries({ queryKey: [key, projectId] });
        }
        qc.invalidateQueries({ queryKey: ["projects"] });
      }
    } catch (e) {
      setMessages([...next, { role: "assistant", content: `⚠️ ${e}` }]);
    } finally {
      setBusy(false);
      requestAnimationFrame(() => {
        scroller.current?.scrollTo({ top: scroller.current.scrollHeight });
      });
    }
  };

  return (
    <aside className="flex w-96 shrink-0 flex-col border-l border-line">
      <div className="flex items-center gap-2 border-b border-line px-3 py-2.5">
        <span className="text-[13px] font-semibold">Project chat</span>
        <select
          value={profileId}
          onChange={(e) => setProfileId(e.target.value)}
          title="Which AI answers in this chat"
          className="ml-auto max-w-[130px] rounded-md border border-line bg-panel-2 px-1.5 py-0.5 text-[11px] text-muted focus:border-solder focus:outline-none"
        >
          <option value="">Default AI</option>
          {(profiles ?? []).map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
        <button
          onClick={() => setMessages([])}
          className="text-[11px] text-muted hover:text-text"
          title="Clear conversation"
        >
          clear
        </button>
        <button
          onClick={onClose}
          className="text-[12px] text-muted hover:text-text"
          title="Close chat"
        >
          ✕
        </button>
      </div>

      <div ref={scroller} className="flex-1 overflow-y-auto p-3">
        {messages.length === 0 && (
          <p className="mt-6 px-2 text-center text-[12px] leading-relaxed text-muted">
            Ask about this project — the model sees its files, milestones, tasks
            and log.
            {allowActions && (
              <>
                {" "}
                It can change things too: “add a task to panelize the gerbers”,
                “move the datasheets into Docs”, “mark the layout milestone
                done”.
              </>
            )}
          </p>
        )}
        <div className="flex flex-col gap-2.5">
          {messages.map((m, i) => (
            <div key={i} className={m.role === "user" ? "self-end" : "self-start"}>
              <div
                className={`max-w-[300px] whitespace-pre-wrap rounded-lg px-3 py-2 text-[12.5px] leading-relaxed select-text ${
                  m.role === "user" ? "bg-solder/15 text-text" : "bg-panel-2"
                }`}
              >
                {m.content}
              </div>
              {m.attached && m.attached.length > 0 && (
                <div className="mt-1 text-right font-mono text-[10px] text-muted">
                  📎 {m.attached.join(", ")}
                </div>
              )}
              {m.actions && m.actions.length > 0 && (
                <div className="mt-1.5 rounded-lg border border-line bg-panel px-2.5 py-2">
                  <div className="mb-1 text-[10px] font-medium uppercase tracking-wide text-muted">
                    Changes made
                  </div>
                  {m.actions.map((a, j) => (
                    <div
                      key={j}
                      className="flex items-start gap-1.5 py-0.5 font-mono text-[10.5px]"
                      title={a.detail}
                    >
                      <span className={a.ok ? "text-solder" : "text-st-late"}>
                        {a.ok ? "✓" : "✕"}
                      </span>
                      <span className={a.ok ? "text-muted" : "text-st-late"}>
                        {a.label}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          ))}
          {busy && (
            <div className="self-start rounded-lg bg-panel-2 px-3 py-2 font-mono text-[12px] text-muted">
              thinking…
            </div>
          )}
        </div>
      </div>

      {picking && (
        <FilePicker
          files={files ?? []}
          attached={attached}
          onToggle={(f) =>
            setAttached((prev) =>
              prev.some((p) => p.id === f.id)
                ? prev.filter((p) => p.id !== f.id)
                : [...prev, f],
            )
          }
          onClose={() => setPicking(false)}
        />
      )}

      <div className="border-t border-line p-2.5">
        {attached.length > 0 && (
          <div className="mb-1.5 flex flex-wrap gap-1">
            {attached.map((f) => (
              <button
                key={f.id}
                onClick={() => setAttached((p) => p.filter((x) => x.id !== f.id))}
                className="rounded-md border border-line bg-panel-2 px-1.5 py-0.5 font-mono text-[10px] text-muted hover:border-st-late hover:text-st-late"
                title="Remove attachment"
              >
                📎 {f.name} ✕
              </button>
            ))}
          </div>
        )}
        <textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              send();
            }
          }}
          rows={2}
          placeholder="Ask, or tell it what to change… (↩ to send)"
          className="w-full resize-none rounded-lg border border-line bg-panel-2 px-3 py-2 text-[12.5px] placeholder:text-muted focus:border-solder focus:outline-none"
        />
        <div className="mt-1.5 flex items-center gap-2">
          <button
            onClick={() => setPicking(!picking)}
            className="rounded-md border border-line px-2 py-0.5 text-[11px] text-muted hover:border-solder hover:text-solder"
            title="Attach project files to this message"
          >
            📎 Attach
          </button>
          <label
            className="flex cursor-pointer items-center gap-1.5 text-[11px] text-muted"
            title="Let the AI add tasks, move milestones and organise files. Every change is listed under its reply."
          >
            <input
              type="checkbox"
              checked={allowActions}
              onChange={(e) => setAllowActions(e.target.checked)}
              className="accent-(--color-solder)"
            />
            Can make changes
          </label>
        </div>
      </div>
    </aside>
  );
}

/** Searchable list of the project's files, for attaching to a message. */
function FilePicker({
  files,
  attached,
  onToggle,
  onClose,
}: {
  files: FileRow[];
  attached: FileRow[];
  onToggle: (f: FileRow) => void;
  onClose: () => void;
}) {
  const [q, setQ] = useState("");
  const shown = useMemo(() => {
    const needle = q.trim().toLowerCase();
    const list = needle
      ? files.filter((f) => f.name.toLowerCase().includes(needle))
      : files;
    return list.slice(0, 60);
  }, [files, q]);

  return (
    <div className="max-h-64 overflow-y-auto border-t border-line bg-panel">
      <div className="sticky top-0 flex items-center gap-2 border-b border-line bg-panel px-2.5 py-1.5">
        <input
          autoFocus
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => e.key === "Escape" && onClose()}
          placeholder="Find a file…"
          className="flex-1 bg-transparent text-[12px] placeholder:text-muted focus:outline-none"
        />
        <button onClick={onClose} className="text-[11px] text-muted hover:text-text">
          done
        </button>
      </div>
      {shown.length === 0 ? (
        <p className="px-3 py-2 text-[11.5px] text-muted">
          {files.length === 0 ? "No files in this project yet." : "No match."}
        </p>
      ) : (
        shown.map((f) => (
          <label
            key={f.id}
            className="flex cursor-pointer items-center gap-2 px-2.5 py-1 hover:bg-panel-2"
          >
            <input
              type="checkbox"
              checked={attached.some((a) => a.id === f.id)}
              onChange={() => onToggle(f)}
              className="shrink-0"
            />
            <span className="min-w-0 flex-1 truncate text-[11.5px]">{f.name}</span>
            <span className="shrink-0 font-mono text-[10px] text-muted">
              {f.ext ?? ""}
            </span>
          </label>
        ))
      )}
    </div>
  );
}
