import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../lib/api";

export function RootPicker() {
  const qc = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  const { data: defaultPath } = useQuery({
    queryKey: ["defaultRoot"],
    queryFn: api.defaultRoot,
  });

  const setRoot = useMutation({
    mutationFn: api.setRoot,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["root"] });
      qc.invalidateQueries({ queryKey: ["projects"] });
    },
    onError: (e) => setError(String(e)),
  });

  const choose = async () => {
    const dir = await open({ directory: true, title: "Choose your projects folder" });
    if (typeof dir === "string") setRoot.mutate(dir);
  };

  return (
    <div className="flex flex-1 items-center justify-center">
      <div className="w-[420px] rounded-panel border border-line bg-panel p-8">
        <div className="mb-1 text-[20px] font-semibold">Welcome to Hangar</div>
        <p className="mb-6 leading-relaxed text-muted">
          Pick the folder where your projects live. Every project is a normal
          folder on disk — Hangar is just the index and UI on top. Delete the
          app and your files stay perfectly organized.
        </p>
        <div className="flex flex-col gap-2">
          <button
            onClick={() => defaultPath && setRoot.mutate(defaultPath)}
            disabled={!defaultPath || setRoot.isPending}
            className="rounded-lg bg-solder px-4 py-2.5 text-[13px] font-semibold text-ink transition-opacity hover:opacity-90 disabled:opacity-50"
          >
            Use{" "}
            <span className="font-mono">
              {defaultPath?.replace(/^\/Users\/[^/]+/, "~") ?? "~/Projects"}
            </span>
          </button>
          <button
            onClick={choose}
            disabled={setRoot.isPending}
            className="rounded-lg border border-line bg-panel-2 px-4 py-2.5 text-[13px] font-medium transition-colors hover:border-line-strong disabled:opacity-50"
          >
            Choose another folder…
          </button>
        </div>
        {setRoot.isPending && (
          <p className="mt-4 font-mono text-[11px] text-muted">Scanning…</p>
        )}
        {error && (
          <p className="mt-4 text-[12px] text-st-late">{error}</p>
        )}
      </div>
    </div>
  );
}
