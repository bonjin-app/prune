import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDisk } from "./disk";
import type { Backend } from "@/lib/tauri";
import { resetStores } from "@/test/reset";
import * as factory from "@/test/factories";
import type { DiskNodeView, DiskSummary, LargeFile } from "@/types/models";

vi.mock("@/lib/tauri", async () => {
  const actual = await vi.importActual<typeof import("@/lib/tauri")>("@/lib/tauri");
  return {
    ...actual,
    backend: {
      diskStartScan: vi.fn(),
      diskCancelScan: vi.fn(),
      diskGetSummary: vi.fn(),
      diskGetNode: vi.fn(),
      diskLargeFiles: vi.fn(),
    },
  };
});

const { backend } = await import("@/lib/tauri");
/** Every backend method, typed as a vitest mock so `mockResolvedValueOnce` is available. */
type MockedBackend = { [K in keyof Backend]: ReturnType<typeof vi.fn> };
const mocked = backend as unknown as MockedBackend;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function summary(overrides: Partial<DiskSummary> = {}): DiskSummary {
  return {
    scanId: "disk1",
    root: "/home/u",
    status: "completed",
    totalBytes: 10_000,
    fileCount: 5,
    dirCount: 2,
    durationMs: 100,
    largeFileCount: 2,
    issueCount: 0,
    topExtensions: [],
    startedAt: new Date().toISOString(),
    ...overrides,
  };
}

function node(path = "/home/u", sizeBytes = 10_000): DiskNodeView {
  return {
    node: {
      name: path.split("/").pop() ?? path,
      path,
      sizeBytes,
      fileCount: 5,
      dirCount: 2,
      childDirs: 1,
      ownFileBytes: 0,
    },
    children: [],
    breadcrumbs: [],
  };
}

