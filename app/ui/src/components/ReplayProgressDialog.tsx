import { useEffect, useState } from "react";
import { api, type ReplayBatchInfo } from "../api";
import { say, set, useStore } from "../store";
import { fmtInt } from "../lib/format";
import { t } from "../i18n";

function batchState(b: ReplayBatchInfo) {
  if (b.submissionDone && b.running === 0) return b.stopped ? t("Stopped") : t("Completed");
  if (b.stopped) return t("Finishing requests already submitted");
  return t("Running");
}

function BatchCard({ batch: b }: { batch: ReplayBatchInfo }) {
  const [busy, setBusy] = useState(false);
  const metrics = [
    [t("Planned"), b.planned], [t("Submitted"), b.submitted], [t("Not submitted"), b.planned - b.submitted],
    [t("In flight"), b.running], [t("Completed"), b.completed], [t("Failed"), b.failed], [t("Peak concurrency"), b.peak],
  ] as const;
  const ms = (n: number | null) => n == null ? "—" : `${n.toLocaleString(undefined, { maximumFractionDigits: 1 })} ms`;
  return <article className="replay-batch">
    <div className="replay-batch-head">
      <b>{t("Replay batch #{id}", { id: b.id })}</b><span>{batchState(b)}</span>
      <span className="tp-spacer" />
      {!b.submissionDone && !b.stopped && <button disabled={busy} onClick={async () => {
        setBusy(true);
        try {
          await api.replayStopBatch(b.id);
          set({ replayBatches: await api.replayBatches() });
        } catch (e) { say(String(e), "error"); }
        finally { setBusy(false); }
      }}>{t("Stop This Batch")}</button>}
    </div>
    <progress max={b.planned || 1} value={b.completed} aria-label={t("Completed requests")} />
    <div className="replay-metrics">{metrics.map(([label, value]) => <div key={label}><span className="muted">{label}</span><b>{fmtInt(value)}</b></div>)}</div>
    <p className="small muted">{t("Elapsed: {seconds} s · Throughput: {rate} requests/s", { seconds: (b.elapsedMs / 1000).toFixed(1), rate: b.requestsPerSecond.toFixed(1) })}</p>
    <p className="small muted">{t("Request duration: mean {mean} · min {min} · max {max}", { mean: b.completed ? ms(b.meanMs) : "—", min: ms(b.minMs), max: ms(b.maxMs) })}</p>
  </article>;
}

export function ReplayProgressPanel() {
  const batches = useStore((s) => s.replayBatches);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    void api.replayBatches().then((replayBatches) => alive && set({ replayBatches })).catch((e) => alive && setError(String(e)));
    return () => { alive = false; };
  }, []);
  return <div className="replay-progress">
    <p className="muted small">{t("Each batch counts only its own requests. Failures include network errors and HTTP status 400 or above. Durations include breakpoint waits. Stop ends submission; in-flight requests finish.")}</p>
    <p className="muted small">{t("In-flight and peak counts include requests waiting for a connection or breakpoint.")}</p>
    {error && <p className="err">{error}</p>}
    {!batches.length && <p className="muted">{t("No replay batches yet. Use Replay or Shift+R to start one.")}</p>}
    {batches.map((b) => <BatchCard key={b.id} batch={b} />)}
  </div>;
}
