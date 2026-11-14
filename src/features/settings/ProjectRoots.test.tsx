import { act } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { backend } from "@/lib/tauri";
import { useSettings } from "@/stores/settings";
import { ProjectRoots } from "./ProjectRoots";

vi.mock("@/lib/tauri", () => ({
  backend: { settingsGet: vi.fn(), settingsSetProjectRoots: vi.fn() },
  errorMessage: (error: unknown) => String(error),
}));

beforeEach(() => {
  vi.resetAllMocks();
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

it("prefills and resets the draft for each new suggestion without saving", async () => {
  const { rerender } = render(<ProjectRoots suggestedPath="/home/u/first" />);
  const input = screen.getByRole("textbox", { name: "Project scan folder to add" });
  expect(input).toHaveValue("/home/u/first");
  await userEvent.clear(input);
  await userEvent.type(input, "/edited");
  rerender(<ProjectRoots suggestedPath="/home/u/second" />);
  expect(screen.getByRole("textbox")).toHaveValue("/home/u/second");
  expect(screen.getByRole("status")).toHaveTextContent("/home/u/second");
  expect(screen.getByText(/only updates the configured scan scope/)).toHaveTextContent(
    "Source files stay untouched",
  );
  expect(backend.settingsSetProjectRoots).not.toHaveBeenCalled();
});

it("adds the reviewed folder alongside guessed roots and clears the draft after saving", async () => {
  vi.mocked(backend.settingsSetProjectRoots).mockImplementation(async (roots) => ({
    settings: { projectRoots: roots },
    effectiveProjectRoots: roots,
    projectRootsConfigured: true,
    homeDir: "/home/u",
  }));
  render(<ProjectRoots suggestedPath="/home/u/new" />);
  await userEvent.click(screen.getByRole("button", { name: "Add scan folder" }));
  expect(backend.settingsSetProjectRoots).toHaveBeenCalledWith(["/home/u/code", "/home/u/new"]);
  await waitFor(() => expect(screen.getByRole("textbox")).toHaveValue(""));
});

it("blocks both button and Enter submission while saving", async () => {
  render(<ProjectRoots suggestedPath="/home/u/new" />);
  act(() => useSettings.setState({ saving: true }));
  expect(screen.getByRole("button", { name: /Add scan folder/ })).toBeDisabled();
  await userEvent.type(screen.getByRole("textbox"), "{Enter}");
  expect(backend.settingsSetProjectRoots).not.toHaveBeenCalled();
  expect(screen.getByRole("textbox")).toHaveValue("/home/u/new");
});
