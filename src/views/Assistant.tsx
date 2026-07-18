import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, AiProfile, ChatMessageRow, DetectedProvider } from "../lib/api";
import { useToasts } from "../lib/store";
import { ImportPanel, sourceLabel } from "./ai/ImportPanel";

/** Presets so adding a provider is one pick + (usually) one key. */
const PRESETS: {
  label: string;
  provider: string;
  model: string;
  base_url: string;
  command?: string;
}[] = [
  { label: "Claude (Anthropic API)", provider: "anthropic", model: "claude-sonnet-4-6", base_url: "" },
  { label: "OpenAI / Codex", provider: "openai", model: "gpt-5.1", base_url: "https://api.openai.com/v1" },
  { label: "Hermes (Nous Research)", provider: "openai", model: "Hermes-4-405B", base_url: "https://inference-api.nousresearch.com/v1" },
  { label: "Google Gemini", provider: "openai", model: "gemini-2.5-pro", base_url: "https://generativelanguage.googleapis.com/v1beta/openai" },
  { label: "xAI Grok", provider: "openai", model: "grok-4", base_url: "https://api.x.ai/v1" },
  { label: "Mistral", provider: "openai", model: "mistral-large-latest", base_url: "https://api.mistral.ai/v1" },
  { label: "DeepSeek", provider: "openai", model: "deepseek-chat", base_url: "https://api.deepseek.com/v1" },
  { label: "Perplexity", provider: "openai", model: "sonar-pro", base_url: "https://api.perplexity.ai" },
  { label: "OpenRouter (almost any model)", provider: "openai", model: "", base_url: "https://openrouter.ai/api/v1" },
  { label: "Groq", provider: "openai", model: "llama-3.3-70b-versatile", base_url: "https://api.groq.com/openai/v1" },
  { label: "Together AI", provider: "openai", model: "", base_url: "https://api.together.xyz/v1" },
  { label: "Fireworks AI", provider: "openai", model: "", base_url: "https://api.fireworks.ai/inference/v1" },
  { label: "Cerebras", provider: "openai", model: "", base_url: "https://api.cerebras.ai/v1" },
  { label: "Ollama (local)", provider: "ollama", model: "", base_url: "http://localhost:11434" },
  { label: "LM Studio (local)", provider: "openai", model: "", base_url: "http://localhost:1234/v1" },
  { label: "Custom CLI (advanced, no key)", provider: "cli", model: "", base_url: "", command: "" },
  { label: "Custom endpoint", provider: "openai", model: "", base_url: "" },
];

const inputCls =
  "rounded-md border border-line bg-panel-2 px-2.5 py-1.5 text-[12px] placeholder:text-muted focus:border-solder focus:outline-none";
const ghostBtn =
  "rounded-md border border-line px-2.5 py-1 text-[11.5px] font-medium text-muted transition-colors hover:border-solder hover:text-solder";
const primaryBtn =
  "rounded-md bg-solder px-3 py-1.5 text-[12px] font-semibold text-on-accent transition-opacity hover:opacity-90 disabled:opacity-40";

