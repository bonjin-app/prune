import { render, screen } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";
import { ResultDialog } from "./ResultDialog";
import * as factory from "@/test/factories";
import { useScan } from "@/stores/scan";

/**
 * What the user is told after something was removed.
 *
 * By the time this appears the files are gone, so its only job is to be accurate: how much went
 * and where it went, what did not go and why, and the one case that reads like a failure but is
 * not — a cleanup that worked but could not be written to the history.
 */
function show(result: ReturnType<typeof factory.cleanupResult>) {
  act(() => useScan.setState({ result }));
  return render(<ResultDialog />);
}

describe("ResultDialog", () => {
  beforeEach(() => act(() => useScan.setState({ result: null })));

  it("shows nothing until a cleanup has run", () => {
    const { container } = render(<ResultDialog />);
    expect(container).toBeEmptyDOMElement();
  });

  it("says where the things went, not just how many", () => {
    show(
      factory.cleanupResult({
        mode: "trash",
        removedTargets: 3,
        removedFiles: 120,
        removedBytes: 4_000_000,
      }),
    );
    expect(screen.getByText(/moved to the Trash/)).toBeInTheDocument();
    expect(screen.queryByText(/deleted permanently/)).toBeNull();
  });

  it("does not say Trash when nothing went there", () => {
    show(factory.cleanupResult({ mode: "permanent", removedTargets: 1 }));
    expect(screen.getByText(/deleted permanently/)).toBeInTheDocument();
    expect(screen.queryByText(/moved to the Trash/)).toBeNull();
  });

  it("reports a failed cleanup as a failure, not in the colour of success", () => {
    const { container } = show(
      factory.cleanupResult({ status: "failed", removedTargets: 0, removedBytes: 0 }),
    );
    expect(screen.getByRole("heading", { name: "Nothing removed" })).toBeInTheDocument();
    // The badge is the biggest thing on screen and is often the only thing read.
    expect(container.querySelector(".bg-accent-soft")).toBeNull();
    expect(container.querySelector(".bg-danger-soft")).not.toBeNull();
  });

  it("distinguishes a partial cleanup from both", () => {
    const { container } = show(factory.cleanupResult({ status: "partial" }));
    expect(screen.getByRole("heading", { name: "Partially pruned" })).toBeInTheDocument();
    expect(container.querySelector(".bg-warn-soft")).not.toBeNull();
    expect(container.querySelector(".bg-accent-soft")).toBeNull();
  });

  it("says a cleanup worked even when the history could not be written", () => {
    // The removal succeeded; only the record of it failed. Reporting that as a failed cleanup
    // would tell the user the opposite of what happened and invite them to run it again.
    show(
      factory.cleanupResult({
        status: "success",
        removedTargets: 2,
        logError: "permission denied",
      }),
    );
    expect(screen.getByRole("heading", { name: "Pruned" })).toBeInTheDocument();
    expect(screen.getByText(/could not be added to the operation log/)).toBeInTheDocument();
    expect(screen.getByText(/permission denied/)).toBeInTheDocument();
  });

  it("lists what could not be removed, with the reason and the path", () => {
    show(
      factory.cleanupResult({
        status: "partial",
        failed: [
          { targetId: "a", path: "/home/u/Library/Caches/locked", error: "resource busy" },
        ],
      }),
    );
    expect(screen.getByText("/home/u/Library/Caches/locked")).toBeInTheDocument();
    expect(screen.getByText("resource busy")).toBeInTheDocument();
  });

  it("keeps quiet about failures when there were none", () => {
    show(factory.cleanupResult({ status: "success", failed: [] }));
    expect(screen.queryByText(/could not be added to the operation log/)).toBeNull();
  });
});
