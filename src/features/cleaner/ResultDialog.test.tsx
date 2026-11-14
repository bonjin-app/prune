import { fireEvent, render, screen } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ResultDialog } from "./ResultDialog";
import * as factory from "@/test/factories";
import { useScan } from "@/stores/scan";
import type { SystemSnapshot } from "@/types/models";

vi.mock("@/lib/tauri", async () => ({
  ...(await vi.importActual<typeof import("@/lib/tauri")>("@/lib/tauri")),
  backend: { systemGetSnapshot: vi.fn() },
}));
const { backend } = await import("@/lib/tauri");
function snapshot(availableBytes = 2_000_000_000): SystemSnapshot {
  return {
    cpu: { usagePercent: 1, perCorePercent: [1] },
    memory: {
      totalBytes: 100,
      usedBytes: 50,
      availableBytes: 50,
      swapTotalBytes: 0,
      swapUsedBytes: 0,
    },
    disks: [
      {
        name: "Data",
        mountPoint: "/",
        fileSystem: "apfs",
        totalBytes: 10_000_000_000,
        availableBytes,
        isRemovable: false,
        isPrimary: true,
      },
    ],
    network: {
      totalReceivedBytes: 0,
      totalTransmittedBytes: 0,
      downBytesPerSec: 0,
      upBytesPerSec: 0,
    },
    uptimeSeconds: 100,
    processCount: 10,
  };
}

/**
 * What the user is told after something was removed.
 *
 * By the time this appears the files are gone, so its only job is to be accurate: how much went
 * and where it went, what did not go and why, and the one case that reads like a failure but is
 * not — a cleanup that worked but could not be written to the history.
 */
function show(result: ReturnType<typeof factory.cleanupResult>) {
  act(() => useScan.setState({ result }));
  return render(<ResultDialog />);
}

