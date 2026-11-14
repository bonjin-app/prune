import { act } from "react";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { resetStores } from "@/test/reset";
import { useDisk } from "@/stores/disk";
import { useUi } from "@/stores/ui";
import type { DiskNode, DiskNodeView } from "@/types/models";
import { FolderExplorer } from "./FolderExplorer";

vi.mock("@/lib/tauri", async () => {
  const actual = await vi.importActual<typeof import("@/lib/tauri")>("@/lib/tauri");
  return { ...actual, backend: { ...actual.backend, diskGetNode: vi.fn(), fsReveal: vi.fn() } };
});
const { backend } = await import("@/lib/tauri");
const folder = (name: string, path = `/root/${name}`): DiskNode => ({
  name,
  path,
  sizeBytes: 100,
  fileCount: 2,
  dirCount: 0,
  childDirs: 0,
  ownFileBytes: 100,
});
function view(count = 2): DiskNodeView {
  return {
    node: { ...folder("root", "/root"), sizeBytes: count * 100, ownFileBytes: 0 },
    children: Array.from({ length: count }, (_, i) => folder(`folder${i}`)),
    breadcrumbs: [],
  };
}
/**
 * Controls found by their label, and folder rows found inside the folder list only. With a few
 * hundred rows and map tiles on screen, `getByRole("button", { name })` computes an accessible
 * name (and visibility) for every one of them on each call, which is what pushed these tests
 * past CI's time limit. These tests are about names and enabled state, not visibility.
 */
const button = (label: string | RegExp) => screen.getByLabelText(label);
const row = (name: RegExp) =>
  within(screen.getByLabelText("Folders")).getByRole("button", { name, hidden: true });
beforeEach(() => {
  resetStores();
  vi.clearAllMocks();
});

