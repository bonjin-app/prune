import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UninstallerView } from "./UninstallerView";
import * as factory from "@/test/factories";
import { useApps } from "@/stores/apps";
import { useScan } from "@/stores/scan";

vi.mock("@/lib/tauri", async () => {
  const actual = await vi.importActual<typeof import("@/lib/tauri")>("@/lib/tauri");
  return {
    ...actual,
    backend: { ...actual.backend, cleanerPreview: vi.fn().mockResolvedValue(null) },
  };
});
const { backend } = await import("@/lib/tauri");

/**
 * Uninstalling removes more than the application: it removes the data the application left
 * behind. These hold the two things that keep that from going wrong — that something marked
 * protected can never be chosen by any route, and that data another installed copy is still
 * using is named as such rather than quietly offered.
 */
const app = factory.application({ id: "app1", name: "Thing" });

function showDetail(items: Parameters<typeof factory.appDetail>[1], sharedWith?: string[]) {
  const detail = factory.appDetail(app, items, sharedWith ? { sharedWith } : {});
  act(() =>
    useApps.setState({
      apps: [app],
      selectedId: app.id,
      detail,
      detailLoading: false,
      selectedItems: new Set<string>(),
      loading: false,
      measuring: false,
      error: null,
    }),
  );
  return render(<UninstallerView />);
}

describe("UninstallerView", () => {
  beforeEach(() => {
    act(() => {
      useApps.setState({ selectedItems: new Set<string>(), error: null });
      useScan.setState({ deleteMode: "trash", previewing: false, plan: null });
    });
    vi.mocked(backend.cleanerPreview).mockClear();
  });

  it("refuses a protected item by row click and by checkbox", async () => {
    const user = userEvent.setup();
    showDetail([
      factory.appItem("application", "medium", 500),
      factory.appItem("caches", "protected", 100),
    ]);

    const box = screen.getByRole("checkbox", { name: "caches" });
    expect(box).toBeDisabled();

    // Clicking the row is the other way in, and it must be shut too.
    await user.click(screen.getByText("caches"));
    expect([...useApps.getState().selectedItems]).toEqual([]);
  });

  it("selects only what may be removed when selecting everything", async () => {
    const user = userEvent.setup();
    showDetail([
      factory.appItem("application", "medium", 500),
      factory.appItem("caches", "safe", 100),
      factory.appItem("preferences", "protected", 10),
    ]);

    await user.click(screen.getByRole("checkbox", { name: "Select all" }));
    expect([...useApps.getState().selectedItems].sort()).toEqual(["application", "caches"]);
  });

  it("names the other copy that is still using this data", () => {
    // Two installed copies share one bundle identifier, so the leftovers belong to neither.
    showDetail(
      [
        factory.appItem("application", "medium", 500),
        factory.appItem("application_support", "protected", 900),
      ],
      ["/Applications/Thing-1.2-backup.app"],
    );

    expect(screen.getByText(/Another copy of this application is installed/)).toBeInTheDocument();
    expect(screen.getByText(/Thing-1.2-backup.app/)).toBeInTheDocument();
    expect(screen.getByText(/Remove the other copy first/)).toBeInTheDocument();
    // The bundle the user asked about can still go; its data cannot.
    expect(screen.getByRole("checkbox", { name: "application" })).toBeEnabled();
    expect(screen.getByRole("checkbox", { name: "application_support" })).toBeDisabled();
  });

  it("says nothing about other copies when there are none", () => {
    showDetail([factory.appItem("application", "medium", 500)]);
    expect(screen.queryByText(/Another copy of this application/)).toBeNull();
  });

  it("says whether it is uninstalling or only clearing up after", async () => {
    const user = userEvent.setup();
    showDetail([
      factory.appItem("application", "medium", 500),
      factory.appItem("caches", "safe", 100),
    ]);

    await user.click(screen.getByRole("checkbox", { name: "caches" }));
    expect(screen.getByRole("button", { name: /Remove leftovers/ })).toBeInTheDocument();

    await user.click(screen.getByRole("checkbox", { name: "application" }));
    expect(screen.getByRole("button", { name: /Uninstall/ })).toBeInTheDocument();
  });

  it("asks to remove only what was chosen", async () => {
    const user = userEvent.setup();
    showDetail([
      factory.appItem("application", "medium", 500),
      factory.appItem("caches", "safe", 100),
    ]);

    await user.click(screen.getByRole("checkbox", { name: "caches" }));
    await user.click(screen.getByRole("button", { name: /Remove leftovers/ }));

    expect(backend.cleanerPreview).toHaveBeenCalledWith("app:app1", ["caches"], "trash");
  });

  it("does nothing when nothing is chosen", async () => {
    showDetail([factory.appItem("application", "medium", 500)]);
    expect(screen.getByRole("button", { name: /Uninstall|Remove leftovers/ })).toBeDisabled();
    expect(backend.cleanerPreview).not.toHaveBeenCalled();
  });
});
