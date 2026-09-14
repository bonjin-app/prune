import { beforeEach, describe, expect, it, vi } from "vitest";
import { selectedTargets, sumBytes, useScan } from "./scan";
import { useApps } from "./apps";
import { useDisk } from "./disk";
import type { Backend } from "@/lib/tauri";
import { resetStores } from "@/test/reset";
import * as factory from "@/test/factories";

vi.mock("@/lib/tauri", async () => {
  const actual = await vi.importActual<typeof import("@/lib/tauri")>("@/lib/tauri");
  return {
    ...actual,
    backend: {
      cleanerStartScan: vi.fn(),
      cleanerCancelScan: vi.fn(),
      cleanerPreview: vi.fn(),
      cleanerExecute: vi.fn(),
      cleanerListProviders: vi.fn(),
    },
  };
});

const { backend } = await import("@/lib/tauri");
/** Every backend method, typed as a vitest mock so `mockResolvedValueOnce` is available. */
type MockedBackend = { [K in keyof Backend]: ReturnType<typeof vi.fn> };
const mocked = backend as unknown as MockedBackend;

/** A session with one safe cache, one low-risk build folder and one protected item. */
function threeTargets() {
  const safe = factory.target({ id: "safe", risk: "safe", sizeBytes: 1_000 });
  const low = factory.target({
    id: "low",
    risk: "low",
    sizeBytes: 2_000,
    providerId: "project_artifacts",
  });
  const locked = factory.target({ id: "locked", risk: "protected", sizeBytes: 4_000 });
  return {
    safe,
    low,
    locked,
    session: factory.session([
      factory.result("user_cache", "application_cache", [safe, locked]),
      factory.result("project_artifacts", "developer_files", [low]),
    ]),
  };
}

beforeEach(() => {
  resetStores();
  factory.resetIds();
  vi.clearAllMocks();
});

describe("selection", () => {
  it("toggles a single target", () => {
    const { session } = threeTargets();
    useScan.setState({ session, scanId: session.id });
    useScan.getState().toggleTarget("safe");
    expect([...useScan.getState().selected]).toEqual(["safe"]);
    useScan.getState().toggleTarget("safe");
    expect(useScan.getState().selected.size).toBe(0);
  });

  it("sets and clears a group of targets", () => {
    const { session } = threeTargets();
    useScan.setState({ session, scanId: session.id });
    useScan.getState().setTargets(["safe", "low"], true);
    expect(useScan.getState().selected.size).toBe(2);
    useScan.getState().setTargets(["safe"], false);
    expect([...useScan.getState().selected]).toEqual(["low"]);
    useScan.getState().clearSelection();
    expect(useScan.getState().selected.size).toBe(0);
  });

  it("recommends only safe targets, never low risk or protected", () => {
    const { session } = threeTargets();
    useScan.setState({ session, scanId: session.id });
    useScan.getState().selectRecommended();
    expect([...useScan.getState().selected]).toEqual(["safe"]);
  });

  it("limits recommendations to the requested categories", () => {
    const safeDev = factory.target({ id: "devsafe", risk: "safe", providerId: "npm_cache" });
    const safeCache = factory.target({ id: "cachesafe", risk: "safe" });
    const session = factory.session([
      factory.result("user_cache", "application_cache", [safeCache]),
      factory.result("npm_cache", "developer_files", [safeDev]),
    ]);
    useScan.setState({ session, scanId: session.id });
    useScan.getState().selectRecommended(["developer_files"]);
    expect([...useScan.getState().selected]).toEqual(["devsafe"]);
  });

  it("resolves selected ids back to targets and sums them", () => {
    const { session } = threeTargets();
    useScan.setState({ session, scanId: session.id, selected: new Set(["safe", "low"]) });
    const targets = selectedTargets(useScan.getState());
    expect(targets.map((t) => t.id)).toEqual(["safe", "low"]);
    expect(sumBytes(targets)).toBe(3_000);
  });
});

describe("scan lifecycle", () => {
  it("keeps only selections that still exist when a scan completes", () => {
    const { safe, session } = threeTargets();
    useScan.setState({ scanId: "scan1", selected: new Set(["safe", "gone"]), scanning: true });
    useScan.getState().onScanCompleted(session);
    expect([...useScan.getState().selected]).toEqual(["safe"]);
    expect(useScan.getState().scanning).toBe(false);
    expect(useScan.getState().session?.results[0]?.targets[0]?.id).toBe(safe.id);
  });

  it("ignores events from a scan that is no longer the current one", () => {
    useScan.setState({ scanId: "current", scanning: true });
    useScan.getState().onScanCompleted(factory.session([], "stale"));
    expect(useScan.getState().scanning).toBe(true);
    expect(useScan.getState().session).toBeNull();

    useScan.getState().onScanProgress({
      scanId: "stale",
      providerId: "p",
      scannedFiles: 1,
      discoveredBytes: 1,
      providersDone: 0,
      providersTotal: 1,
    });
    expect(Object.keys(useScan.getState().progress)).toHaveLength(0);
  });

  it("records progress per provider for the current scan", () => {
    useScan.setState({ scanId: "current" });
    useScan.getState().onScanProgress({
      scanId: "current",
      providerId: "npm_cache",
      scannedFiles: 5,
      discoveredBytes: 500,
      providersDone: 0,
      providersTotal: 2,
    });
    expect(useScan.getState().progress.npm_cache?.scannedFiles).toBe(5);
  });

  it("reports a failure to start instead of getting stuck scanning", async () => {
    mocked.cleanerStartScan.mockRejectedValueOnce({ code: "io", message: "nope" });
    await useScan.getState().startScan();
    expect(useScan.getState().scanning).toBe(false);
    expect(useScan.getState().error).toContain("nope");
  });
});

