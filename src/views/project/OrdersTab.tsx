import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../../lib/api";
import { useToasts } from "../../lib/store";

const STATUS_FLOW = ["ordered", "shipped", "arrived", "issue"] as const;
const STATUS_COLOR: Record<string, string> = {
  ordered: "#8A97AC",
  shipped: "#38BDF8",
  arrived: "#22D3A6",
  issue: "#F5556D",
};

export function OrdersTab({ projectId }: { projectId: number }) {
  const [showForm, setShowForm] = useState(false);
  const qc = useQueryClient();
  const { push } = useToasts();

  const { data: orders } = useQuery({
    queryKey: ["orders", projectId],
    queryFn: () => api.listOrders(projectId),
  });
  const { data: spend } = useQuery({
    queryKey: ["spend", projectId],
    queryFn: () => api.spendSummary(projectId),
  });

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ["orders", projectId] });
    qc.invalidateQueries({ queryKey: ["spend", projectId] });
  };

  const setStatus = useMutation({
    mutationFn: ({ id, status }: { id: number; status: string }) =>
      api.updateOrderStatus(id, status),
    onSuccess: invalidate,
    onError: (e) => push(String(e), "error"),
  });

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[860px] px-6 py-5">
        <div className="mb-4 flex items-center gap-4">
          <div className="flex gap-6 rounded-panel border border-line bg-panel px-5 py-3">
            <div>
              <div className="text-[10px] uppercase tracking-wide text-muted">total spend</div>
              <div className="font-mono text-[16px] font-medium">
                ${((spend?.total_cents ?? 0) / 100).toFixed(2)}
              </div>
            </div>
            <div>
              <div className="text-[10px] uppercase tracking-wide text-muted">in flight</div>
              <div className="font-mono text-[16px] font-medium text-st-risk">
                ${((spend?.in_flight_cents ?? 0) / 100).toFixed(2)}
              </div>
            </div>
          </div>
          <button
            onClick={() => setShowForm(!showForm)}
            className="ml-auto rounded-lg bg-solder px-3.5 py-1.5 text-[12px] font-semibold text-ink"
          >
            {showForm ? "Close" : "Add order"}
          </button>
        </div>

        {showForm && (
          <OrderForm
            projectId={projectId}
            onDone={() => {
              setShowForm(false);
              invalidate();
            }}
          />
        )}

        <div className="overflow-hidden rounded-panel border border-line">
          {(orders ?? []).length === 0 ? (
            <p className="bg-panel px-4 py-6 text-center text-[12px] text-muted">
              No orders yet. Track PCBs, parts, stencils — spend totals and
              arrival alerts come free.
            </p>
          ) : (
            (orders ?? []).map((o) => (
              <div
                key={o.id}
                className="group flex items-center gap-3 border-b border-line/50 bg-panel px-4 py-3 last:border-b-0"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[13px] font-medium">{o.vendor}</span>
                    {o.ref && <span className="font-mono text-[11px] text-muted">#{o.ref}</span>}
                  </div>
                  {o.items && <div className="truncate text-[11.5px] text-muted">{o.items}</div>}
                </div>
                <span className="font-mono text-[12px]">${(o.cost_cents / 100).toFixed(2)}</span>
                {o.eta && (
                  <span className="font-mono text-[11px] text-muted" title="ETA">
                    → {o.eta.slice(5)}
                  </span>
                )}
                {o.tracking_url && (
                  <button
                    onClick={() => openUrl(o.tracking_url!)}
                    className="text-[11px] text-muted underline hover:text-solder"
                  >
                    track
                  </button>
                )}
                <select
                  value={o.status}
                  onChange={(e) => setStatus.mutate({ id: o.id, status: e.target.value })}
                  className="rounded-md border border-line bg-panel-2 px-2 py-1 text-[11px]"
                  style={{ color: STATUS_COLOR[o.status] }}
                >
                  {STATUS_FLOW.map((s) => (
                    <option key={s} value={s}>
                      {s}
                    </option>
                  ))}
                </select>
                <button
                  onClick={async () => {
                    await api.deleteOrder(o.id);
                    invalidate();
                  }}
                  className="hidden text-[11px] text-muted hover:text-st-late group-hover:block"
                >
                  ✕
                </button>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

function OrderForm({ projectId, onDone }: { projectId: number; onDone: () => void }) {
  const [vendor, setVendor] = useState("");
  const [ref, setRef] = useState("");
  const [items, setItems] = useState("");
  const [cost, setCost] = useState("");
  const [eta, setEta] = useState("");
  const [tracking, setTracking] = useState("");
  const { push } = useToasts();

  const submit = async () => {
    if (!vendor.trim()) return;
    try {
      await api.addOrder({
        project_id: projectId,
        vendor: vendor.trim(),
        ref: ref.trim() || null,
        items: items.trim() || null,
        cost_cents: Math.round(parseFloat(cost || "0") * 100),
        eta: eta || null,
        tracking_url: tracking.trim() || null,
      });
      onDone();
    } catch (e) {
      push(String(e), "error");
    }
  };

  const input =
    "rounded-md border border-line bg-panel-2 px-2.5 py-1.5 text-[12px] placeholder:text-muted focus:border-solder focus:outline-none";

  return (
    <div className="mb-4 grid grid-cols-3 gap-2 rounded-panel border border-line bg-panel p-4">
      <input className={input} placeholder="Vendor (JLCPCB, LCSC…)" value={vendor} onChange={(e) => setVendor(e.target.value)} />
      <input className={input} placeholder="Order ref" value={ref} onChange={(e) => setRef(e.target.value)} />
      <input className={input} placeholder="Cost (USD)" value={cost} onChange={(e) => setCost(e.target.value)} />
      <input className={`${input} col-span-2`} placeholder="Items" value={items} onChange={(e) => setItems(e.target.value)} />
      <input className={input} type="date" value={eta} onChange={(e) => setEta(e.target.value)} title="ETA" />
      <input className={`${input} col-span-2`} placeholder="Tracking URL" value={tracking} onChange={(e) => setTracking(e.target.value)} />
      <button
        onClick={submit}
        disabled={!vendor.trim()}
        className="rounded-md bg-solder px-3 py-1.5 text-[12px] font-semibold text-ink disabled:opacity-40"
      >
        Add order
      </button>
    </div>
  );
}
