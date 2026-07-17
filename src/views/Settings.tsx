import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { api, RuleRow } from "../lib/api";
import { Theme, useTheme, useToasts } from "../lib/store";

export function Settings({ root }: { root: string | null }) {
  const qc = useQueryClient();
  const { push } = useToasts();

  const setRoot = useMutation({
    mutationFn: api.setRoot,
    onSuccess: () => qc.invalidateQueries(),
    onError: (e) => push(String(e), "error"),
  });

  const changeRoot = async () => {
    const dir = await open({ directory: true, title: "Choose your projects folder" });
    if (typeof dir === "string") setRoot.mutate(dir);
  };

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[640px] px-6 pb-10 pt-5">
        <h1 className="mb-6 text-[20px] font-semibold">Settings</h1>

        <Appearance />

        <section className="mb-8">
          <h2 className="mb-2 text-[14px] font-semibold">Root folder</h2>
          <div className="flex items-center gap-3 rounded-panel border border-line bg-panel px-4 py-3">
            <span className="flex-1 truncate font-mono text-[12px]">
              {root?.replace(/^\/Users\/[^/]+/, "~") ?? "—"}
            </span>
            <button
              onClick={changeRoot}
              className="rounded-md border border-line px-3 py-1.5 text-[12px] text-muted hover:border-solder hover:text-solder"
            >
              Change…
            </button>
          </div>
          <p className="mt-2 text-[12px] leading-relaxed text-muted">
            Every project is a real folder inside the root. The database is
            just a cache — Rescan rebuilds everything from disk and sidecars.
          </p>
        </section>

        <AiSection />
        <McpSection />
        <RulesEditor />
        <WatchedFolders />
        <Backups />
      </div>
    </div>
  );
}

function Appearance() {
  const { theme, setTheme } = useTheme();
  const options: { id: Theme; label: string }[] = [
    { id: "system", label: "System" },
    { id: "light", label: "Light" },
    { id: "dark", label: "Dark" },
  ];
  return (
    <section className="mb-8">
      <h2 className="mb-2 text-[14px] font-semibold">Appearance</h2>
      <div className="flex w-fit rounded-lg border border-line p-0.5">
        {options.map((o) => (
          <button
            key={o.id}
            onClick={() => setTheme(o.id)}
            className={`rounded-md px-4 py-1.5 text-[12px] font-medium transition-colors ${
              theme === o.id ? "bg-panel-2 text-text" : "text-muted hover:text-text"
            }`}
          >
            {o.label}
          </button>
        ))}
      </div>
    </section>
  );
}

