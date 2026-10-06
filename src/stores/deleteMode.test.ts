import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/**
 * "Delete permanently" holds for the run it was chosen in and no longer.
 *
 * It used to be written to localStorage, so choosing it once made every later cleanup one that
 * cannot be undone. Each test loads the store afresh, because the starting mode is decided when
 * the module is first evaluated.
 */
const KEY = "prune.deleteMode";

async function freshStore() {
  vi.resetModules();
  return (await import("./scan")).useScan;
}

describe("delete mode", () => {
  beforeEach(() => localStorage.clear());
  afterEach(() => localStorage.clear());

  it("starts out recoverable", async () => {
    const useScan = await freshStore();
    expect(useScan.getState().deleteMode).toBe("trash");
  });

  it("does not remember a choice of permanent deletion", async () => {
    const first = await freshStore();
    first.getState().setDeleteMode("permanent");
    expect(first.getState().deleteMode).toBe("permanent");

    // The next run.
    const second = await freshStore();
    expect(second.getState().deleteMode).toBe("trash");
  });

  it("writes nothing down when the mode is changed", async () => {
    const useScan = await freshStore();
    useScan.getState().setDeleteMode("permanent");
    expect(localStorage.getItem(KEY)).toBeNull();
  });

  it("does not carry someone who had already chosen permanent into this run", async () => {
    // What an earlier version left behind.
    localStorage.setItem(KEY, "permanent");
    const useScan = await freshStore();
    expect(useScan.getState().deleteMode).toBe("trash");
    expect(localStorage.getItem(KEY)).toBeNull();
  });

  it("still lets the mode be changed back and forth within a run", async () => {
    const useScan = await freshStore();
    useScan.getState().setDeleteMode("permanent");
    useScan.getState().setDeleteMode("trash");
    expect(useScan.getState().deleteMode).toBe("trash");
  });
});