export function Assistant() {
  const qc = useQueryClient();
  const [chatId, setChatId] = useState<number | null>(null);
  const [profileId, setProfileId] = useState<string | null>(null);
  const [managing, setManaging] = useState(false);
  const [importing, setImporting] = useState(false);

  const { data: profiles } = useQuery({
    queryKey: ["aiProfiles"],
    queryFn: api.aiListProfiles,
  });
  const { data: chats } = useQuery({ queryKey: ["aiChats"], queryFn: api.aiListChats });
  const { data: projects } = useQuery({
    queryKey: ["projects"],
    queryFn: api.listProjects,
  });

  // Default selections: active profile, most recent chat.
  useEffect(() => {
    if (profileId === null && profiles?.length) {
      setProfileId((profiles.find((p) => p.active) ?? profiles[0]).id);
    }
  }, [profiles, profileId]);
  useEffect(() => {
    if (chatId === null && chats?.length) setChatId(chats[0].id);
  }, [chats, chatId]);

  const newChat = async () => {
    const id = await api.aiNewChat(null);
    await qc.invalidateQueries({ queryKey: ["aiChats"] });
    setChatId(id);
  };

  const chat = chats?.find((c) => c.id === chatId) ?? null;
  const noProfiles = (profiles ?? []).length === 0;

  // Group the sidebar by where each chat came from, so imported Claude Code and
  // Codex conversations sit under their own headings instead of being mixed in
  // with chats started here. Chats started in Hangar always come first.
  const groupedChats = (() => {
    const m = new Map<string, typeof chats>();
    for (const c of chats ?? []) {
      const src = c.source ?? "hangar";
      m.set(src, [...(m.get(src) ?? []), c] as typeof chats);
    }
    return [...m.entries()].sort(([a], [b]) =>
      a === "hangar" ? -1 : b === "hangar" ? 1 : a.localeCompare(b),
    );
  })();

  return (
    <div className="flex flex-1 overflow-hidden">
      {/* Chat list */}
      <aside className="flex w-[230px] shrink-0 flex-col border-r border-line">
        <div className="flex items-center gap-2 px-3 pb-2 pt-5">
          <h1 className="text-[15px] font-semibold">AI</h1>
          <button onClick={newChat} className={`ml-auto ${ghostBtn}`}>
            New chat
          </button>
        </div>
        <div className="flex-1 overflow-y-auto px-2 pb-3">
          {(chats ?? []).length === 0 ? (
            <p className="px-2 py-3 text-[12px] leading-relaxed text-muted">
              No chats yet. Start one — every conversation is saved here.
            </p>
          ) : (
            groupedChats.map(([source, list]) => (
              <div key={source} className="mb-2">
                {/* Only label groups once there's more than one source in play. */}
                {groupedChats.length > 1 && (
                  <div className="mb-1 px-2.5 pt-1 text-[10px] font-semibold uppercase tracking-wide text-muted">
                    {sourceLabel(source)}
                  </div>
                )}
                {(list ?? []).map((c) => (
                  <button
                    key={c.id}
                    onClick={() => {
                      setChatId(c.id);
                      setManaging(false);
                      setImporting(false);
                    }}
                    className={`group mb-0.5 block w-full rounded-md px-2.5 py-2 text-left transition-colors ${
                      c.id === chatId ? "bg-panel-2" : "hover:bg-panel"
                    }`}
                  >
                    <div className="truncate text-[12.5px] font-medium">
                      {c.title}
                    </div>
                    <div className="mt-0.5 flex items-center gap-2 text-[10.5px] text-muted">
                      {c.project_name && (
                        <span className="truncate">{c.project_name}</span>
                      )}
                      <span className="shrink-0">{c.message_count} msgs</span>
                    </div>
                  </button>
                ))}
              </div>
            ))
          )}
        </div>
      </aside>

      {/* Main area */}
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex items-center gap-2 border-b border-line px-4 py-2.5">
          <select
            value={profileId ?? ""}
            onChange={async (e) => {
              setProfileId(e.target.value);
              await api.aiActivateProfile(e.target.value);
              qc.invalidateQueries({ queryKey: ["aiProfiles"] });
            }}
            className={inputCls}
            title="Which AI answers — the whole conversation carries over when you switch"
          >
            {noProfiles && <option value="">No AI connected</option>}
            {(profiles ?? []).map((p) => (
              <option key={p.id} value={p.id}>
                {p.name} · {p.model || "default model"}
              </option>
            ))}
          </select>
          {chat && (
            <select
              value={chat.project_id ?? ""}
              onChange={async (e) => {
                const v = e.target.value;
                await api.aiUpdateChat(
                  chat.id,
                  v === "" ? { clearProject: true } : { projectId: Number(v) },
                );
                qc.invalidateQueries({ queryKey: ["aiChats"] });
              }}
              className={inputCls}
              title="Give the AI this project's context (milestones, tasks, files, log)"
            >
              <option value="">No project context</option>
              {(projects ?? []).map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          )}
          <div className="ml-auto flex items-center gap-1.5">
            <button
              onClick={() => {
                setImporting((v) => !v);
                setManaging(false);
              }}
              className={importing ? primaryBtn : ghostBtn}
              title="Pull in past conversations from Claude Code, Codex, or a Claude/ChatGPT export"
            >
              {importing ? "Back to chat" : "Import chats"}
            </button>
            <button
              onClick={() => {
                setManaging((m) => !m);
                setImporting(false);
              }}
              className={managing ? primaryBtn : ghostBtn}
            >
              {managing ? "Back to chat" : "Manage AIs"}
            </button>
          </div>
        </div>

        {importing ? (
          <ImportPanel
            profileId={profileId}
            onImported={() => {
              qc.invalidateQueries({ queryKey: ["aiChats"] });
            }}
          />
        ) : managing || noProfiles ? (
          <ProviderManager
            profiles={profiles ?? []}
            onDone={() => setManaging(false)}
            onConnected={(id) => {
              setProfileId(id);
              setManaging(false);
            }}
          />
        ) : chat ? (
          <ChatThread
            key={chat.id}
            chatId={chat.id}
            profileId={profileId}
            profiles={profiles ?? []}
            projectId={chat.project_id}
            onDeleted={() => {
              setChatId(null);
              qc.invalidateQueries({ queryKey: ["aiChats"] });
            }}
          />
        ) : (
          <div className="flex flex-1 items-center justify-center">
            <div className="max-w-[380px] text-center">
              <div className="mb-1 text-[15px] font-semibold">Ask anything</div>
              <p className="mb-4 leading-relaxed text-muted">
                Chats are saved and organized on the left. Link a chat to a
                project to give the AI full context, attach files, and switch
                between your AIs mid-conversation — the history carries over.
              </p>
              <button onClick={newChat} className={primaryBtn}>
                Start a chat
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- providers

function ProviderManager({
  profiles,
  onDone,
  onConnected,
}: {
  profiles: AiProfile[];
  onDone: () => void;
  onConnected: (id: string) => void;
}) {
  const qc = useQueryClient();
  const { push } = useToasts();
  const [preset, setPreset] = useState(0);
  const [name, setName] = useState("");
  const [model, setModel] = useState(PRESETS[0].model);
  const [baseUrl, setBaseUrl] = useState(PRESETS[0].base_url);
  const [command, setCommand] = useState("");
  const [key, setKey] = useState("");
  const [keyEdits, setKeyEdits] = useState<Record<string, string>>({});

  const { data: detected, isFetching: detecting } = useQuery({
    queryKey: ["aiDetected"],
    queryFn: api.aiDetectProviders,
  });

  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["aiProfiles"] });
    qc.invalidateQueries({ queryKey: ["aiDetected"] });
  };

  const activePreset = PRESETS[preset];
  const connectDetected = async (d: DetectedProvider) => {
    try {
      const id = await api.aiSaveProfile({
        id: "",
        name: d.name,
        provider: d.provider,
        model: d.model,
        base_url: d.base_url,
        command: d.command,
        args: d.args,
      });
      await api.aiActivateProfile(id);
      push(`${d.name} connected — no key needed`);
      refresh();
      onConnected(id);
    } catch (e) {
      push(String(e), "error");
    }
  };

  const add = async () => {
    const p = activePreset;
    // Guard the common footgun: a keyed provider with no key won't answer.
    if (p.provider !== "cli" && p.provider !== "ollama" && !key.trim()) {
      const local = p.base_url.includes("127.0.0.1") || p.base_url.includes("localhost");
      if (!local) {
        push(`${name.trim() || p.label} needs an API key to answer`, "error");
        return;
      }
    }
    try {
      const id = await api.aiSaveProfile({
        id: "",
        name: name.trim() || p.label,
        provider: p.provider,
        model: model.trim(),
        base_url: p.provider === "cli" ? "" : baseUrl.trim(),
        command: p.provider === "cli" ? command.trim() || "claude" : "",
      });
      if (key.trim()) await api.aiSetProfileKey(id, key.trim());
      await api.aiActivateProfile(id);
      setName("");
      setKey("");
      push("AI connected");
      refresh();
      onConnected(id);
    } catch (e) {
      push(String(e), "error");
    }
  };

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[620px] px-6 pb-10 pt-6">
        {(detected ?? []).length > 0 && (
          <>
            <h2 className="mb-1 text-[15px] font-semibold">
              Found on this Mac — no key needed
            </h2>
            <p className="mb-3 text-[12px] leading-relaxed text-muted">
              Your signed-in AIs and local models. One click to connect; entries
              marked “Setup needed” show how to enable them.
            </p>
            <div className="mb-6 flex flex-col gap-2">
              {(detected ?? []).map((d) => {
                const already = profiles.some(
                  (p) => p.command === d.command && p.base_url === d.base_url && d.command + d.base_url !== "",
                );
                return (
                  <div
                    key={d.id}
                    className="flex items-center gap-3 rounded-panel border border-line bg-panel px-4 py-3"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="text-[13px] font-semibold">{d.name}</div>
                      <div className="text-[11.5px] leading-relaxed text-muted">{d.note}</div>
                    </div>
                    {d.connectable ? (
                      <button
                        onClick={() => connectDetected(d)}
                        disabled={already}
                        className={primaryBtn}
                      >
                        {already ? "Connected" : "Connect"}
                      </button>
                    ) : (
                      <span className="shrink-0 rounded-full border border-line px-2.5 py-1 text-[10.5px] font-medium text-muted">
                        Setup needed
                      </span>
                    )}
                  </div>
                );
              })}
            </div>
          </>
        )}
        {detecting && (detected ?? []).length === 0 && (
          <p className="mb-4 text-[12px] text-muted">Scanning this Mac…</p>
        )}

        <h2 className="mb-1 text-[15px] font-semibold">Connected AIs</h2>
        <p className="mb-4 text-[12px] leading-relaxed text-muted">
          Connect as many as you like — Claude, OpenAI, Hermes, Gemini, Grok,
          local models, or almost any OpenAI-compatible server. Keys (when a
          provider needs one) are stored in the macOS Keychain, never on disk.
        </p>

        <div className="mb-6 overflow-hidden rounded-panel border border-line">
          {profiles.length === 0 ? (
            <p className="bg-panel px-4 py-3 text-[12px] text-muted">
              Nothing connected yet — add your first AI below.
            </p>
          ) : (
            profiles.map((p) => (
              <div
                key={p.id}
                className="border-b border-line/50 bg-panel px-4 py-3 last:border-b-0"
              >
                <div className="flex items-center gap-2.5">
                  <span className="text-[13px] font-semibold">{p.name}</span>
                  <span className="font-mono text-[11px] text-muted">
                    {p.model || "default model"}
                  </span>
                  {p.active && (
                    <span className="rounded-full border border-solder/50 px-2 py-0.5 text-[10px] font-medium text-solder">
                      In use
                    </span>
                  )}
                  <div className="ml-auto flex items-center gap-1.5">
                    {!p.active && (
                      <button
                        onClick={async () => {
                          await api.aiActivateProfile(p.id);
                          refresh();
                        }}
                        className={ghostBtn}
                      >
                        Use
                      </button>
                    )}
                    <button
                      onClick={async () => {
                        await api.aiDeleteProfile(p.id);
                        push(`Removed ${p.name}`);
                        refresh();
                      }}
                      className="rounded-md px-2 py-1 text-[11.5px] text-st-late hover:bg-st-late/10"
                    >
                      Remove
                    </button>
                  </div>
                </div>
                {p.provider === "cli" && (
                  <div className="mt-1 text-[11.5px] text-muted">
                    Runs <span className="font-mono">{p.command}</span> locally — no key.
                  </div>
                )}
                {p.needs_key && (
                  <div className="mt-2 flex items-center gap-2">
                    <input
                      type="password"
                      value={keyEdits[p.id] ?? ""}
                      onChange={(e) =>
                        setKeyEdits((k) => ({ ...k, [p.id]: e.target.value }))
                      }
                      placeholder={
                        p.has_key ? "Key saved in Keychain — paste to replace" : "API key"
                      }
                      className={`${inputCls} flex-1`}
                    />
                    <button
                      onClick={async () => {
                        await api.aiSetProfileKey(p.id, keyEdits[p.id] ?? "");
                        setKeyEdits((k) => ({ ...k, [p.id]: "" }));
                        push((keyEdits[p.id] ?? "").trim() ? "Key saved" : "Key removed");
                        refresh();
                      }}
                      className={ghostBtn}
                    >
                      Save key
                    </button>
                  </div>
                )}
              </div>
            ))
          )}
        </div>

        <h3 className="mb-2 text-[13px] font-semibold">Add an AI</h3>
        <div className="flex flex-col gap-2 rounded-panel border border-line bg-panel p-4">
          <div className="flex gap-2">
            <select
              value={preset}
              onChange={(e) => {
                const i = Number(e.target.value);
                setPreset(i);
                setModel(PRESETS[i].model);
                setBaseUrl(PRESETS[i].base_url);
                setCommand(PRESETS[i].command ?? "");
              }}
              className={inputCls}
            >
              {PRESETS.map((p, i) => (
                <option key={p.label} value={i}>
                  {p.label}
                </option>
              ))}
            </select>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={`Name (defaults to “${PRESETS[preset].label}”)`}
              className={`${inputCls} flex-1`}
            />
          </div>
          {activePreset.provider === "cli" ? (
            <div className="flex gap-2">
              <input
                value={command}
                onChange={(e) => setCommand(e.target.value)}
                placeholder="Command on your PATH, e.g. codex"
                className={`${inputCls} flex-1`}
              />
              <input
                value={model}
                onChange={(e) => setModel(e.target.value)}
                placeholder="Model flag (optional)"
                className={`${inputCls} w-48`}
              />
            </div>
          ) : (
            <div className="flex gap-2">
              <input
                value={model}
                onChange={(e) => setModel(e.target.value)}
                placeholder="Model"
                className={`${inputCls} w-56`}
              />
              {activePreset.provider !== "anthropic" && (
                <input
                  value={baseUrl}
                  onChange={(e) => setBaseUrl(e.target.value)}
                  placeholder="Base URL"
                  className={`${inputCls} flex-1`}
                />
              )}
            </div>
          )}
          {activePreset.provider === "cli" && (
            <p className="text-[11px] leading-relaxed text-muted">
              Runs the command locally with the conversation as its prompt —
              no key stored anywhere. Only point this at a CLI you trust.
            </p>
          )}
          {activePreset.provider !== "ollama" && activePreset.provider !== "cli" && (
            <input
              type="password"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              placeholder="API key (stored in Keychain — optional for local servers)"
              className={inputCls}
            />
          )}
          <div className="flex items-center gap-3">
            <button onClick={add} className={primaryBtn}>
              Connect
            </button>
            <button onClick={onDone} className={ghostBtn}>
              Done
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- thread

export function ChatThread({
  chatId,
  profileId,
  profiles,
  projectId,
  onDeleted,
}: {
  chatId: number;
  profileId: string | null;
  profiles: AiProfile[];
  projectId: number | null;
  onDeleted: () => void;
}) {
  const qc = useQueryClient();
  const { push } = useToasts();
  const [draft, setDraft] = useState("");
  const [attachments, setAttachments] = useState<{ name: string; content: string }[]>([]);
  // "Bring context" lets you carry an earlier conversation (with any files that
  // were attached in it) into this one — including across different AIs, since
  // the transcript is plain text by the time it moves.
  const [pickingContext, setPickingContext] = useState(false);
  const { data: allChats } = useQuery({
    queryKey: ["aiChats"],
    queryFn: api.aiListChats,
    enabled: pickingContext,
  });
  const bottomRef = useRef<HTMLDivElement>(null);

  const { data: messages } = useQuery({
    queryKey: ["aiChat", chatId],
    queryFn: () => api.aiChatHistory(chatId),
  });

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ block: "end" });
  }, [messages?.length]);

  // The full text to send and a short display version are computed at click
  // time and passed AS THE MUTATION ARGUMENT. This is deliberate: onMutate
  // clears the draft (setDraft("")), which re-renders and would swap out any
  // closure that read `draft` inside mutationFn — sending an empty string and
  // erroring with "empty message". Passing the payload sidesteps that entirely.
  type SendPayload = {
    text: string;
    display: string;
    draft: string;
    attachments: { name: string; content: string }[];
  };
  const send = useMutation({
    mutationFn: async (payload: SendPayload) => {
      if (!profileId) throw new Error("Connect an AI first — open Manage AIs");
      if (!payload.text.trim()) throw new Error("Type a message first");
      return api.aiChatSend(chatId, payload.text, profileId);
    },
    onMutate: (payload: SendPayload) => {
      const optimistic: ChatMessageRow = {
        id: -Date.now(),
        role: "user",
        content: payload.display,
        provider: null,
        model: null,
        ts: new Date().toISOString(),
      };
      qc.setQueryData<ChatMessageRow[]>(["aiChat", chatId], (old) => [
        ...(old ?? []),
        optimistic,
      ]);
      // Clear the composer now that we've snapshotted everything in the payload.
      setDraft("");
      setAttachments([]);
      return { sentDraft: payload.draft, sentAttachments: payload.attachments };
    },
    onError: (e, _vars, ctx) => {
      // Put the message back in the box so nothing is ever lost.
      if (ctx?.sentDraft !== undefined) setDraft(ctx.sentDraft);
      if (ctx?.sentAttachments) setAttachments(ctx.sentAttachments);
      push(String(e), "error");
    },
    onSettled: () => {
      qc.invalidateQueries({ queryKey: ["aiChat", chatId] });
      qc.invalidateQueries({ queryKey: ["aiChats"] });
    },
  });

  // Build the payload from the current composer state and fire the send.
  const submit = () => {
    if (send.isPending) return;
    if (!profileId) {
      push("Connect an AI first — open Manage AIs", "error");
      return;
    }
    const body = draft.trim();
    if (!body && attachments.length === 0) return; // nothing to send
    let text = body;
    for (const a of attachments) {
      text += `\n\n[Attached file: ${a.name}]\n\`\`\`\n${a.content}\n\`\`\``;
    }
    const display =
      body +
      (attachments.length
        ? `${body ? "\n\n" : ""}(${attachments.length} file${attachments.length === 1 ? "" : "s"} attached)`
        : "");
    send.mutate({ text, display, draft, attachments });
  };

  const attach = async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({ multiple: true, title: "Attach files to the conversation" });
    if (!picked) return;
    for (const path of Array.isArray(picked) ? picked : [picked]) {
      try {
        const f = await api.readTextFile(path);
        setAttachments((a) => [...a, { name: f.name, content: f.content }]);
        if (f.truncated) push(`${f.name} is large — attached the first 64 KB`);
      } catch (e) {
        push(String(e), "error");
      }
    }
  };

  const pasteContext = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (!text.trim()) {
        push("Clipboard is empty");
        return;
      }
      setAttachments((a) => [...a, { name: "Pasted text", content: text }]);
    } catch {
      push("Couldn't read the clipboard", "error");
    }
  };

  const transcript = () =>
    (messages ?? [])
      .map((m) =>
        m.role === "user"
          ? `**You:**\n${m.content}`
          : `**${m.provider ?? "AI"} (${m.model ?? ""}):**\n${m.content}`,
      )
      .join("\n\n---\n\n");

  /** Pull another chat's transcript into this composer as quoted context. */
  const bringContext = async (fromId: number, title: string) => {
    try {
      const history = await api.aiChatHistory(fromId);
      if (history.length === 0) {
        push("That chat has no messages yet", "error");
        return;
      }
      const body = history
        .map((m) => `${m.role === "user" ? "Me" : "AI"}: ${m.content}`)
        .join("\n\n");
      const block = `Context from an earlier conversation ("${title}"):\n\n${body}\n\n---\n\n`;
      setDraft((d) => block + d);
      setPickingContext(false);
      push(`Brought ${history.length} messages from "${title}"`);
    } catch (e) {
      push(String(e), "error");
    }
  };

  const providerLabel = (m: ChatMessageRow) => {
    const p = profiles.find(
      (p) => p.model === m.model || p.provider === m.provider,
    );
    return p?.name ?? m.model ?? "AI";
  };

  return (
    <>
      <div className="flex-1 overflow-y-auto px-5 py-4">
        <div className="mx-auto flex max-w-[720px] flex-col gap-4">
          {(messages ?? []).map((m) => (
            <div key={m.id}>
              <div className="mb-1 flex items-baseline gap-2">
                <span className="text-[11px] font-semibold uppercase tracking-wide text-muted">
                  {m.role === "user" ? "You" : providerLabel(m)}
                </span>
                {m.role === "assistant" && m.model && (
                  <span className="font-mono text-[10px] text-muted">{m.model}</span>
                )}
              </div>
              <div
                className={`whitespace-pre-wrap rounded-panel border px-4 py-3 text-[13px] leading-relaxed ${
                  m.role === "user"
                    ? "border-line bg-panel-2"
                    : "border-line bg-panel"
                }`}
              >
                {m.content}
              </div>
            </div>
          ))}
          {send.isPending && (
            <div className="text-[12px] text-muted">Thinking…</div>
          )}
          <div ref={bottomRef} />
        </div>
      </div>

      {pickingContext && (
        <div className="border-t border-line bg-panel px-5 py-3">
          <div className="mb-1.5 flex items-center gap-2">
            <span className="text-[11.5px] font-medium">
              Bring an earlier conversation into this chat
            </span>
            <button
              onClick={() => setPickingContext(false)}
              className="ml-auto text-[11px] text-muted hover:text-text"
            >
              Cancel
            </button>
          </div>
          <div className="max-h-[180px] overflow-y-auto">
            {(allChats ?? []).filter((c) => c.id !== chatId).length === 0 ? (
              <p className="py-2 text-[11.5px] text-muted">
                No other chats yet. Import past conversations to pull context
                from them.
              </p>
            ) : (
              (allChats ?? [])
                .filter((c) => c.id !== chatId)
                .map((c) => (
                  <button
                    key={c.id}
                    onClick={() => bringContext(c.id, c.title)}
                    className="mb-0.5 block w-full rounded-md px-2.5 py-1.5 text-left hover:bg-panel-2"
                  >
                    <span className="block truncate text-[12px]">{c.title}</span>
                    <span className="text-[10.5px] text-muted">
                      {sourceLabel(c.source)} · {c.message_count} msgs
                    </span>
                  </button>
                ))
            )}
          </div>
        </div>
      )}

      <div className="border-t border-line px-5 py-3">
        <div className="mx-auto max-w-[720px]">
          {attachments.length > 0 && (
            <div className="mb-2 flex flex-wrap gap-1.5">
              {attachments.map((a, i) => (
                <span
                  key={i}
                  className="flex items-center gap-1.5 rounded-full border border-line bg-panel px-2.5 py-1 font-mono text-[11px]"
                >
                  {a.name}
                  <button
                    onClick={() =>
                      setAttachments((arr) => arr.filter((_, j) => j !== i))
                    }
                    className="text-muted hover:text-st-late"
                    title="Remove attachment"
                  >
                    remove
                  </button>
                </span>
              ))}
            </div>
          )}
          <div className="flex items-end gap-2">
            <textarea
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  submit();
                }
              }}
              rows={2}
              placeholder="Message — Enter to send, Shift+Enter for a new line"
              className="flex-1 resize-none rounded-panel border border-line bg-panel-2 px-3.5 py-2.5 text-[13px] placeholder:text-muted focus:border-solder focus:outline-none"
            />
            <div className="flex flex-col gap-1.5">
              <button
                onClick={submit}
                disabled={send.isPending || (!draft.trim() && attachments.length === 0)}
                className={primaryBtn}
              >
                Send
              </button>
              <button onClick={attach} className={ghostBtn} title="Attach a file's contents to the conversation">
                Attach file
              </button>
              <button onClick={pasteContext} className={ghostBtn} title="Attach whatever's on your clipboard">
                Paste context
              </button>
              <button
                onClick={() => setPickingContext((v) => !v)}
                className={ghostBtn}
                title="Carry an earlier conversation into this one — works across different AIs"
              >
                Bring context
              </button>
            </div>
          </div>
          <div className="mt-2 flex items-center gap-2 text-[11px] text-muted">
            <span>Switching AI above carries the whole conversation over.</span>
            <div className="ml-auto flex items-center gap-1.5">
              <button
                onClick={() => {
                  navigator.clipboard.writeText(transcript());
                  push("Chat copied as Markdown");
                }}
                className="hover:text-text"
              >
                Copy chat
              </button>
              {projectId !== null && (
                <button
                  onClick={async () => {
                    await api.addLog(projectId, `AI chat transcript\n\n${transcript()}`);
                    push("Saved to the project log");
                  }}
                  className="hover:text-text"
                >
                  Save to project log
                </button>
              )}
              <button
                onClick={async () => {
                  await api.aiDeleteChat(chatId);
                  push("Chat deleted");
                  onDeleted();
                }}
                className="hover:text-st-late"
              >
                Delete chat
              </button>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
