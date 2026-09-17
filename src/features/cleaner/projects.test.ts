import { beforeEach, describe, expect, it } from "vitest";
import { daysSince, describeActivity, groupByProject } from "./projects";
import * as factory from "@/test/factories";
import type { CleanupTarget } from "@/types/models";

function inProject(
  key: string,
  label: string,
  sizeBytes: number,
  lastActiveAt?: string,
): CleanupTarget {
  return factory.target({
    providerId: "project_artifacts",
    sizeBytes,
    risk: "low",
    group: { key, label, ...(lastActiveAt ? { lastActiveAt } : {}) },
  });
}

beforeEach(() => factory.resetIds());

describe("groupByProject", () => {
  it("collects a project's artifacts and adds up what it holds", () => {
    const { projects, ungrouped } = groupByProject([
      inProject("/p/web", "web", 1_000),
      inProject("/p/web", "web", 3_000),
      inProject("/p/api", "api", 500),
    ]);

    expect(ungrouped).toEqual([]);
    expect(projects.map((p) => p.group.label)).toEqual(["web", "api"]);
    expect(projects[0]!.sizeBytes).toBe(4_000);
    expect(projects[0]!.targets).toHaveLength(2);
  });

  it("puts the biggest project first, and the biggest artifact first inside it", () => {
    const { projects } = groupByProject([
      inProject("/p/small", "small", 10),
      inProject("/p/big", "big", 100),
      inProject("/p/big", "big", 900),
    ]);

    expect(projects.map((p) => p.group.label)).toEqual(["big", "small"]);
    expect(projects[0]!.targets.map((t) => t.sizeBytes)).toEqual([900, 100]);
  });

  it("keeps targets that belong to no project separate rather than inventing one", () => {
    const loose = factory.target({ providerId: "npm_cache" });
    const { projects, ungrouped } = groupByProject([inProject("/p/web", "web", 1), loose]);

    expect(projects).toHaveLength(1);
    expect(ungrouped).toEqual([loose]);
  });

  it("groups by key, not by label, so two projects of the same name stay apart", () => {
    const { projects } = groupByProject([
      inProject("/work/app", "app", 100),
      inProject("/personal/app", "app", 200),
    ]);

    expect(projects).toHaveLength(2);
    expect(projects.map((p) => p.group.key)).toEqual(["/personal/app", "/work/app"]);
  });

  it("handles an empty list", () => {
    expect(groupByProject([])).toEqual({ projects: [], ungrouped: [] });
  });
});

describe("daysSince", () => {
  const now = new Date("2026-09-17T12:00:00Z").getTime();

  it("counts whole days", () => {
    expect(daysSince("2026-09-17T11:00:00Z", now)).toBe(0);
    expect(daysSince("2026-09-16T00:00:00Z", now)).toBe(1);
    expect(daysSince("2026-06-19T12:00:00Z", now)).toBe(90);
  });

  it("returns null when there is nothing to read", () => {
    expect(daysSince(undefined, now)).toBeNull();
    expect(daysSince("not a date", now)).toBeNull();
  });

  it("never reports a negative age for a clock that ran ahead", () => {
    expect(daysSince("2026-09-18T12:00:00Z", now)).toBe(0);
  });
});

describe("describeActivity", () => {
  it("says nothing was read rather than claiming the project is dead", () => {
    expect(describeActivity(null)).toBe("no repository");
  });

  it("scales the wording with the age", () => {
    expect(describeActivity(0)).toBe("worked on today");
    expect(describeActivity(1)).toBe("worked on yesterday");
    expect(describeActivity(12)).toBe("12 days since work");
    expect(describeActivity(90)).toBe("3 months since work");
    expect(describeActivity(400)).toBe("over a year since work");
  });
});