describe("ResultDialog", () => {
  beforeEach(() => {
    act(() => useScan.setState({ result: null }));
    vi.mocked(backend.systemGetSnapshot).mockReset();
    vi.mocked(backend.systemGetSnapshot).mockReturnValue(new Promise(() => {}));
  });

  it("separates reported file sizes from current free space and explains Trash", async () => {
    vi.mocked(backend.systemGetSnapshot).mockResolvedValueOnce(snapshot());
    show(factory.cleanupResult({ mode: "trash", removedBytes: 4_000_000 }));
    expect(screen.getByText(/not disk space gained/)).toBeInTheDocument();
    expect(screen.getByText(/does not free space until/)).toBeInTheDocument();
    expect(await screen.findByText("2.00 GB")).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Available disk space now" })).toHaveTextContent(
      "Data",
    );
  });

  it("explains allocation differences without claiming a measured gain after permanent removal", () => {
    show(factory.cleanupResult({ mode: "permanent" }));
    expect(screen.getByText(/Shared, compressed or sparse files/)).toBeInTheDocument();
    expect(screen.queryByText(/does not free space until/)).toBeNull();
  });

  it("keeps cleanup results visible when capacity cannot be read", async () => {
    vi.mocked(backend.systemGetSnapshot).mockRejectedValueOnce(new Error("volume offline"));
    show(factory.cleanupResult());
    expect(await screen.findByText(/Could not read current disk space/)).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Pruned" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry disk space reading" })).toBeEnabled();
  });

  it("does not say files moved when cleanup failed", () => {
    show(factory.cleanupResult({ status: "failed", removedTargets: 0, removedBytes: 0 }));
    expect(screen.queryByText(/files in .*moved to/)).toBeNull();
    expect(screen.getByText(/No targets were confirmed removed/)).toBeInTheDocument();
  });

  it("does not headline success when zero targets were removed", () => {
    show(
      factory.cleanupResult({
        status: "success",
        removedTargets: 0,
        removedFiles: 0,
        removedBytes: 0,
      }),
    );
    expect(screen.queryByRole("heading", { name: "Pruned" })).toBeNull();
    expect(screen.getByRole("heading", { name: "Nothing removed" })).toBeInTheDocument();
  });

  it("retries capacity independently without repeating cleanup", async () => {
    vi.mocked(backend.systemGetSnapshot).mockRejectedValueOnce(new Error("offline"));
    vi.mocked(backend.systemGetSnapshot).mockResolvedValueOnce(snapshot(3_000_000_000));
    const result = factory.cleanupResult();
    show(result);
    fireEvent.click(await screen.findByRole("button", { name: "Retry disk space reading" }));
    expect(await screen.findByText("3.00 GB")).toBeInTheDocument();
    expect(useScan.getState().result).toBe(result);
    expect(backend.systemGetSnapshot).toHaveBeenCalledTimes(2);
  });

  it("ignores the old operation's late capacity response", async () => {
    let resolve!: (value: SystemSnapshot) => void;
    vi.mocked(backend.systemGetSnapshot).mockReturnValueOnce(
      new Promise((done) => {
        resolve = done;
      }),
    );
    vi.mocked(backend.systemGetSnapshot).mockResolvedValueOnce(snapshot(3_000_000_000));
    show(factory.cleanupResult({ operationId: "old" }));
    act(() => useScan.setState({ result: factory.cleanupResult({ operationId: "new" }) }));
    expect(await screen.findByText("3.00 GB")).toBeInTheDocument();
    await act(async () => resolve(snapshot(1_000_000_000)));
    expect(screen.queryByText("1.00 GB")).toBeNull();
    expect(screen.getByText("3.00 GB")).toBeInTheDocument();
  });

  it("does not interpret unavailable volume readings as zero available bytes", async () => {
    vi.mocked(backend.systemGetSnapshot).mockResolvedValueOnce({ ...snapshot(), disks: [] });
    show(factory.cleanupResult());
    expect(await screen.findByText(/Could not read current disk space/)).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Available disk space now" })).not.toHaveTextContent(
      "0 B",
    );
  });

  it("shows nothing until a cleanup has run", () => {
    const { container } = render(<ResultDialog />);
    expect(container).toBeEmptyDOMElement();
  });

  it("says where the things went, not just how many", () => {
    show(
      factory.cleanupResult({
        mode: "trash",
        removedTargets: 3,
        removedFiles: 120,
        removedBytes: 4_000_000,
      }),
    );
    expect(screen.getByText(/moved to the Trash/)).toBeInTheDocument();
    expect(screen.queryByText(/deleted permanently/)).toBeNull();
  });

  it("does not say Trash when nothing went there", () => {
    show(factory.cleanupResult({ mode: "permanent", removedTargets: 1 }));
    expect(screen.getByText(/deleted permanently/)).toBeInTheDocument();
    expect(screen.queryByText(/moved to the Trash/)).toBeNull();
  });

  it("reports a failed cleanup as a failure, not in the colour of success", () => {
    const { container } = show(
      factory.cleanupResult({ status: "failed", removedTargets: 0, removedBytes: 0 }),
    );
    expect(screen.getByRole("heading", { name: "Nothing removed" })).toBeInTheDocument();
    // The badge is the biggest thing on screen and is often the only thing read.
    expect(container.querySelector(".bg-accent-soft")).toBeNull();
    expect(container.querySelector(".bg-danger-soft")).not.toBeNull();
  });

  it("distinguishes a partial cleanup from both", () => {
    const { container } = show(factory.cleanupResult({ status: "partial" }));
    expect(screen.getByRole("heading", { name: "Partially pruned" })).toBeInTheDocument();
    expect(container.querySelector(".bg-warn-soft")).not.toBeNull();
    expect(container.querySelector(".bg-accent-soft")).toBeNull();
  });

  it("says a cleanup worked even when the history could not be written", () => {
    // The removal succeeded; only the record of it failed. Reporting that as a failed cleanup
    // would tell the user the opposite of what happened and invite them to run it again.
    show(
      factory.cleanupResult({
        status: "success",
        removedTargets: 2,
        logError: "permission denied",
      }),
    );
    expect(screen.getByRole("heading", { name: "Pruned" })).toBeInTheDocument();
    expect(screen.getByText(/could not be added to the operation log/)).toBeInTheDocument();
    expect(screen.getByText(/permission denied/)).toBeInTheDocument();
  });

  it("lists what could not be removed, with the reason and the path", () => {
    show(
      factory.cleanupResult({
        status: "partial",
        failed: [{ targetId: "a", path: "/home/u/Library/Caches/locked", error: "resource busy" }],
      }),
    );
    expect(screen.getByText("/home/u/Library/Caches/locked")).toBeInTheDocument();
    expect(screen.getByText("resource busy")).toBeInTheDocument();
  });

  it("keeps quiet about failures when there were none", () => {
    show(factory.cleanupResult({ status: "success", failed: [] }));
    expect(screen.queryByText(/could not be added to the operation log/)).toBeNull();
  });
});
