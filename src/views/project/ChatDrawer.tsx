import { useRef, useState } from "react";
import { api } from "../../lib/api";

interface Msg {
  role: "user" | "assistant";
  content: string;
}

/** Right-panel chat scoped to one project — context is built by the backend
 *  from the file tree, logs, milestones and tasks. */
export function ChatDrawer({
  projectId,
  onClose,
}: {
  projectId: number;
  onClose: () => void;
}) {
  const [messages, setMessages] = useState<Msg[]>([]);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const scroller = useRef<HTMLDivElement>(null);

  const send = async () => {
    const content = draft.trim();
    if (!content || busy) return;
    const next: Msg[] = [...messages, { role: "user", content }];
    setMessages(next);
    setDraft("");
    setBusy(true);
    try {
      const reply = await api.aiProjectChat(projectId, next);
      setMessages([...next, { role: "assistant", content: reply }]);
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
    <aside className="flex w-80 shrink-0 flex-col border-l border-line">
      <div className="flex items-center gap-2 border-b border-line px-3 py-2.5">
        <span className="text-[13px] font-semibold">✳️ Project chat</span>
        <button
          onClick={() => setMessages([])}
          className="ml-auto text-[11px] text-muted hover:text-text"
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
          <p className="mt-6 text-center text-[12px] leading-relaxed text-muted">
            Ask about this project — the model sees its files, milestones,
            tasks and log.
          </p>
        )}
        <div className="flex flex-col gap-2.5">
          {messages.map((m, i) => (
            <div
              key={i}
              className={`max-w-[90%] whitespace-pre-wrap rounded-lg px-3 py-2 text-[12.5px] leading-relaxed select-text ${
                m.role === "user"
                  ? "self-end bg-solder/15 text-text"
                  : "self-start bg-panel-2"
              }`}
            >
              {m.content}
            </div>
          ))}
          {busy && (
            <div className="self-start rounded-lg bg-panel-2 px-3 py-2 font-mono text-[12px] text-muted">
              thinking…
            </div>
          )}
        </div>
      </div>
      <div className="border-t border-line p-2.5">
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
          placeholder="Ask… (↩ to send)"
          className="w-full resize-none rounded-lg border border-line bg-panel-2 px-3 py-2 text-[12.5px] placeholder:text-muted focus:border-solder focus:outline-none"
        />
      </div>
    </aside>
  );
}
