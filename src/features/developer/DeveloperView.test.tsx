import { act } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { backend } from "@/lib/tauri";
import { useSettings } from "@/stores/settings";
import { useUi } from "@/stores/ui";
import { DeveloperView } from "./DeveloperView";

vi.mock("@/lib/tauri", () => ({
  backend: {
    settingsGet: vi.fn(),
    settingsSetProjectRoots: vi.fn(),
    cleanerStartScan: vi.fn(),
    cleanerExecute: vi.fn(),
  },
  errorMessage: (error: unknown) => String(error),
}));
vi.mock("../cleaner/CleanerWorkspace", () => ({
  CleanerWorkspace: () => <div>Scan workspace</div>,
}));
vi.mock("../cleaner/DeleteModeToggle", () => ({ DeleteModeToggle: () => null }));
vi.mock("./DockerCard", () => ({ DockerCard: () => null }));

beforeEach(() => {
  vi.resetAllMocks();
  useUi.setState({ view: "disk", projectRootDraft: null });
  useSettings.setState({
    view: {
      settings: { projectRoots: [] },
      effectiveProjectRoots: ["/home/u/code"],
      projectRootsConfigured: false,
      homeDir: "/home/u",
    },
    loading: false,
    saving: false,
    error: null,
  });
});

it("starts collapsed and allows opening the native folder panel", async () => {
  expect(useUi.getInitialState().projectRootDraft).toBeNull();
  render(<DeveloperView />);
  const summary = screen.getByText("Project scan folders");
  expect(summary.closest("details")).not.toHaveAttribute("open");
  await userEvent.click(summary);
  expect(summary.closest("details")).toHaveAttribute("open");
});

it("hands off a Storage path, opens its draft, and lets the user collapse it", async () => {
  render(<DeveloperView />);
  act(() => useUi.getState().openProjectReview("/home/u/new"));
  expect(useUi.getState()).toMatchObject({ view: "developer", projectRootDraft: "/home/u/new" });
  const summary = screen.getByText("Project scan folders");
  expect(summary.closest("details")).toHaveAttribute("open");
  expect(screen.getByRole("textbox", { name: "Project scan folder to add" })).toHaveValue(
    "/home/u/new",
  );
  expect(screen.getByRole("status")).toBeVisible();
  await userEvent.click(summary);
  await waitFor(() => expect(summary.closest("details")).not.toHaveAttribute("open"));
  act(() => useSettings.setState({ error: "unrelated update" }));
  expect(summary.closest("details")).not.toHaveAttribute("open");
  act(() => useUi.getState().openProjectReview("/home/u/next"));
  expect(screen.getByText("Project scan folders").closest("details")).toHaveAttribute("open");
  expect(screen.getByRole("textbox")).toHaveValue("/home/u/next");
  expect(backend.settingsSetProjectRoots).not.toHaveBeenCalled();
  expect(backend.cleanerStartScan).not.toHaveBeenCalled();
  expect(backend.cleanerExecute).not.toHaveBeenCalled();
});