function AiSection() {
  const qc = useQueryClient();
  const { push } = useToasts();
  const { data: config } = useQuery({ queryKey: ["aiConfig"], queryFn: api.aiGetConfig });
  const { data: usage } = useQuery({ queryKey: ["aiUsage"], queryFn: api.aiUsage });
  const [key, setKey] = useState("");
  const [testing, setTesting] = useState(false);

  const setConfig = async (patch: Partial<{ provider: string; model: string; base_url: string }>) => {
    if (!config) return;
    await api.aiSetConfig(
      patch.provider ?? config.provider,
      patch.model ?? config.model,
      patch.base_url ?? config.base_url,
    );
    qc.invalidateQueries({ queryKey: ["aiConfig"] });
  };

  const input =
    "rounded-md border border-line bg-panel-2 px-2.5 py-1.5 text-[12px] placeholder:text-muted focus:border-solder focus:outline-none";

  return (
    <section className="mt-8">
      <h2 className="mb-2 text-[14px] font-semibold">AI</h2>
      <p className="mb-3 text-[12px] leading-relaxed text-muted">
        Optional. Cloud Claude or a local model — filing, summaries,
        milestones, renames. Every AI action is a plan you approve first, and
        nothing ever leaves your machine without you hitting Apply.
      </p>
      <div className="flex flex-col gap-2 rounded-panel border border-line bg-panel p-4">
        <div className="flex gap-2">
          <select
            value={config?.provider ?? "none"}
            onChange={(e) => setConfig({ provider: e.target.value })}
            className={input}
          >
            <option value="none">Off</option>
            <option value="anthropic">Anthropic (Claude)</option>
            <option value="ollama">Ollama (local)</option>
            <option value="openai">OpenAI-compatible</option>
          </select>
          <input
            className={`${input} w-52`}
            placeholder={
              config?.provider === "ollama" ? "llama3.2" : "claude-sonnet-4-6"
            }
            defaultValue={config?.model}
            key={`model-${config?.model}`}
            onBlur={(e) => setConfig({ model: e.target.value })}
            title="Model (blur to save)"
          />
          {(config?.provider === "ollama" || config?.provider === "openai") && (
            <input
              className={`${input} flex-1`}
              placeholder={
                config?.provider === "ollama"
                  ? "http://localhost:11434"
                  : "https://…/v1"
              }
              defaultValue={config?.base_url}
              key={`base-${config?.base_url}`}
              onBlur={(e) => setConfig({ base_url: e.target.value })}
              title="Base URL (blur to save)"
            />
          )}
        </div>
        {config?.provider === "anthropic" && (
          <div className="flex gap-2">
            <input
              type="password"
              className={`${input} flex-1`}
              placeholder={config.has_key ? "API key saved in Keychain — paste to replace" : "sk-ant-…"}
              value={key}
              onChange={(e) => setKey(e.target.value)}
            />
            <button
              onClick={async () => {
                await api.aiSetKey(key);
                setKey("");
                push(key.trim() ? "Key saved to macOS Keychain" : "Key removed");
                qc.invalidateQueries({ queryKey: ["aiConfig"] });
              }}
              className="rounded-md border border-line px-3 py-1.5 text-[12px] text-muted hover:border-solder hover:text-solder"
            >
              Save key
            </button>
          </div>
        )}
        {config?.provider === "ollama" && (
          <button
            onClick={async () => {
              try {
                const models = await api.aiOllamaModels();
                push(models.length ? `Ollama models: ${models.join(", ")}` : "Ollama has no models pulled");
              } catch (e) {
                push(String(e), "error");
              }
            }}
            className="self-start rounded-md border border-line px-3 py-1.5 text-[12px] text-muted hover:border-solder hover:text-solder"
          >
            List local models
          </button>
        )}
        {config && config.provider !== "none" && (
          <div className="flex items-center gap-3">
            <button
              onClick={async () => {
                setTesting(true);
                try {
                  const reply = await api.aiTest();
                  push(`Connection OK — "${reply.trim()}"`);
                } catch (e) {
                  push(String(e), "error");
                } finally {
                  setTesting(false);
                  qc.invalidateQueries({ queryKey: ["aiUsage"] });
                }
              }}
              disabled={testing}
              className="rounded-md bg-solder px-3 py-1.5 text-[12px] font-semibold text-ink disabled:opacity-50"
            >
              {testing ? "Testing…" : "Test connection"}
            </button>
            {usage && (
              <span className="font-mono text-[11px] text-muted">
                this month: {usage.month_runs} runs ·{" "}
                {(usage.month_tokens_in + usage.month_tokens_out).toLocaleString()} tokens
              </span>
            )}
          </div>
        )}
      </div>
    </section>
  );
}

function WatchedFolders() {
  const qc = useQueryClient();
  const { push } = useToasts();
  const { data: dirs } = useQuery({
    queryKey: ["watchedDirs"],
    queryFn: api.getWatchedDirs,
  });
  const { data: patterns } = useQuery({
    queryKey: ["sweepPatterns"],
    queryFn: api.getSweepPatterns,
  });

  const addDir = async () => {
    const dir = await open({ directory: true, title: "Watch a folder" });
    if (typeof dir !== "string") return;
    await api.setWatchedDirs([...(dirs ?? []), dir]);
    push("Watching folder — matching files sweep to Inbox");
    qc.invalidateQueries({ queryKey: ["watchedDirs"] });
  };

  return (
    <section className="mt-8">
      <h2 className="mb-2 text-[14px] font-semibold">Watched folders</h2>
      <p className="mb-3 text-[12px] leading-relaxed text-muted">
        New files matching the sweep patterns in these folders (Downloads,
        say) move straight into <span className="font-mono">_Inbox</span>.
      </p>
      <div className="mb-2 overflow-hidden rounded-panel border border-line">
        {(dirs ?? []).length === 0 ? (
          <p className="bg-panel px-4 py-3 text-[12px] text-muted">
            No watched folders yet.
          </p>
        ) : (
          (dirs ?? []).map((d) => (
            <div
              key={d}
              className="flex items-center gap-3 border-b border-line/50 bg-panel px-4 py-2.5 last:border-b-0"
            >
              <span className="flex-1 truncate font-mono text-[12px]">
                {d.replace(/^\/Users\/[^/]+/, "~")}
              </span>
              <button
                onClick={async () => {
                  await api.setWatchedDirs((dirs ?? []).filter((x) => x !== d));
                  qc.invalidateQueries({ queryKey: ["watchedDirs"] });
                }}
                className="text-[11px] text-muted hover:text-st-late"
              >
                Remove
              </button>
            </div>
          ))
        )}
      </div>
      <div className="flex gap-2">
        <button
          onClick={addDir}
          className="rounded-md border border-line px-3 py-1.5 text-[12px] text-muted hover:border-solder hover:text-solder"
        >
          Watch a folder…
        </button>
        <input
          defaultValue={patterns}
          key={patterns}
          onBlur={async (e) => {
            await api.setSweepPatterns(e.target.value);
            push("Sweep patterns updated");
          }}
          placeholder="*.zip,*.pdf,*.step"
          className="flex-1 rounded-md border border-line bg-panel-2 px-3 py-1.5 font-mono text-[12px] placeholder:text-muted focus:border-solder focus:outline-none"
          title="Sweep patterns (blur to save)"
        />
      </div>
    </section>
  );
}

