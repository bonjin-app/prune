import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { backend } from "@/lib/tauri";
import { useSettings } from "@/stores/settings";
import type { SettingsView } from "@/types/models";
import { ProtectedPaths } from "./ProtectedPaths";

vi.mock("@/lib/tauri", () => ({
  backend: {
    settingsGet: vi.fn(),
    settingsAddProtectedPath: vi.fn(),
    settingsRemoveProtectedPath: vi.fn(),
    settingsPickProtectedPath: vi.fn(),
  },
  errorMessage: (error: unknown) => (error as { message?: string }).message ?? String(error),
}));

const base: SettingsView = {
  settings: { projectRoots: ["/home/u/code"], protectedPaths: ["/home/u/code/site/dist"] },
  effectiveProjectRoots: ["/home/u/code"],
  projectRootsConfigured: true,
  homeDir: "/home/u",
  problem: null,
};

beforeEach(() => {
  vi.resetAllMocks();
  useSettings.setState({ view: base, loading: false, saving: false, error: null });
  // The backend answers with what is on disk, which the page may not have seen yet.
  let onDisk = [...base.settings.protectedPaths];
  const answer = () => ({ ...base, settings: { ...base.settings, protectedPaths: [...onDisk] } });
  vi.mocked(backend.settingsGet).mockImplementation(async () => answer());
  vi.mocked(backend.settingsAddProtectedPath).mockImplementation(async (path) => {
    if (!onDisk.includes(path)) onDisk.push(path);
    return answer();
  });
  vi.mocked(backend.settingsRemoveProtectedPath).mockImplementation(async (path) => {
    onDisk = onDisk.filter((p) => p !== path);
    return answer();
  });
});

it("lists what is protected and adds to it, keeping what was there", async () => {
  render(<ProtectedPaths />);
  const list = screen.getByRole("list", { name: "Protected paths" });
  expect(within(list).getByText("~/code/site/dist")).toBeInTheDocument();

  await userEvent.type(
    screen.getByRole("textbox", { name: "Folder or file to protect" }),
    "/home/u/Documents/keys",
  );
  await userEvent.click(screen.getByRole("button", { name: /Protect/ }));
  expect(backend.settingsAddProtectedPath).toHaveBeenCalledWith("/home/u/Documents/keys");
  await waitFor(() =>
    expect(screen.getByRole("textbox", { name: "Folder or file to protect" })).toHaveValue(""),
  );
  expect(within(list).getByText("~/Documents/keys")).toBeInTheDocument();
});

it("reads ~ as the home folder", async () => {
  render(<ProtectedPaths />);
  await userEvent.type(
    screen.getByRole("textbox", { name: "Folder or file to protect" }),
    "~/Documents/keys",
  );
  await userEvent.click(screen.getByRole("button", { name: /Protect/ }));
  expect(backend.settingsAddProtectedPath).toHaveBeenCalledWith("/home/u/Documents/keys");
});

it("stops protecting one path, sending only that path", async () => {
  render(<ProtectedPaths />);
  await userEvent.click(
    await screen.findByRole("button", { name: "Stop protecting /home/u/code/site/dist" }),
  );
  expect(backend.settingsRemoveProtectedPath).toHaveBeenCalledWith("/home/u/code/site/dist");
  await waitFor(() => expect(screen.getByText(/Nothing is protected yet/)).toBeInTheDocument());
});

it("shows what the command line protected since the page last looked", async () => {
  vi.mocked(backend.settingsGet).mockResolvedValue({
    ...base,
    settings: { ...base.settings, protectedPaths: ["/home/u/code/site/dist", "/home/u/from-cli"] },
  });
  render(<ProtectedPaths />);
  expect(await screen.findByText("~/from-cli")).toBeInTheDocument();
});

it("keeps the draft and shows why when a path is refused", async () => {
  vi.mocked(backend.settingsAddProtectedPath).mockRejectedValue({
    code: "invalid_path",
    message: "/home/u/typo: folder does not exist",
  });
  render(<ProtectedPaths />);
  const input = screen.getByRole("textbox", { name: "Folder or file to protect" });
  await userEvent.type(input, "/home/u/typo");
  await userEvent.click(screen.getByRole("button", { name: /Protect/ }));
  expect(await screen.findByText("/home/u/typo: folder does not exist")).toBeInTheDocument();
  expect(input).toHaveValue("/home/u/typo");
});

it("says plainly when the settings could not be read", () => {
  useSettings.setState({
    view: {
      ...base,
      settings: { projectRoots: [], protectedPaths: [] },
      problem: "The settings file could not be read. Nothing will be removed until it is fixed.",
    },
  });
  render(<ProtectedPaths />);
  expect(screen.getByRole("alert")).toHaveTextContent("Nothing will be removed");
});

it("explains an empty list instead of showing nothing", () => {
  useSettings.setState({
    view: { ...base, settings: { ...base.settings, protectedPaths: [] } },
  });
  render(<ProtectedPaths />);
  expect(screen.getByText(/Nothing is protected yet/)).toBeInTheDocument();
});

it("protects what is chosen in the system panel", async () => {
  vi.mocked(backend.settingsPickProtectedPath).mockResolvedValue("/home/u/Documents/keys");
  render(<ProtectedPaths />);
  await userEvent.click(screen.getByRole("button", { name: /Choose a folder/ }));
  expect(backend.settingsPickProtectedPath).toHaveBeenCalledWith("folder");
  expect(backend.settingsAddProtectedPath).toHaveBeenCalledWith("/home/u/Documents/keys");
});

it("saves nothing when the panel is cancelled", async () => {
  vi.mocked(backend.settingsPickProtectedPath).mockResolvedValue(null);
  render(<ProtectedPaths />);
  await userEvent.click(screen.getByRole("button", { name: /Choose a file/ }));
  expect(backend.settingsPickProtectedPath).toHaveBeenCalledWith("file");
  expect(backend.settingsAddProtectedPath).not.toHaveBeenCalled();
});
