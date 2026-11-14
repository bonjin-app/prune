import { fireEvent, render, screen } from "@testing-library/react";
import { act } from "react";
import { beforeEach, expect, it, vi } from "vitest";
import { resetStores } from "@/test/reset";
import { useSystem } from "@/stores/system";
import { useSettings } from "@/stores/settings";
import { useDisk } from "@/stores/disk";
import { useUi } from "@/stores/ui";
import { DiskView } from "./DiskView";
import { useScan } from "@/stores/scan";
import * as factory from "@/test/factories";

vi.mock("@/lib/tauri", async () => {
  const actual = await vi.importActual<typeof import("@/lib/tauri")>("@/lib/tauri");
  return {
    ...actual,
    backend: { ...actual.backend, cleanerPreview: vi.fn(), cleanerExecute: vi.fn() },
  };
});
const { backend } = await import("@/lib/tauri");

vi.mock("@/stores/system", async () => ({
  ...(await vi.importActual<typeof import("@/stores/system")>("@/stores/system")),
  startSnapshotPolling: () => () => {},
}));

beforeEach(() => {
  resetStores();
  vi.clearAllMocks();
  useUi.setState({ permissionNoticeDismissed: false });
  useSettings.setState({
    view: {
      settings: { projectRoots: [] },
      effectiveProjectRoots: [],
      projectRootsConfigured: true,
      homeDir: "/Users/u",
    },
  });
});

it("explains inaccessible locations on the storage screen", () => {
  useSystem.setState({
    permissions: {
      fullDiskAccess: "denied",
      blocked: ["Trash"],
      howToGrant: "Add Prune under Full Disk Access.",
    },
  });
  render(<DiskView />);
  expect(screen.getByText("Some locations are hidden from Prune")).toBeVisible();
  expect(screen.getByRole("button", { name: "Open Settings" })).toBeVisible();
});

it("explains OS access requests while a scan has not made progress", () => {
  useDisk.setState({ scanning: true, root: "/Users/u/Documents" });
  useSystem.setState({ permissions: { fullDiskAccess: "denied", blocked: ["Trash"] } });
  render(<DiskView />);
  expect(screen.getByText(/check for a macOS folder access request/)).toBeVisible();
});

it("acknowledges cancellation without claiming the OS has stopped", () => {
  useDisk.setState({ scanning: true, cancelling: true, root: "/Users/u/Documents" });
  render(<DiskView />);
  expect(screen.getByRole("button", { name: "Cancelling…" })).toBeDisabled();
  expect(screen.getByRole("status")).toHaveTextContent(/waiting for file access to return/i);
});

it("cannot install a late disk preview over a pending Cleaner cleanup", async () => {
  let resolvePreview!: (plan: ReturnType<typeof factory.plan>) => void;
  let resolveExecution!: (result: ReturnType<typeof factory.cleanupResult>) => void;
  vi.mocked(backend.cleanerPreview).mockReturnValueOnce(
    new Promise((resolve) => {
      resolvePreview = resolve;
    }),
  );
  vi.mocked(backend.cleanerExecute).mockReturnValueOnce(
    new Promise((resolve) => {
      resolveExecution = resolve;
    }),
  );
  useDisk.setState({
    scanId: "disk1",
    tab: "large",
    selected: new Set(["large"]),
    summary: {
      scanId: "disk1",
      root: "/home/u",
      status: "completed",
      totalBytes: 1_000_000_000,
      fileCount: 1,
      dirCount: 0,
      durationMs: 1,
      largeFileCount: 1,
      issueCount: 0,
      topExtensions: [],
      startedAt: new Date().toISOString(),
    },
    largeFiles: [factory.largeFile({ targetId: "large", name: "big.zip" })],
  });
  render(<DiskView />);
  fireEvent.click(screen.getByRole("button", { name: /Review/ }));
  const target = factory.target({ id: "cleaner" });
  const cleanerPlan = factory.plan([target]);
  act(() => useScan.setState({ scanId: "scan1", plan: cleanerPlan, previewing: false }));
  let execution!: Promise<void>;
  act(() => {
    execution = useScan.getState().execute();
  });
  await act(async () => {
    resolvePreview(
      factory.plan([factory.target({ id: "large" })], { id: "disk-plan", scanId: "disk1" }),
    );
  });
  expect(useScan.getState().plan).toBe(cleanerPlan);
  await act(async () => {
    resolveExecution(factory.cleanupResult());
    await execution;
  });
  expect(useScan.getState().executing).toBe(false);
  expect(useScan.getState().result?.operationId).toBe("op1");
});