function Backups() {
  const qc = useQueryClient();
  const { push } = useToasts();
  const { data: status } = useQuery({
    queryKey: ["backup"],
    queryFn: api.backupStatus,
  });
  const backup = useMutation({
    mutationFn: api.runBackup,
    onSuccess: (path) => {
      push(`Backup verified → ${path.split("/").pop()}`);
      qc.invalidateQueries({ queryKey: ["backup"] });
    },
    onError: (e) => push(String(e), "error"),
  });

  return (
    <section className="mt-8">
      <h2 className="mb-2 text-[14px] font-semibold">Backups</h2>
      <p className="mb-3 text-[12px] leading-relaxed text-muted">
        Zips the whole root to a destination of your choice (an external SSD,
        ideally), verifies the archive, and keeps the last {status?.keep ?? 5}.
      </p>
      <div className="flex items-center gap-2">
        <button
          onClick={async () => {
            const dir = await open({ directory: true, title: "Backup destination" });
            if (typeof dir !== "string") return;
            await api.setBackupDir(dir);
            qc.invalidateQueries({ queryKey: ["backup"] });
          }}
          className="rounded-md border border-line px-3 py-1.5 text-[12px] text-muted hover:border-solder hover:text-solder"
        >
          {status?.backup_dir
            ? status.backup_dir.replace(/^\/Users\/[^/]+/, "~")
            : "Choose destination…"}
        </button>
        <button
          onClick={() => backup.mutate()}
          disabled={!status?.backup_dir || backup.isPending}
          className="rounded-md bg-solder px-3 py-1.5 text-[12px] font-semibold text-ink disabled:opacity-40"
        >
          {backup.isPending ? "Backing up…" : "Back up now"}
        </button>
        {status?.last_backup && (
          <span className="font-mono text-[11px] text-muted">
            last: {status.last_backup.slice(0, 16)}
          </span>
        )}
      </div>
      {(status?.backups.length ?? 0) > 0 && (
        <div className="mt-2 font-mono text-[11px] text-muted">
          {status!.backups.map(([name]) => name).join(" · ")}
        </div>
      )}
    </section>
  );
}

function McpSection() {
  const { push } = useToasts();
  const { data: info } = useQuery({ queryKey: ["mcp"], queryFn: api.mcpInfo });

  return (
    <section className="mt-8">
      <h2 className="mb-2 text-[14px] font-semibold">MCP server</h2>
      <p className="mb-3 text-[12px] leading-relaxed text-muted">
        Claude Code and Claude Desktop can drive Hangar directly — create
        projects, file things, complete tasks, set progress. Local only,
        bearer-token protected.
      </p>
      {info && (
        <div className="flex flex-col gap-2 rounded-panel border border-line bg-panel p-4">
          <div className="flex items-center gap-2">
            <span className="w-14 text-[11px] text-muted">URL</span>
            <span className="font-mono text-[12px]">{info.url}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="w-14 text-[11px] text-muted">Token</span>
            <span className="font-mono text-[12px]">
              {info.token.slice(0, 6)}…{info.token.slice(-4)}
            </span>
            <button
              onClick={() => {
                navigator.clipboard.writeText(info.token);
                push("Token copied");
              }}
              className="rounded-md border border-line px-2 py-0.5 text-[11px] text-muted hover:border-solder hover:text-solder"
            >
              Copy
            </button>
          </div>
          <button
            onClick={() => {
              navigator.clipboard.writeText(info.install_cmd);
              push("Install command copied — paste it in a terminal");
            }}
            className="mt-1 self-start rounded-md bg-solder px-3 py-1.5 text-[12px] font-semibold text-ink"
          >
            Copy `claude mcp add` command
          </button>
        </div>
      )}
    </section>
  );
}

