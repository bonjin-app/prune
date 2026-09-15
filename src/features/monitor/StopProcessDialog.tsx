import { useState } from "react";
import { AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { formatBytes, formatPercent } from "@/lib/format";
import { useSystem } from "@/stores/system";
import type { ProcessInfo } from "@/types/models";

/**
 * Confirms stopping a process.
 *
 * Quitting asks the program to exit so it can save; forcing does not. Both are offered here
 * rather than behind one button, because the difference is the difference between closing a
 * document and losing it.
 */
export function StopProcessDialog({
  process,
  onClose,
}: {
  process: ProcessInfo | null;
  onClose: () => void;
}) {
  const stopProcess = useSystem((s) => s.stopProcess);
  const [busy, setBusy] = useState<"ask" | "force" | null>(null);

  if (!process) return null;

  const stop = async (mode: "ask" | "force") => {
    setBusy(mode);
    const ok = await stopProcess(process.pid, mode);
    setBusy(null);
    if (ok) onClose();
  };

  return (
    <Dialog open onClose={onClose} className="max-w-[440px]" label={`Stop ${process.name}`}>
      <div className="px-5 pt-5 pb-4">
        <div className="flex items-start gap-3">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-warn-soft text-warn">
            <AlertTriangle size={18} />
          </div>
          <div className="min-w-0 flex-1">
            <h2 className="text-[15px] font-semibold tracking-tight">Stop {process.name}?</h2>
            <p className="mt-0.5 text-[12.5px] text-fg-muted">
              Anything it has not saved will be lost. Quitting lets it save first; forcing does not.
            </p>
            <div className="mt-2 text-[12px] text-fg-faint tnum">
              PID {process.pid} · {formatPercent(process.cpuPercent, 1)} CPU ·{" "}
              {formatBytes(process.memoryBytes)}
            </div>
          </div>
        </div>
      </div>
      <div className="flex items-center justify-end gap-2 border-t border-line bg-surface-2/60 px-5 py-3">
        <Button variant="ghost" onClick={onClose} disabled={busy !== null}>
          Cancel
        </Button>
        <Button
          variant="danger"
          onClick={() => void stop("force")}
          loading={busy === "force"}
          disabled={busy !== null}
        >
          Force stop
        </Button>
        <Button
          variant="primary"
          onClick={() => void stop("ask")}
          loading={busy === "ask"}
          disabled={busy !== null}
        >
          Quit
        </Button>
      </div>
    </Dialog>
  );
}
