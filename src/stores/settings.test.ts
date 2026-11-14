import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SettingsView } from "@/types/models";
import { backend } from "@/lib/tauri";
import { useSettings } from "./settings";

vi.mock("@/lib/tauri", () => ({
  backend: { settingsGet: vi.fn(), settingsSetProjectRoots: vi.fn() },
  errorMessage: (error: unknown) => String(error),
}));

const guessed: SettingsView = {
  settings: { projectRoots: [] },
  effectiveProjectRoots: ["/home/u/Projects", "/home/u/code"],
  projectRootsConfigured: false,
  homeDir: "/home/u",
};

beforeEach(() => {
  vi.resetAllMocks();
  useSettings.setState({ view: guessed, saving: false, loading: false, error: null });
  vi.mocked(backend.settingsSetProjectRoots).mockImplementation(async (roots) => ({
    ...guessed,
    settings: { projectRoots: roots },
    effectiveProjectRoots: roots,
    projectRootsConfigured: true,
  }));
});

describe("project scan folders", () => {
  it("preserves guessed scopes when adding the first configured root", async () => {
    expect(await useSettings.getState().addProjectRoot(" /home/u/new ")).toBe(true);
    expect(backend.settingsSetProjectRoots).toHaveBeenCalledWith([
      ...guessed.effectiveProjectRoots,
      "/home/u/new",
    ]);
    expect(useSettings.getState().view?.projectRootsConfigured).toBe(true);
  });

  it("keeps the explicit list instead of adding guessed or normalized scopes", async () => {
    useSettings.setState({
      view: {
        ...guessed,
        projectRootsConfigured: true,
        settings: { projectRoots: ["/home/u/explicit"] },
      },
    });
    await useSettings.getState().addProjectRoot("/home/u/new");
    expect(backend.settingsSetProjectRoots).toHaveBeenCalledWith([
      "/home/u/explicit",
      "/home/u/new",
    ]);
  });

  it("does not save duplicates, empty drafts or unloaded settings", async () => {
    expect(await useSettings.getState().addProjectRoot("/home/u/code")).toBe(true);
    expect(await useSettings.getState().addProjectRoot(" ")).toBe(false);
    useSettings.setState({ view: null });
    expect(await useSettings.getState().addProjectRoot("/home/u/new")).toBe(false);
    expect(backend.settingsSetProjectRoots).not.toHaveBeenCalled();
  });

  it("refuses concurrent edits until a pending save completes", async () => {
    let resolve!: (view: SettingsView) => void;
    vi.mocked(backend.settingsSetProjectRoots).mockReturnValueOnce(
      new Promise((done) => {
        resolve = done;
      }),
    );
    const pending = useSettings.getState().addProjectRoot("/home/u/first");
    expect(useSettings.getState().saving).toBe(true);
    expect(await useSettings.getState().addProjectRoot("/home/u/second")).toBe(false);
    expect(await useSettings.getState().removeProjectRoot("/home/u/code")).toBe(false);
    expect(backend.settingsSetProjectRoots).toHaveBeenCalledTimes(1);
    resolve(guessed);
    await pending;
    expect(useSettings.getState().saving).toBe(false);
  });

  it("retains the previous scopes when saving fails", async () => {
    vi.mocked(backend.settingsSetProjectRoots).mockRejectedValueOnce(new Error("Save failed"));
    expect(await useSettings.getState().addProjectRoot("/home/u/new")).toBe(false);
    expect(useSettings.getState()).toMatchObject({
      view: guessed,
      saving: false,
      error: "Error: Save failed",
    });
  });
});
