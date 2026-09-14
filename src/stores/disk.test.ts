import { beforeEach, describe, expect, it, vi } from "vitest";
import { useDisk } from "./disk";
import type { Backend } from "@/lib/tauri";
import { resetStores } from "@/test/reset";
import * as factory from "@/test/factories";
import type { DiskNodeView, DiskSummary } from "@/types/models";

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
  vi.clearAllMocks();
});

describe("scan lifecycle", () => {
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

describe("large file filter", () => {
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
  it("drops the removed files and refreshes the tree", async () => {
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
    expect(useDisk.getState().error).toBeNull();
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