describe("preview", () => {
  it("does nothing without a selection", async () => {
    useScan.setState({ scanId: "scan1", session: threeTargets().session });
    await useScan.getState().preview();
    expect(mocked.cleanerPreview).not.toHaveBeenCalled();
    expect(useScan.getState().plan).toBeNull();
  });

  it("sends the selected ids and the chosen mode", async () => {
    const { session } = threeTargets();
    const expected = factory.plan([]);
    mocked.cleanerPreview.mockResolvedValueOnce(expected);
    useScan.setState({
      session,
      scanId: session.id,
      selected: new Set(["safe", "low"]),
      deleteMode: "permanent",
    });
    await useScan.getState().preview();
    expect(mocked.cleanerPreview).toHaveBeenCalledWith("scan1", ["safe", "low"], "permanent");
    expect(useScan.getState().plan).toBe(expected);
    expect(useScan.getState().previewing).toBe(false);
  });
});

describe("execute", () => {
  it("removes cleaned targets from the session and recomputes totals", async () => {
    const { safe, low, locked, session } = threeTargets();
    const p = factory.plan([safe, low]);
    mocked.cleanerExecute.mockResolvedValueOnce(
      factory.cleanupResult({ removedBytes: 3_000, removedTargets: 2 }),
    );
    useScan.setState({
      session,
      scanId: session.id,
      plan: p,
      selected: new Set(["safe", "low", "locked"]),
    });

    await useScan.getState().execute();

    const after = useScan.getState();
    expect(mocked.cleanerExecute).toHaveBeenCalledWith("plan1");
    expect(after.executing).toBe(false);
    expect(after.plan).toBeNull();
    expect(after.result?.removedBytes).toBe(3_000);
    // Only the protected item, which was never in the plan, survives.
    const remaining = after.session?.results.flatMap((r) => r.targets.map((t) => t.id));
    expect(remaining).toEqual([locked.id]);
    expect(after.session?.totalBytes).toBe(4_000);
    expect([...after.selected]).toEqual(["locked"]);
  });

  it("keeps targets that failed to be removed", async () => {
    const { safe, low, session } = threeTargets();
    mocked.cleanerExecute.mockResolvedValueOnce(
      factory.cleanupResult({
        status: "partial",
        failed: [{ targetId: "low", path: low.path, error: "in use" }],
      }),
    );
    useScan.setState({ session, scanId: session.id, plan: factory.plan([safe, low]) });

    await useScan.getState().execute();

    const remaining = useScan
      .getState()
      .session?.results.flatMap((r) => r.targets.map((t) => t.id));
    expect(remaining).toContain("low");
    expect(remaining).not.toContain("safe");
  });

  it("surfaces an execution error and stops the spinner", async () => {
    const { safe, session } = threeTargets();
    mocked.cleanerExecute.mockRejectedValueOnce({ code: "io", message: "disk busy" });
    useScan.setState({ session, scanId: session.id, plan: factory.plan([safe]) });

    await useScan.getState().execute();

    expect(useScan.getState().executing).toBe(false);
    expect(useScan.getState().error).toContain("disk busy");
    // Nothing was removed locally, because nothing was removed on disk.
    expect(useScan.getState().session?.results[0]?.targets).toHaveLength(2);
  });

  it("tells the disk view to forget what was removed from its own scan", async () => {
    const file = factory.target({ id: "big", providerId: "large_files", kind: "file" });
    const diskSession = factory.session(
      [factory.result("large_files", "large_files", [file])],
      "disk1",
    );
    const forget = vi.fn();
    useDisk.setState({ scanId: "disk1", forgetRemoved: forget });
    mocked.cleanerExecute.mockResolvedValueOnce(factory.cleanupResult());
    useScan.setState({
      session: diskSession,
      scanId: "disk1",
      plan: factory.plan([file], { scanId: "disk1" }),
    });

    await useScan.getState().execute();

    expect(forget).toHaveBeenCalledWith(["big"]);
  });

  it("tells the uninstaller to forget what was removed from an app detail", async () => {
    const item = factory.target({ id: "caches", providerId: "uninstaller" });
    const forget = vi.fn();
    useApps.setState({ forgetRemoved: forget });
    mocked.cleanerExecute.mockResolvedValueOnce(factory.cleanupResult());
    useScan.setState({
      session: factory.session([factory.result("uninstaller", "applications", [item])], "app:a1"),
      scanId: "app:a1",
      plan: factory.plan([item], { scanId: "app:a1" }),
    });

    await useScan.getState().execute();

    expect(forget).toHaveBeenCalledWith(["caches"]);
  });
});

describe("delete mode", () => {
  it("remembers the choice", () => {
    useScan.getState().setDeleteMode("permanent");
    expect(useScan.getState().deleteMode).toBe("permanent");
    expect(localStorage.getItem("prune.deleteMode")).toBe("permanent");
  });
});
