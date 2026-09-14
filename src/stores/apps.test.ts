import { beforeEach, describe, expect, it, vi } from "vitest";
import { useApps } from "./apps";
import { useStartup } from "./startup";
import type { Backend } from "@/lib/tauri";
import { resetStores } from "@/test/reset";
import * as factory from "@/test/factories";

vi.mock("@/lib/tauri", async () => {
  const actual = await vi.importActual<typeof import("@/lib/tauri")>("@/lib/tauri");
  return {
    ...actual,
    backend: {
      appsStartScan: vi.fn(),
      appsGetDetail: vi.fn(),
      appsRunUninstaller: vi.fn(),
      startupList: vi.fn(),
      startupSetEnabled: vi.fn(),
    },
  };
});

const { backend } = await import("@/lib/tauri");
/** Every backend method, typed as a vitest mock so `mockResolvedValueOnce` is available. */
type MockedBackend = { [K in keyof Backend]: ReturnType<typeof vi.fn> };
const mocked = backend as unknown as MockedBackend;

beforeEach(() => {
  resetStores();
  factory.resetIds();
  vi.clearAllMocks();
});

describe("uninstaller", () => {
  it("lists applications and then fills in sizes as they are measured", async () => {
    const apps = [factory.application({ id: "a" }), factory.application({ id: "b" })];
    mocked.appsStartScan.mockResolvedValueOnce(apps);

    await useApps.getState().load();
    expect(useApps.getState().apps).toHaveLength(2);
    expect(useApps.getState().measuring).toBe(true);

    useApps.getState().onProgress({
      scanId: "s",
      done: 1,
      total: 2,
      app: { ...apps[0]!, sizeBytes: 500 },
    });
    expect(useApps.getState().apps.find((a) => a.id === "a")?.sizeBytes).toBe(500);
    expect(useApps.getState().progress).toEqual({ done: 1, total: 2 });

    useApps.getState().onCompleted(apps.map((a) => ({ ...a, sizeBytes: 900 })));
    expect(useApps.getState().measuring).toBe(false);
    expect(useApps.getState().apps.every((a) => a.sizeBytes === 900)).toBe(true);
  });

  it("pre-selects everything removable but never a protected item", async () => {
    const app = factory.application({ id: "a" });
    mocked.appsGetDetail.mockResolvedValueOnce(
      factory.appDetail(app, [
        factory.appItem("application", "protected", 5_000),
        factory.appItem("caches", "safe", 3_000),
        factory.appItem("preferences", "low", 100),
      ]),
    );

    await useApps.getState().select("a");

    expect([...useApps.getState().selectedItems].sort()).toEqual(["caches", "preferences"]);
    expect(useApps.getState().detailLoading).toBe(false);
  });

  it("drops the detail when the selection is cleared", async () => {
    await useApps.getState().select(null);
    expect(mocked.appsGetDetail).not.toHaveBeenCalled();
    expect(useApps.getState().detail).toBeNull();
  });

  it("discards a detail that arrives after the user moved on", async () => {
    const app = factory.application({ id: "a" });
    let release: (() => void) | undefined;
    mocked.appsGetDetail.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          release = () =>
            resolve(factory.appDetail(app, [factory.appItem("caches", "safe", 1_000)]));
        }),
    );
    const pending = useApps.getState().select("a");
    // The user clicks a different application before the first one finishes measuring.
    useApps.setState({ selectedId: "b" });
    release?.();
    await pending;
    expect(useApps.getState().detail).toBeNull();
  });

  it("removes leftovers from the detail without dropping the application", async () => {
    const app = factory.application({ id: "a" });
    useApps.setState({
      apps: [app],
      selectedId: "a",
      detail: factory.appDetail(app, [
        factory.appItem("application", "medium", 5_000),
        factory.appItem("caches", "safe", 3_000),
      ]),
      selectedItems: new Set(["application", "caches"]),
    });

    await useApps.getState().forgetRemoved(["caches"]);

    const s = useApps.getState();
    expect(s.detail?.items.map((i) => i.kind)).toEqual(["application"]);
    expect(s.detail?.totalBytes).toBe(5_000);
    expect(s.detail?.leftoverBytes).toBe(0);
    expect([...s.selectedItems]).toEqual(["application"]);
    expect(s.apps).toHaveLength(1);
  });

  it("removes the application from the list once its bundle is gone", async () => {
    const app = factory.application({ id: "a" });
    useApps.setState({
      apps: [app, factory.application({ id: "b" })],
      selectedId: "a",
      detail: factory.appDetail(app, [factory.appItem("application", "medium", 5_000)]),
    });

    await useApps.getState().forgetRemoved(["application"]);

    const s = useApps.getState();
    expect(s.apps.map((a) => a.id)).toEqual(["b"]);
    expect(s.selectedId).toBeNull();
    expect(s.detail).toBeNull();
  });

  it("reports a failure to list applications", async () => {
    mocked.appsStartScan.mockRejectedValueOnce({ code: "io", message: "denied" });
    await useApps.getState().load();
    expect(useApps.getState().loading).toBe(false);
    expect(useApps.getState().error).toContain("denied");
  });
});

describe("startup", () => {
  it("loads items", async () => {
    mocked.startupList.mockResolvedValueOnce([factory.startupItem({ id: "x" })]);
    await useStartup.getState().load();
    expect(useStartup.getState().items).toHaveLength(1);
    expect(useStartup.getState().loading).toBe(false);
  });

  it("applies the new state and clears the pending flag", async () => {
    const item = factory.startupItem({ id: "x", enabled: true });
    mocked.startupSetEnabled.mockResolvedValueOnce({ ...item, enabled: false });
    useStartup.setState({ items: [item] });

    await useStartup.getState().setEnabled("x", false);

    expect(useStartup.getState().items[0]?.enabled).toBe(false);
    expect(useStartup.getState().pending.size).toBe(0);
  });

  it("leaves the item unchanged when the OS refuses", async () => {
    const item = factory.startupItem({ id: "x", enabled: true });
    mocked.startupSetEnabled.mockRejectedValueOnce({
      code: "other",
      message: "requires administrator rights",
    });
    useStartup.setState({ items: [item] });

    await useStartup.getState().setEnabled("x", false);

    expect(useStartup.getState().items[0]?.enabled).toBe(true);
    expect(useStartup.getState().error).toContain("administrator");
    expect(useStartup.getState().pending.size).toBe(0);
  });
});
