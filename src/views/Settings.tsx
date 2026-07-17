import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { api, RuleRow } from "../lib/api";
import { useToasts } from "../lib/store";

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

        <RulesEditor />
      </div>
    </div>
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