function RulesEditor() {
  const qc = useQueryClient();
  const { push } = useToasts();
  const { data: rules } = useQuery({ queryKey: ["rules"], queryFn: api.listRules });

  const [pattern, setPattern] = useState("");
  const [matchKind, setMatchKind] = useState("glob");
  const [tester, setTester] = useState("BOM_rev2.csv\nboard-F_Cu.gbr\nenclosure.step");
  const { data: testResults } = useQuery({
    queryKey: ["ruletest", pattern, matchKind, tester],
    queryFn: () =>
      api.testRule(
        pattern,
        matchKind,
        tester.split("\n").filter((s) => s.trim()),
      ),
    enabled: pattern.trim().length > 0,
  });

  const save = useMutation({
    mutationFn: () =>
      api.saveRule({ pattern: pattern.trim(), matchKind, enabled: true }),
    onSuccess: () => {
      push("Rule saved");
      setPattern("");
      qc.invalidateQueries({ queryKey: ["rules"] });
    },
    onError: (e) => push(String(e), "error"),
  });

  const del = useMutation({
    mutationFn: (id: number) => api.deleteRule(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["rules"] }),
  });

  const toggle = useMutation({
    mutationFn: (r: RuleRow) =>
      api.saveRule({
        id: r.id,
        projectId: r.project_id,
        pattern: r.pattern,
        matchKind: r.match_kind,
        destBinId: r.dest_bin_id,
        enabled: !r.enabled,
      }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["rules"] }),
  });

  const samples = tester.split("\n").filter((s) => s.trim());

  return (
    <section>
      <h2 className="mb-2 text-[14px] font-semibold">Filing rules</h2>
      <p className="mb-3 text-[12px] leading-relaxed text-muted">
        Rules suggest a bin for Inbox items. Built-ins cover Gerbers, CAD,
        BOMs, datasheets, photos and firmware; your rules run first.
      </p>

      <div className="mb-4 rounded-panel border border-line bg-panel p-4">
        <div className="mb-2 flex gap-2">
          <select
            value={matchKind}
            onChange={(e) => setMatchKind(e.target.value)}
            className="rounded-md border border-line bg-panel-2 px-2 py-1.5 text-[12px]"
          >
            <option value="glob">Glob</option>
            <option value="ext">Extensions</option>
            <option value="regex">Regex</option>
          </select>
          <input
            value={pattern}
            onChange={(e) => setPattern(e.target.value)}
            placeholder={
              matchKind === "ext" ? "gbr,drl,gtl" : matchKind === "glob" ? "BOM*.csv,*.step" : "^bom.*csv$"
            }
            className="flex-1 rounded-md border border-line bg-panel-2 px-3 py-1.5 font-mono text-[12px] placeholder:text-muted focus:border-solder focus:outline-none"
          />
          <button
            onClick={() => save.mutate()}
            disabled={!pattern.trim()}
            className="rounded-md bg-solder px-3 py-1.5 text-[12px] font-semibold text-ink disabled:opacity-40"
          >
            Add rule
          </button>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <textarea
            value={tester}
            onChange={(e) => setTester(e.target.value)}
            rows={3}
            placeholder="Test filenames, one per line"
            className="resize-none rounded-md border border-line bg-panel-2 px-3 py-2 font-mono text-[11px] focus:border-solder focus:outline-none"
          />
          <div className="flex flex-col justify-center gap-1 font-mono text-[11px]">
            {pattern.trim() ? (
              samples.map((s, i) => (
                <span
                  key={i}
                  className={testResults?.[i] ? "text-solder" : "text-muted"}
                >
                  {testResults?.[i] ? "✓" : "✗"} {s}
                </span>
              ))
            ) : (
              <span className="text-muted">Type a pattern to live-test it</span>
            )}
          </div>
        </div>
      </div>

      <div className="overflow-hidden rounded-panel border border-line">
        {(rules ?? []).length === 0 ? (
          <p className="bg-panel px-4 py-3 text-[12px] text-muted">
            No custom rules yet — built-ins are active.
          </p>
        ) : (
          (rules ?? []).map((r) => (
            <div
              key={r.id}
              className="flex items-center gap-3 border-b border-line/50 bg-panel px-4 py-2.5 last:border-b-0"
            >
              <span className="w-14 font-mono text-[10px] uppercase text-muted">
                {r.match_kind}
              </span>
              <span className="flex-1 truncate font-mono text-[12px]">
                {r.pattern}
              </span>
              {r.dest_bin_name && (
                <span className="text-[11px] text-muted">→ {r.dest_bin_name}</span>
              )}
              <button
                onClick={() => toggle.mutate(r)}
                className={`rounded-md px-2 py-0.5 text-[11px] ${
                  r.enabled ? "text-solder" : "text-muted"
                }`}
              >
                {r.enabled ? "On" : "Off"}
              </button>
              <button
                onClick={() => del.mutate(r.id)}
                className="rounded-md px-2 py-0.5 text-[11px] text-st-late hover:bg-st-late/10"
              >
                Delete
              </button>
            </div>
          ))
        )}
      </div>
    </section>
  );
}
