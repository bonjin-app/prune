import { describe, expect, it } from "vitest";
import { cleanerSources, trashName } from "./wording";

describe("trashName", () => {
  it("uses the word the platform uses", () => {
    expect(trashName("macos")).toBe("Trash");
    expect(trashName("windows")).toBe("Recycle Bin");
    expect(trashName("linux")).toBe("Trash");
  });

  it("falls back to the Mac word before the platform is known", () => {
    expect(trashName(undefined)).toBe("Trash");
  });
});

describe("cleanerSources", () => {
  it("names the Trash where it is scanned", () => {
    expect(cleanerSources("macos")).toBe(
      "application caches, logs, temporary files, browser caches, the Trash and old installers",
    );
  });

  it("does not promise a Recycle Bin scan Prune does not make", () => {
    const text = cleanerSources("windows");
    expect(text).not.toMatch(/Trash|Recycle/);
    expect(text).toBe(
      "application caches, logs, temporary files, browser caches and old installers",
    );
  });
});