beforeEach(() => {
  resetStores();
  factory.resetIds();
  vi.resetAllMocks();
  mocked.diskLargeFiles.mockResolvedValue([]);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("completion reconciliation", () => {
  it("recovers the completed analysis when native events are missed", async () => {
    vi.useFakeTimers();
    mocked.diskStartScan.mockResolvedValueOnce("disk1");
    mocked.diskGetSummary.mockResolvedValueOnce(summary());
    mocked.diskGetNode.mockResolvedValueOnce(node());
    await useDisk.getState().startScan("/home/u");
    await vi.advanceTimersByTimeAsync(1_000);
    expect(useDisk.getState()).toMatchObject({
      scanning: false,
      summary: { scanId: "disk1", totalBytes: 10_000 },
      node: node(),
    });
  });

  it("keeps checking a running scan until its result is available", async () => {
    vi.useFakeTimers();
    mocked.diskStartScan.mockResolvedValueOnce("disk1");
    mocked.diskGetSummary
      .mockRejectedValueOnce({ code: "scan_running", message: "analysis not finished" })
      .mockResolvedValueOnce(summary());
    mocked.diskGetNode.mockResolvedValueOnce(node());
    await useDisk.getState().startScan("/home/u");
    await vi.advanceTimersByTimeAsync(1_000);
    expect(useDisk.getState()).toMatchObject({ scanning: true, error: null });
    await vi.advanceTimersByTimeAsync(1_000);
    expect(useDisk.getState()).toMatchObject({
      scanning: false,
      summary: { scanId: "disk1", totalBytes: 10_000 },
    });
  });

  it("does not let a late summary overwrite a newer scan", async () => {
    vi.useFakeTimers();
    const oldSummary = deferred<DiskSummary>();
    mocked.diskStartScan.mockResolvedValueOnce("disk1").mockResolvedValueOnce("disk2");
    mocked.diskGetSummary.mockReturnValueOnce(oldSummary.promise);
    await useDisk.getState().startScan("/old");
    await vi.advanceTimersByTimeAsync(1_000);
    expect(mocked.diskGetSummary).toHaveBeenCalledWith("disk1");
    resetStores();
    await useDisk.getState().startScan("/new");
    oldSummary.resolve(summary({ root: "/old" }));
    await vi.advanceTimersByTimeAsync(0);
    expect(useDisk.getState()).toMatchObject({ scanId: "disk2", scanning: true, summary: null });
  });

  it("does not reset folder navigation on duplicate completion", async () => {
    const completed = summary();
    useDisk.setState({ scanId: "disk1", summary: completed, node: node("/home/u/child") });
    mocked.diskGetNode.mockResolvedValueOnce(node());
    await useDisk.getState().onCompleted(completed);
    expect(useDisk.getState().node?.node.path).toBe("/home/u/child");
  });

  it("reports a terminal reconciliation error instead of spinning forever", async () => {
    vi.useFakeTimers();
    mocked.diskStartScan.mockResolvedValueOnce("disk1");
    mocked.diskGetSummary.mockRejectedValueOnce({ code: "unknown_scan", message: "scan expired" });
    await useDisk.getState().startScan("/home/u");
    await vi.advanceTimersByTimeAsync(1_000);
    expect(useDisk.getState()).toMatchObject({
      scanning: false,
      error: "scan expired (unknown_scan)",
    });
  });
});

describe("scan lifecycle", () => {
  it("queues cancellation during launch and only cancels the returned scan", async () => {
    const launch = deferred<string>();
    mocked.diskStartScan.mockReturnValueOnce(launch.promise);
    useDisk.setState({ scanId: "old" });
    const starting = useDisk.getState().startScan("/new");
    await useDisk.getState().cancelScan();
    expect(useDisk.getState()).toMatchObject({ scanning: true, cancelling: true, scanId: null });
    expect(mocked.diskCancelScan).not.toHaveBeenCalled();
    launch.resolve("new");
    await starting;
    expect(mocked.diskCancelScan).toHaveBeenCalledExactlyOnceWith("new");
    expect(useDisk.getState()).toMatchObject({ scanning: true, cancelling: true });
  });

  it("keeps cancellation visible until completion and ignores repeat requests", async () => {
    const cancelling = deferred<void>();
    useDisk.setState({ scanId: "disk1", scanning: true });
    mocked.diskCancelScan.mockReturnValueOnce(cancelling.promise);
    const request = useDisk.getState().cancelScan();
    await useDisk.getState().cancelScan();
    expect(useDisk.getState()).toMatchObject({ scanning: true, cancelling: true });
    expect(mocked.diskCancelScan).toHaveBeenCalledTimes(1);
    cancelling.resolve();
    await request;
    expect(useDisk.getState()).toMatchObject({ scanning: true, cancelling: true });
    mocked.diskGetNode.mockResolvedValueOnce(node());
    await useDisk.getState().onCompleted(summary({ status: "cancelled" }));
    expect(useDisk.getState()).toMatchObject({ scanning: false, cancelling: false });
  });

  it("lets a failed cancellation be retried without ending the running scan", async () => {
    useDisk.setState({ scanId: "disk1", scanning: true });
    mocked.diskCancelScan.mockRejectedValueOnce(new Error("cancel refused"));
    await useDisk.getState().cancelScan();
    expect(useDisk.getState()).toMatchObject({
      scanning: true,
      cancelling: false,
      error: "cancel refused",
    });
  });

  it("does not expose a late cancellation error on a newer scan", async () => {
    const cancelling = deferred<void>();
    useDisk.setState({ scanId: "disk1", scanning: true });
    mocked.diskCancelScan.mockReturnValueOnce(cancelling.promise);
    const request = useDisk.getState().cancelScan();
    resetStores();
    mocked.diskStartScan.mockResolvedValueOnce("disk2");
    await useDisk.getState().startScan("/new");
    cancelling.reject(new Error("obsolete cancel"));
    await request;
    expect(useDisk.getState()).toMatchObject({
      scanId: "disk2",
      scanning: true,
      cancelling: false,
      error: null,
    });
  });

  it("replays completion that arrives before the launch ID, ignoring unrelated IDs", async () => {
    const launch = deferred<string>();
    mocked.diskStartScan.mockReturnValueOnce(launch.promise);
    mocked.diskGetNode.mockResolvedValueOnce(node("/new"));
    const starting = useDisk.getState().startScan("/new");
    await useDisk.getState().onCompleted(summary({ scanId: "old", root: "/old" }));
    await useDisk.getState().onCompleted(summary({ scanId: "new", root: "/new" }));
    launch.resolve("new");
    await starting;
    expect(useDisk.getState()).toMatchObject({
      scanId: "new",
      scanning: false,
      root: "/new",
      summary: { scanId: "new" },
    });
    expect(mocked.diskGetNode).toHaveBeenCalledWith("new", undefined);
  });
  it.each(["success", "error"])(
    "invalidates previous requests and events while a new scan launches (%s)",
    async (outcome) => {
      const launch = deferred<string>();
      const oldNode = deferred<DiskNodeView>();
      const oldFiles = deferred<LargeFile[]>();
      mocked.diskStartScan.mockReturnValueOnce(launch.promise);
      mocked.diskGetNode.mockReturnValueOnce(oldNode.promise);
      mocked.diskLargeFiles.mockReturnValueOnce(oldFiles.promise);
      useDisk.setState({ scanId: "disk1", node: node(), summary: summary() });
      const navigation = useDisk.getState().openNode("/old");
      const filter = useDisk.getState().setMinBytes(5_000_000_000);

      const scanning = useDisk.getState().startScan("/new");

      expect(useDisk.getState()).toMatchObject({
        scanId: null,
        scanning: true,
        node: null,
        nodeLoading: false,
        summary: null,
        largeFiles: [],
      });
      useDisk.getState().onProgress({ scanId: "disk1", files: 1, bytes: 1, dirs: 1 });
      await useDisk.getState().onCompleted(summary());
      await useDisk.getState().openNode("/old");
      await useDisk.getState().refreshLargeFiles();
      expect(mocked.diskGetNode).toHaveBeenCalledTimes(1);
      expect(mocked.diskLargeFiles).toHaveBeenCalledTimes(1);

      if (outcome === "success") {
        oldNode.resolve(node("/old"));
        oldFiles.resolve([factory.largeFile()]);
      } else {
        oldNode.reject(new Error("obsolete node"));
        oldFiles.reject(new Error("obsolete files"));
      }
      await Promise.all([navigation, filter]);
      expect(useDisk.getState()).toMatchObject({
        scanning: true,
        progress: null,
        summary: null,
        node: null,
        nodeLoading: false,
        largeFiles: [],
        error: null,
      });
      launch.resolve("disk2");
      await scanning;
      expect(useDisk.getState().scanId).toBe("disk2");
    },
  );

  it.each(["success", "error"])(
    "ignores an obsolete scan launch after a reset and newer launch (%s)",
    async (outcome) => {
      const oldLaunch = deferred<string>();
      mocked.diskStartScan.mockReturnValueOnce(oldLaunch.promise).mockResolvedValueOnce("disk2");
      const oldScan = useDisk.getState().startScan("/old");
      resetStores();
      await useDisk.getState().startScan("/new");

      if (outcome === "success") oldLaunch.resolve("disk1");
      else oldLaunch.reject(new Error("obsolete launch"));
      await oldScan;

      expect(useDisk.getState()).toMatchObject({ scanId: "disk2", scanning: true, error: null });
    },
  );

  it("resets folder loading between tests", () => {
    useDisk.setState({ nodeLoading: true });
    resetStores();
    expect(useDisk.getState().nodeLoading).toBe(false);
  });

  it("clears previous results when a new scan starts", async () => {
    mocked.diskStartScan.mockResolvedValueOnce("disk2");
    useDisk.setState({
      summary: summary(),
      largeFiles: [factory.largeFile()],
      selected: new Set(["x"]),
      root: "/home/u",
    });

    await useDisk.getState().startScan();

    expect(mocked.diskStartScan).toHaveBeenCalledWith("/home/u");
    const s = useDisk.getState();
    expect(s.scanId).toBe("disk2");
    expect(s.scanning).toBe(true);
    expect(s.summary).toBeNull();
    expect(s.largeFiles).toEqual([]);
    expect(s.selected.size).toBe(0);
  });

  it("refuses to start a second scan while one is running", async () => {
    useDisk.setState({ scanning: true });
    await useDisk.getState().startScan();
    expect(mocked.diskStartScan).not.toHaveBeenCalled();
  });

  it("loads the tree and the large files when a scan completes", async () => {
    const files = [factory.largeFile({ targetId: "a" }), factory.largeFile({ targetId: "b" })];
    mocked.diskGetNode.mockResolvedValueOnce(node());
    mocked.diskLargeFiles.mockResolvedValueOnce(files);
    useDisk.setState({ scanId: "disk1", scanning: true });

    await useDisk.getState().onCompleted(summary());

    const s = useDisk.getState();
    expect(s.scanning).toBe(false);
    expect(s.root).toBe("/home/u");
    expect(s.node?.node.sizeBytes).toBe(10_000);
    expect(s.largeFiles).toHaveLength(2);
    expect(mocked.diskLargeFiles).toHaveBeenCalledWith("disk1", 1_000_000_000, 500);
  });

  it("ignores a completion from an older scan", async () => {
    useDisk.setState({ scanId: "current", scanning: true });
    await useDisk.getState().onCompleted(summary({ scanId: "stale" }));
    expect(useDisk.getState().scanning).toBe(true);
    expect(mocked.diskGetNode).not.toHaveBeenCalled();
  });

  it("ignores progress from an older scan", () => {
    useDisk.setState({ scanId: "current" });
    useDisk.getState().onProgress({ scanId: "stale", files: 1, bytes: 1, dirs: 1 });
    expect(useDisk.getState().progress).toBeNull();
  });
});

describe("folder navigation", () => {
  it("keeps the newest folder when requests finish out of order, even for the same path", async () => {
    const older = deferred<DiskNodeView>();
    const newer = deferred<DiskNodeView>();
    mocked.diskGetNode.mockReturnValueOnce(older.promise).mockReturnValueOnce(newer.promise);
    useDisk.setState({ scanId: "disk1", node: node("/previous") });

    const first = useDisk.getState().openNode("/folder");
    const second = useDisk.getState().openNode("/folder");
    expect(useDisk.getState().nodeLoading).toBe(true);
    newer.resolve(node("/folder", 20_000));
    await second;
    expect(useDisk.getState().nodeLoading).toBe(false);

    older.resolve(node("/folder", 10_000));
    await first;
    expect(useDisk.getState().node?.node.sizeBytes).toBe(20_000);
    expect(mocked.diskGetNode).toHaveBeenNthCalledWith(2, "disk1", "/folder");
  });

  it.each(["success", "error"])(
    "keeps loading the latest folder when an obsolete request settles (%s)",
    async (outcome) => {
      const older = deferred<DiskNodeView>();
      const newer = deferred<DiskNodeView>();
      mocked.diskGetNode.mockReturnValueOnce(older.promise).mockReturnValueOnce(newer.promise);
      useDisk.setState({ scanId: "disk1", node: node("/previous") });
      const first = useDisk.getState().openNode("/old");
      const second = useDisk.getState().openNode("/new");

      if (outcome === "success") older.resolve(node("/old"));
      else older.reject(new Error("obsolete folder"));
      await first;
      expect(useDisk.getState()).toMatchObject({ nodeLoading: true, error: null });
      expect(useDisk.getState().node?.node.path).toBe("/previous");

      newer.resolve(node("/new"));
      await second;
      expect(useDisk.getState().node?.node.path).toBe("/new");
      expect(useDisk.getState().nodeLoading).toBe(false);
    },
  );

  it("reports the latest folder error and clears loading without replacing the displayed node", async () => {
    const request = deferred<DiskNodeView>();
    mocked.diskGetNode.mockReturnValueOnce(request.promise);
    useDisk.setState({ scanId: "disk1", node: node("/previous") });
    const navigation = useDisk.getState().openNode("/missing");
    expect(useDisk.getState().nodeLoading).toBe(true);

    request.reject(new Error("missing folder"));
    await navigation;

    expect(useDisk.getState().error).toContain("missing folder");
    expect(useDisk.getState().nodeLoading).toBe(false);
    expect(useDisk.getState().node?.node.path).toBe("/previous");
  });

  it.each(["success", "error"])(
    "ignores a previous scan's node after the new scan completes (%s)",
    async (outcome) => {
      const older = deferred<DiskNodeView>();
      mocked.diskGetNode.mockReturnValueOnce(older.promise).mockResolvedValueOnce(node("/new"));
      mocked.diskLargeFiles.mockResolvedValueOnce([]);
      mocked.diskStartScan.mockResolvedValueOnce("disk2");
      useDisk.setState({ scanId: "disk1" });
      const first = useDisk.getState().openNode("/old");
      await useDisk.getState().startScan("/new");
      await useDisk.getState().onCompleted(summary({ scanId: "disk2", root: "/new" }));

      if (outcome === "success") older.resolve(node("/old"));
      else older.reject(new Error("obsolete scan node"));
      await first;

      expect(useDisk.getState().node?.node.path).toBe("/new");
      expect(useDisk.getState()).toMatchObject({ nodeLoading: false, error: null });
    },
  );
});

describe("large file filter", () => {
  it.each(["success", "error"])(
    "keeps the latest threshold's results and selections when an older request settles (%s)",
    async (outcome) => {
      const older = deferred<LargeFile[]>();
      const newer = deferred<LargeFile[]>();
      const files = [factory.largeFile({ targetId: "latest" })];
      mocked.diskLargeFiles.mockReturnValueOnce(older.promise).mockReturnValueOnce(newer.promise);
      useDisk.setState({ scanId: "disk1", selected: new Set(["latest", "old"]) });
      const first = useDisk.getState().setMinBytes(100_000_000);
      const second = useDisk.getState().setMinBytes(5_000_000_000);
      newer.resolve(files);
      await second;

      if (outcome === "success") older.resolve([factory.largeFile({ targetId: "old" })]);
      else older.reject(new Error("obsolete threshold"));
      await first;

      expect(useDisk.getState().minBytes).toBe(5_000_000_000);
      expect(useDisk.getState().largeFiles).toEqual(files);
      expect([...useDisk.getState().selected]).toEqual(["latest"]);
      expect(useDisk.getState().error).toBeNull();
      expect(mocked.diskLargeFiles).toHaveBeenNthCalledWith(2, "disk1", 5_000_000_000, 500);
    },
  );

  it("orders repeated requests even when the threshold returns to its original value", async () => {
    const firstRequest = deferred<LargeFile[]>();
    const secondRequest = deferred<LargeFile[]>();
    const thirdRequest = deferred<LargeFile[]>();
    mocked.diskLargeFiles
      .mockReturnValueOnce(firstRequest.promise)
      .mockReturnValueOnce(secondRequest.promise)
      .mockReturnValueOnce(thirdRequest.promise);
    useDisk.setState({ scanId: "disk1" });
    const first = useDisk.getState().setMinBytes(1_000_000_000);
    const second = useDisk.getState().setMinBytes(5_000_000_000);
    const third = useDisk.getState().setMinBytes(1_000_000_000);
    const files = [factory.largeFile({ targetId: "latest" })];
    thirdRequest.resolve(files);
    await third;
    firstRequest.resolve([factory.largeFile({ targetId: "old" })]);
    secondRequest.resolve([]);
    await Promise.all([first, second]);
    expect(useDisk.getState().largeFiles).toEqual(files);
  });

  it.each(["success", "error"])(
    "ignores a previous scan's filter after the new scan completes (%s)",
    async (outcome) => {
      const older = deferred<LargeFile[]>();
      const files = [factory.largeFile({ targetId: "latest" })];
      mocked.diskLargeFiles.mockReturnValueOnce(older.promise).mockResolvedValueOnce(files);
      mocked.diskGetNode.mockResolvedValueOnce(node("/new"));
      mocked.diskStartScan.mockResolvedValueOnce("disk2");
      useDisk.setState({ scanId: "disk1" });
      const first = useDisk.getState().refreshLargeFiles();
      await useDisk.getState().startScan("/new");
      await useDisk.getState().onCompleted(summary({ scanId: "disk2", root: "/new" }));
      useDisk.getState().toggle("latest");

      if (outcome === "success") older.resolve([factory.largeFile({ targetId: "old" })]);
      else older.reject(new Error("obsolete scan files"));
      await first;

      expect(useDisk.getState().largeFiles).toEqual(files);
      expect([...useDisk.getState().selected]).toEqual(["latest"]);
      expect(useDisk.getState().error).toBeNull();
    },
  );

  it("reports a current filter error", async () => {
    const request = deferred<LargeFile[]>();
    mocked.diskLargeFiles.mockReturnValueOnce(request.promise);
    useDisk.setState({ scanId: "disk1" });
    const filter = useDisk.getState().setMinBytes(5_000_000_000);
    request.reject(new Error("filter failed"));
    await filter;
    expect(useDisk.getState().error).toContain("filter failed");
  });

  it("re-queries when the threshold changes and drops selections that fall out", async () => {
    mocked.diskLargeFiles.mockResolvedValueOnce([factory.largeFile({ targetId: "big" })]);
    useDisk.setState({
      scanId: "disk1",
      largeFiles: [
        factory.largeFile({ targetId: "big" }),
        factory.largeFile({ targetId: "small" }),
      ],
      selected: new Set(["big", "small"]),
    });

    await useDisk.getState().setMinBytes(5_000_000_000);

    expect(mocked.diskLargeFiles).toHaveBeenCalledWith("disk1", 5_000_000_000, 500);
    expect(useDisk.getState().minBytes).toBe(5_000_000_000);
    // "small" is no longer listed, so it must not stay selected and get removed by accident.
    expect([...useDisk.getState().selected]).toEqual(["big"]);
  });
});

describe("after a cleanup", () => {
  it("refreshes the intended destination and discards its pre-cleanup navigation response", async () => {
    const navigation = deferred<DiskNodeView>();
    const refresh = deferred<DiskNodeView>();
    mocked.diskGetSummary.mockResolvedValueOnce(summary({ totalBytes: 4_000 }));
    mocked.diskGetNode.mockReturnValueOnce(navigation.promise).mockReturnValueOnce(refresh.promise);
    useDisk.setState({ scanId: "disk1", node: node("/old") });
    const opening = useDisk.getState().openNode("/new");
    const cleanup = useDisk.getState().forgetRemoved([]);

    refresh.resolve(node("/new", 4_000));
    await cleanup;
    expect(mocked.diskGetNode).toHaveBeenLastCalledWith("disk1", "/new");
    navigation.resolve(node("/new"));
    await opening;
    expect(useDisk.getState().node?.node.path).toBe("/new");
    expect(useDisk.getState().node?.node.sizeBytes).toBe(4_000);
    expect(useDisk.getState().nodeLoading).toBe(false);
  });

  it("does not restore the previous folder over newer navigation", async () => {
    const refresh = deferred<DiskNodeView>();
    mocked.diskGetSummary.mockResolvedValueOnce(summary({ totalBytes: 4_000 }));
    mocked.diskGetNode.mockReturnValueOnce(refresh.promise).mockResolvedValueOnce(node("/new"));
    useDisk.setState({ scanId: "disk1", node: node("/old") });
    const cleanup = useDisk.getState().forgetRemoved([]);
    await useDisk.getState().openNode("/new");
    refresh.resolve(node("/old", 4_000));
    await cleanup;
    expect(useDisk.getState().node?.node.path).toBe("/new");
    expect(useDisk.getState().nodeLoading).toBe(false);
    expect(useDisk.getState().summary?.totalBytes).toBe(4_000);
  });

  it("does not restore a previous scan's summary or node during a new launch", async () => {
    const refresh = deferred<DiskNodeView>();
    const launch = deferred<string>();
    mocked.diskGetSummary.mockResolvedValueOnce(summary());
    mocked.diskGetNode.mockReturnValueOnce(refresh.promise);
    mocked.diskStartScan.mockReturnValueOnce(launch.promise);
    useDisk.setState({ scanId: "disk1", node: node() });
    const cleanup = useDisk.getState().forgetRemoved([]);
    const scanning = useDisk.getState().startScan("/new");
    refresh.resolve(node());
    await cleanup;
    expect(useDisk.getState()).toMatchObject({ summary: null, node: null, nodeLoading: false });
    launch.resolve("disk2");
    await scanning;
  });

  it("drops the removed files and refreshes the tree", async () => {
    mocked.diskLargeFiles.mockResolvedValueOnce([factory.largeFile({ targetId: "kept" })]);
    mocked.diskGetSummary.mockResolvedValueOnce(summary({ totalBytes: 4_000 }));
    mocked.diskGetNode.mockResolvedValueOnce(node("/home/u", 4_000));
    useDisk.setState({
      scanId: "disk1",
      node: node("/home/u", 10_000),
      largeFiles: [
        factory.largeFile({ targetId: "gone" }),
        factory.largeFile({ targetId: "kept" }),
      ],
      selected: new Set(["gone", "kept"]),
      summary: summary(),
    });

    await useDisk.getState().forgetRemoved(["gone"]);

    const s = useDisk.getState();
    expect(s.largeFiles.map((f) => f.targetId)).toEqual(["kept"]);
    expect([...s.selected]).toEqual(["kept"]);
    expect(s.summary?.totalBytes).toBe(4_000);
    expect(s.node?.node.sizeBytes).toBe(4_000);
  });

  it("still updates the list when the backend can no longer be queried", async () => {
    mocked.diskGetSummary.mockRejectedValueOnce(new Error("gone"));
    mocked.diskGetNode.mockRejectedValueOnce(new Error("gone"));
    useDisk.setState({
      scanId: "disk1",
      largeFiles: [factory.largeFile({ targetId: "gone" })],
      selected: new Set(["gone"]),
    });

    await useDisk.getState().forgetRemoved(["gone"]);

    expect(useDisk.getState().largeFiles).toEqual([]);
    expect(useDisk.getState().error).toContain("gone");
  });

  it("never restores a deleted file from a pre-cleanup filter response", async () => {
    const stale = deferred<LargeFile[]>();
    mocked.diskLargeFiles
      .mockReturnValueOnce(stale.promise)
      .mockResolvedValueOnce([factory.largeFile({ targetId: "kept" })]);
    mocked.diskGetSummary.mockResolvedValueOnce(summary());
    mocked.diskGetNode.mockResolvedValueOnce(node());
    useDisk.setState({ scanId: "disk1", node: node() });
    const filtering = useDisk.getState().setMinBytes(100_000_000);
    await useDisk.getState().forgetRemoved(["gone"]);
    stale.resolve([factory.largeFile({ targetId: "gone" })]);
    await filtering;
    expect(useDisk.getState().largeFiles.map((file) => file.targetId)).toEqual(["kept"]);
  });
});

describe("errors", () => {
  it("reports a failed start and leaves the view usable", async () => {
    mocked.diskStartScan.mockRejectedValueOnce({
      code: "invalid_path",
      message: "not a directory",
    });
    await useDisk.getState().startScan("/nope");
    expect(useDisk.getState().scanning).toBe(false);
    expect(useDisk.getState().error).toContain("not a directory");
  });
});
