import { useEffect, useState } from "react";
import { Button } from "@/components/ui/Button";
import { formatBytes } from "@/lib/format";
import { backend } from "@/lib/tauri";
import { capacity } from "@/features/disk/storage";
import type { DiskStatus } from "@/types/models";

/** An OS observation after cleanup, never a calculation of space gained. */
export function CleanupCapacity() {
  const [attempt, setAttempt] = useState(0);
  const [disks, setDisks] = useState<DiskStatus[] | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let live = true;
    void backend.systemGetSnapshot().then(
      (snapshot) => {
        if (!live) return;
        setDisks(snapshot.disks);
        setFailed(snapshot.disks.length === 0);
      },
      () => {
        if (live) setFailed(true);
      },
    );
    return () => {
      live = false;
    };
  }, [attempt]);
  return (
    <section
      aria-label="Available disk space now"
      className="mt-4 rounded-md border border-line bg-surface-2/60 p-3 text-left"
    >
      <h3 className="text-[12px] font-medium">Available disk space now</h3>
      <p className="mt-1 text-[11.5px] text-fg-muted">
        Current OS reading, not a measured gain from this cleanup.
      </p>
      {failed ? (
        <div className="mt-2">
          <p role="status" className="text-[11.5px] text-fg-muted">
            Could not read current disk space. Cleanup result is unchanged.
          </p>
          <Button
            className="mt-2"
            onClick={() => {
              setFailed(false);
              setDisks(null);
              setAttempt((value) => value + 1);
            }}
          >
            Retry disk space reading
          </Button>
        </div>
      ) : disks ? (
        <ul className="mt-2 max-h-24 space-y-2 overflow-y-auto">
          {disks.map((disk) => (
            <li
              key={disk.mountPoint}
              className="flex items-center justify-between gap-3 text-[12px]"
            >
              <span className="min-w-0">
                <span className="block truncate">{disk.name}</span>
                <span className="block truncate font-mono text-[10.5px] text-fg-muted">
                  {disk.mountPoint}
                </span>
              </span>
              <span className="shrink-0 font-medium tnum">
                {formatBytes(capacity(disk).available)}
              </span>
            </li>
          ))}
        </ul>
      ) : (
        <p role="status" className="mt-2 text-[11.5px] text-fg-muted">
          Reading current disk space…
        </p>
      )}
    </section>
  );
}
