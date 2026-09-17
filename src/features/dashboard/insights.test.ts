import { beforeEach, describe, expect, it } from "vitest";
import { deriveInsights } from "./insights";
import * as factory from "@/test/factories";
import type { CleanupTarget } from "@/types/models";

const NOW = new Date("2026-09-17T12:00:00Z").getTime();
const daysAgo = (days: number) => new Date(NOW - days * 86_400_000).toISOString();

function artifact(project: string, sizeBytes: number, lastActiveDays: number | null) {
  return factory.target({
    providerId: "project_artifacts",
    risk: "low",
    sizeBytes,
    group: {
      key: `/p/${project}`,
      label: project,
      ...(lastActiveDays === null ? {} : { lastActiveAt: daysAgo(lastActiveDays) }),
    },
  });
}

function sessionOf(targets: CleanupTarget[]) {
  return factory.session([factory.result("mixed", "developer_files", targets)]);
}

beforeEach(() => factory.resetIds());

describe("deriveInsights", () => {
  it("says nothing before there is a scan to talk about", () => {
    expect(deriveInsights(null, NOW)).toEqual([]);
    expect(deriveInsights(factory.session([]), NOW)).toEqual([]);
  });

  it("names dormant projects and what they are holding", () => {
    const insights = deriveInsights(
      sessionOf([
        artifact("abandoned", 8_000_000_000, 300),
        artifact("also-old", 1_000_000_000, 120),
        artifact("current", 20_000_000_000, 2),
      ]),
      NOW,
    );

    const stale = insights.find((i) => i.id === "stale-projects");
    expect(stale?.headline).toContain("2 dormant projects");
    expect(stale?.headline).toContain("9.00 GB");
    // The live project's 20 GB is not counted, however big it is.
    expect(stale?.bytes).toBe(9_000_000_000);
    expect(stale?.detail).toContain("abandoned");
    expect(stale?.view).toBe("developer");
  });

  it("does not call a project dormant when there is no date to judge by", () => {
    const insights = deriveInsights(sessionOf([artifact("no-repo", 9_000_000_000, null)]), NOW);
    expect(insights.find((i) => i.id === "stale-projects")).toBeUndefined();
  });

  it("uses the singular when only one project is dormant", () => {
    const insights = deriveInsights(sessionOf([artifact("lonely", 2_000_000_000, 400)]), NOW);
    const stale = insights.find((i) => i.id === "stale-projects");
    expect(stale?.headline).toContain("1 dormant project is");
    expect(stale?.detail).toContain("lonely");
  });

  it("adds up what carries no risk at all", () => {
    const insights = deriveInsights(
      factory.session([
        factory.result("user_cache", "application_cache", [
          factory.target({ risk: "safe", sizeBytes: 3_000_000_000 }),
          factory.target({ risk: "safe", sizeBytes: 1_000_000_000 }),
          factory.target({ risk: "medium", sizeBytes: 50_000_000_000 }),
        ]),
      ]),
      NOW,
    );

    const safe = insights.find((i) => i.id === "safe");
    expect(safe?.headline).toContain("4.00 GB");
    expect(safe?.safeOnly).toBe(true);
    expect(safe?.view).toBe("cleaner");
  });

  it("counts only what the section it opens will show", () => {
    // Safe items in both sections: sending the user to one while counting both would promise
    // more than the page they land on.
    const insights = deriveInsights(
      factory.session([
        factory.result("user_cache", "application_cache", [
          factory.target({ risk: "safe", sizeBytes: 2_000_000_000 }),
        ]),
        factory.result("npm_cache", "developer_files", [
          factory.target({ risk: "safe", sizeBytes: 9_000_000_000 }),
        ]),
      ]),
      NOW,
    );

    const safe = insights.find((i) => i.id === "safe");
    expect(safe?.view).toBe("developer");
    expect(safe?.bytes).toBe(9_000_000_000);
    expect(safe?.headline).toContain("9.00 GB");
  });

  it("calls out a single item that dwarfs everything else", () => {
    const insights = deriveInsights(
      sessionOf([
        factory.target({ risk: "low", sizeBytes: 42_000_000_000, label: "agent-status/target" }),
        factory.target({ risk: "low", sizeBytes: 100_000_000 }),
      ]),
      NOW,
    );

    const biggest = insights.find((i) => i.id === "biggest");
    expect(biggest?.headline).toContain("42.0 GB");
    expect(biggest?.filter).toBe("agent-status/target");
  });

  it("stays quiet about the biggest item when nothing stands out", () => {
    const even = Array.from({ length: 10 }, () =>
      factory.target({ risk: "low", sizeBytes: 2_000_000_000 }),
    );
    expect(deriveInsights(sessionOf(even), NOW).find((i) => i.id === "biggest")).toBeUndefined();
  });

  it("puts the largest opportunity first and shows at most three", () => {
    const insights = deriveInsights(
      sessionOf([
        artifact("abandoned", 30_000_000_000, 400),
        factory.target({ risk: "safe", sizeBytes: 5_000_000_000 }),
        factory.target({ risk: "low", sizeBytes: 60_000_000_000, label: "monster" }),
      ]),
      NOW,
    );

    expect(insights.length).toBeLessThanOrEqual(3);
    expect(insights[0]!.bytes).toBeGreaterThanOrEqual(insights[insights.length - 1]!.bytes);
    expect(insights[0]!.id).toBe("biggest");
  });
});