describe("FolderExplorer", () => {
  it("hands the current folder to project review without scanning or deleting", async () => {
    useDisk.setState({ node: view() });
    render(<FolderExplorer />);
    await userEvent.click(screen.getByRole("button", { name: "Find build artifacts here" }));
    expect(useUi.getState()).toMatchObject({ view: "developer", projectRootDraft: "/root" });
    expect(backend.diskGetNode).not.toHaveBeenCalled();
    expect(backend.fsReveal).not.toHaveBeenCalled();
  });
  it("shows incomplete folder totals and skipped-path evidence", async () => {
    useDisk.setState({
      node: {
        ...view(),
        partial: true,
        issueCount: 1,
        issues: [{ path: "/root/blocked", message: "Permission denied" }],
      },
    });
    render(<FolderExplorer />);
    expect(screen.getByText(/Partial folder totals/)).toBeVisible();
    await userEvent.click(screen.getByText("Show skipped paths"));
    expect(screen.getByText("/root/blocked")).toBeVisible();
    expect(screen.getByText("Permission denied")).toBeVisible();
  });
  it("distinguishes loading from a failed node and offers retry", async () => {
    useDisk.setState({ scanId: "scan", nodeLoading: true });
    render(<FolderExplorer />);
    expect(screen.getByText("Loading folders…")).toBeInTheDocument();
    act(() => useDisk.setState({ nodeLoading: false }));
    vi.mocked(backend.diskGetNode).mockResolvedValue(view());
    await userEvent.click(screen.getByRole("button", { name: "Retry loading folders" }));
    expect(backend.diskGetNode).toHaveBeenCalledWith("scan", undefined);
    expect(screen.getByRole("region", { name: "Folder explorer" })).toBeInTheDocument();
  });
  it("drills down from the map and goes back through breadcrumbs", async () => {
    useDisk.setState({ scanId: "scan", node: view() });
    vi.mocked(backend.diskGetNode).mockResolvedValue({
      node: folder("folder0"),
      children: [],
      breadcrumbs: [folder("root", "/root")],
    });
    render(<FolderExplorer />);
    await userEvent.click(screen.getByRole("button", { name: /^Explore folder0,/ }));
    expect(backend.diskGetNode).toHaveBeenLastCalledWith("scan", "/root/folder0");
    vi.mocked(backend.diskGetNode).mockResolvedValue(view());
    await userEvent.click(screen.getByRole("button", { name: "Go to parent folder" }));
    expect(backend.diskGetNode).toHaveBeenLastCalledWith("scan", "/root");
    expect(screen.getByRole("button", { name: "Go to parent folder" })).toBeDisabled();
  });
  it("keeps at most 100 rows on every page of 1001 folders and searches all children", async () => {
    const user = userEvent.setup();
    useDisk.setState({ node: view(1001) });
    render(<FolderExplorer />);
    // Count the page's list items rather than every button with a matching name. The map's
    // tiles are buttons too, and computing an accessible name and visibility for each of them on
    // every one of ~30 calls is what made this test run longer than CI allows. `hidden: true`
    // skips the per-element visibility walk; the list itself is checked to be visible below.
    const list = () => screen.getByRole("list", { name: "Folders" });
    const rows = () => within(list()).queryAllByRole("listitem", { hidden: true });
    const folder = (n: number) =>
      within(list()).queryByRole("button", { name: new RegExp(`^folder${n} `), hidden: true });
    const next = screen.getByRole("button", { name: "Next page of folders" });
    const previous = screen.getByRole("button", { name: "Previous page of folders" });
    expect(list()).toBeVisible();
    expect(rows()).toHaveLength(100);
    expect(previous).toBeDisabled();
    expect(screen.getByText(/Page 1 of 11/)).toHaveTextContent("1–100 of 1,001 folders");
    expect(folder(100)).not.toBeInTheDocument();
    next.focus();
    for (let page = 2; page <= 11; page++) {
      await user.keyboard("{Enter}");
      expect(rows()).toHaveLength(page === 11 ? 1 : 100);
      expect(next).toHaveFocus();
      expect(screen.getByText(new RegExp(`Page ${page} of 11`))).toBeInTheDocument();
    }
    expect(folder(1000)).toBeInTheDocument();
    expect(next).toBeDisabled();
    expect(screen.getByText(/Page 11 of 11/)).toHaveTextContent("1001–1001 of 1,001 folders");
    await user.click(previous);
    expect(previous).toHaveFocus();
    expect(rows()).toHaveLength(100);
    expect(folder(900)).toBeInTheDocument();
    const search = screen.getByRole("textbox", { name: "Filter folders" });
    await user.type(search, "folder1000");
    expect(rows()).toHaveLength(1);
    expect(folder(1000)).toBeInTheDocument();
    expect(screen.getByText(/Page 1 of 1/)).toHaveTextContent("1–1 of 1 folder");
    await user.clear(search);
    expect(rows()).toHaveLength(100);
    expect(folder(0)).toBeInTheDocument();
    expect(previous).toBeDisabled();
    await user.click(next);
    expect(folder(100)).toBeInTheDocument();
    await user.type(search, "missing");
    expect(rows()).toHaveLength(0);
    expect(screen.getByText("No folders match your search.")).toBeVisible();
    expect(screen.getByText(/Page 1 of 1/)).toHaveTextContent("0–0 of 0 folders");
    await user.click(screen.getByRole("button", { name: "Hide map" }));
    expect(screen.queryByLabelText("Folder sizes map")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Show map" }));
    expect(screen.getByLabelText("Folder sizes map")).toBeInTheDocument();
  }, 15000);
  it("clamps a shrinking folder page and resets search and page at a new path", async () => {
    const user = userEvent.setup();
    useDisk.setState({ node: view(301) });
    render(<FolderExplorer />);
    const next = button("Next page of folders");
    await user.click(next);
    await user.click(next);
    await user.click(next);
    act(() => useDisk.setState({ node: view(101) }));
    expect(screen.getByText(/Page 2 of 2/)).toHaveTextContent("101–101 of 101 folders");
    expect(row(/^folder100 /)).toBeInTheDocument();
    act(() => useDisk.setState({ node: view(301) }));
    expect(screen.getByText(/Page 2 of 4/)).toBeInTheDocument();
    await user.type(screen.getByRole("textbox", { name: "Filter folders" }), "folder");
    await user.click(next);
    act(() =>
      useDisk.setState({
        node: {
          ...view(201),
          node: folder("nested", "/root/nested"),
          breadcrumbs: [folder("root", "/root")],
        },
      }),
    );
    expect(screen.getByRole("textbox", { name: "Filter folders" })).toHaveValue("");
    expect(screen.getByText(/Page 1 of 3/)).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Folder breadcrumbs" })).toHaveTextContent(
      "root",
    );
    expect(button("Go to parent folder")).toBeEnabled();
  });
  it("disables navigation during a pending request without hiding the current folder", () => {
    useDisk.setState({
      node: {
        ...view(201),
        children: [{ ...folder("folder0"), sizeBytes: 10000 }, ...view(201).children.slice(1)],
        breadcrumbs: [folder("parent", "/")],
      },
    });
    render(<FolderExplorer />);
    act(() => button("Next page of folders").click());
    act(() => useDisk.setState({ nodeLoading: true }));
    expect(screen.getByRole("region", { name: "Folder explorer" })).toHaveAttribute(
      "aria-busy",
      "true",
    );
    expect(button(/^Explore folder0,/)).toBeDisabled();
    expect(button("Next page of folders")).toBeDisabled();
    expect(button("Previous page of folders")).toBeDisabled();
    expect(button("Go to parent folder")).toBeDisabled();
    expect(
      within(screen.getByLabelText("Folder breadcrumbs")).getByRole("button", {
        name: "parent",
        hidden: true,
      }),
    ).toBeDisabled();
    expect(row(/^folder100 /)).toBeDisabled();
  });
  it("reports file-manager errors without deleting anything", async () => {
    useDisk.setState({ node: view() });
    vi.mocked(backend.fsReveal).mockRejectedValue(new Error("Cannot reveal"));
    render(<FolderExplorer />);
    await userEvent.click(screen.getByRole("button", { name: "Show folder in file manager" }));
    expect(backend.fsReveal).toHaveBeenCalledWith("/root");
    expect(useDisk.getState().error).toBe("Cannot reveal");
  });
});
