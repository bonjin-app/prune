import { useEffect, useState } from "react";
import { AlertTriangle, Container, Lock, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Card, CardHeader } from "@/components/ui/Card";
import { Dialog } from "@/components/ui/Dialog";
import { formatBytes, formatCount } from "@/lib/format";
import { backend, errorMessage } from "@/lib/tauri";
import {
  DOCKER_ACTION_COMMAND,
  DOCKER_ACTION_DESCRIPTION,
  DOCKER_ACTION_LABEL,
  type DockerAction,
  type DockerPruneResult,
  type DockerState,
} from "@/types/models";

/**
 * Docker, which is often the largest single thing on a developer's disk.
 *
 * It is also the one thing Prune cannot treat as files: images and build cache live inside a
 * disk image the daemon owns. So this card does not offer checkboxes and a preview like the
 * rest of the app. It shows what Docker reports and runs one of two Docker commands, naming
 * the exact command first, because Prune cannot undo what Docker removes.
 */
export function DockerCard() {
  const [state, setState] = useState<DockerState | null>(null);
  const [loading, setLoading] = useState(false);
  const [confirming, setConfirming] = useState<DockerAction | null>(null);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<DockerPruneResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    try {
      setState(await backend.dockerStatus());
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setLoading(false);
    }
  };

  // Read once per visit; Docker usage does not move on its own.
  useEffect(() => {
    void load();
  }, []);

  const run = async (action: DockerAction) => {
    setRunning(true);
    try {
      setResult(await backend.dockerPrune(action));
      setConfirming(null);
      await load();
    } catch (e) {
      setError(errorMessage(e));
      setConfirming(null);
    } finally {
      setRunning(false);
    }
  };

  // A machine without Docker should not be shown an empty Docker panel.
  if (!state || state.state === "not_installed") return null;

  return (
    <>
      <Card>
        <CardHeader
          title={
            <span className="flex items-center gap-1.5">
              <Container size={14} /> Docker
            </span>
          }
          subtitle={
            state.state === "ready"
              ? `${formatBytes(state.usage.totalBytes)} in use, ${formatBytes(state.usage.reclaimableBytes)} reclaimable`
              : "Docker is installed but not answering."
          }
          action={
            <Button size="sm" variant="ghost" onClick={() => void load()} loading={loading}>
              <RefreshCw size={12} /> Refresh
            </Button>
          }
        />
        <div className="px-4 pb-4">
          {state.state === "not_running" ? (
            <p className="text-[12.5px] text-fg-muted">{state.message}</p>
          ) : (
            <>
              <div className="overflow-hidden rounded-md border border-line">
                {state.usage.entries.map((entry) => (
                  <div
                    key={entry.kind}
                    className="flex items-center gap-3 border-t border-line/60 px-3 py-[7px] first:border-t-0"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-1.5">
                        <span className="text-[12.5px] font-medium">{entry.label}</span>
                        <span className="text-[11px] text-fg-faint tnum">
                          {formatCount(entry.activeCount)} of {formatCount(entry.totalCount)} in use
                        </span>
                        {!entry.reclaimableByPrune && (
                          <span title={entry.note} className="text-fg-faint">
                            <Lock size={11} />
                          </span>
                        )}
                      </div>
                      <div className="truncate text-[11px] text-fg-faint">{entry.note}</div>
                    </div>
                    <div className="w-[80px] text-right text-[11px] text-fg-faint tnum">
                      {entry.reclaimableByPrune
                        ? `${formatBytes(entry.reclaimableBytes)} free`
                        : "kept"}
                    </div>
                    <div className="w-[76px] text-right text-[12.5px] font-medium tnum">
                      {formatBytes(entry.sizeBytes)}
                    </div>
                  </div>
                ))}
              </div>
              <div className="mt-3 flex items-center gap-2">
                <p className="flex-1 text-[11.5px] text-fg-faint">
                  Docker removes these itself, so there is no preview and no trash. Volumes are
                  never touched.
                </p>
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={() => setConfirming("builder_prune")}
                >
                  {DOCKER_ACTION_LABEL.builder_prune}
                </Button>
                <Button size="sm" variant="danger" onClick={() => setConfirming("system_prune")}>
                  {DOCKER_ACTION_LABEL.system_prune}
                </Button>
              </div>
              {error && <p className="mt-2 text-[12px] text-danger">{error}</p>}
            </>
          )}
        </div>
      </Card>

      <Dialog
        open={confirming !== null}
        onClose={() => !running && setConfirming(null)}
        closeOnBackdrop={!running}
        className="max-w-[460px]"
        label="Confirm Docker cleanup"
      >
        {confirming && (
          <>
            <div className="px-5 pt-5 pb-4">
              <div className="flex items-start gap-3">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-danger-soft text-danger">
                  <AlertTriangle size={18} />
                </div>
                <div className="min-w-0 flex-1">
                  <h2 className="text-[15px] font-semibold tracking-tight">
                    {DOCKER_ACTION_LABEL[confirming]}
                  </h2>
                  <p className="mt-0.5 text-[12.5px] text-fg-muted">
                    {DOCKER_ACTION_DESCRIPTION[confirming]}
                  </p>
                </div>
              </div>
              <p className="mt-3 text-[12px] text-fg-muted">Prune will run:</p>
              <pre className="mt-1 overflow-x-auto rounded-md border border-line bg-surface-2 px-3 py-2 font-mono text-[12px]">
                {DOCKER_ACTION_COMMAND[confirming]}
              </pre>
              <p className="mt-2 text-[12px] text-danger">
                Docker does this itself, so it cannot be undone from Prune.
              </p>
            </div>
            <div className="flex items-center justify-end gap-2 border-t border-line bg-surface-2/60 px-5 py-3">
              <Button variant="ghost" onClick={() => setConfirming(null)} disabled={running}>
                Cancel
              </Button>
              <Button variant="danger" loading={running} onClick={() => void run(confirming)}>
                Run it
              </Button>
            </div>
          </>
        )}
      </Dialog>

      <Dialog
        open={result !== null}
        onClose={() => setResult(null)}
        className="max-w-[460px]"
        label="Docker cleanup result"
      >
        {result && (
          <>
            <div className="px-5 pt-5 pb-4">
              <h2 className="text-[15px] font-semibold tracking-tight">
                Docker reclaimed {formatBytes(result.reclaimedBytes)}
              </h2>
              <p className="mt-0.5 text-[12.5px] text-fg-muted">
                From <span className="font-mono text-[11.5px]">{result.command}</span>. Docker's own
                output:
              </p>
              <pre className="mt-2 max-h-[200px] overflow-auto rounded-md border border-line bg-surface-2 px-3 py-2 font-mono text-[11.5px] whitespace-pre-wrap">
                {result.output || "(no output)"}
              </pre>
            </div>
            <div className="flex justify-end border-t border-line bg-surface-2/60 px-5 py-3">
              <Button variant="primary" onClick={() => setResult(null)} autoFocus>
                Done
              </Button>
            </div>
          </>
        )}
      </Dialog>
    </>
  );
}
