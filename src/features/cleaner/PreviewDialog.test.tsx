import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PreviewDialog } from "./PreviewDialog";
import * as factory from "@/test/factories";
import { useScan } from "@/stores/scan";

/**
 * The last thing a user reads before anything is removed.
 *
 * Everything asserted here is a promise the dialog makes about what is about to happen: that it
 * lists what will go, says what cannot go and why, names the one kind of item that cannot be
 * undone, and does not let itself be dismissed halfway through. A defect in any of them means
 * somebody approves something they were not shown.
 */
function show(plan: ReturnType<typeof factory.plan>, extra: Record<string, unknown> = {}) {
  act(() => useScan.setState({ plan, executing: false, cleanupProgress: null, ...extra }));
  return render(<PreviewDialog />);
}

describe("PreviewDialog", () => {
  beforeEach(() => {
    act(() => useScan.setState({ plan: null, executing: false, cleanupProgress: null }));
  });

  it("shows nothing until there is a plan", () => {
    const { container } = render(<PreviewDialog />);
    expect(container).toBeEmptyDOMElement();
  });

  it("lists every target with its path and size", () => {
    const a = factory.target({ id: "a", label: "npm cache", path: "/home/u/.npm/_cacache" });
    const b = factory.target({ id: "b", label: "Xcode DerivedData", sizeBytes: 5_000_000 });
    show(factory.plan([a, b]));

    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("npm cache")).toBeInTheDocument();
    expect(within(dialog).getByText("/home/u/.npm/_cacache")).toBeInTheDocument();
    expect(within(dialog).getByText("Xcode DerivedData")).toBeInTheDocument();
  });

  it("says what cannot be removed and why, rather than leaving it out", () => {
    const plan = factory.plan([factory.target({ id: "a" })], {
      blocked: [
        { targetId: "gone", path: "/home/u/.ssh", reason: "inside a protected tree" },
        { targetId: "stale", path: "", reason: "not part of this scan" },
      ],
    });
    show(plan);

    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText(/inside a protected tree/)).toBeInTheDocument();
    // A blocked entry with no path still has to be identifiable.
    expect(within(dialog).getByText(/not part of this scan/)).toBeInTheDocument();
    expect(within(dialog).getByText("stale")).toBeInTheDocument();
  });

  it("names the items that will be deleted outright even in Trash mode", () => {
    // Anything already in the Trash can only go permanently. The button says "Move to Trash",
    // so the dialog has to say that one of these is not coming back.
    const plan = factory.plan(
      [
        factory.target({ id: "cache" }),
        factory.target({ id: "trashed", permanentOnly: true }),
        factory.target({ id: "trashed2", permanentOnly: true }),
      ],
      { mode: "trash" },
    );
    show(plan);

    expect(screen.getByText(/2 items already in the Trash will be deleted permanently/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Move to Trash/ })).toBeInTheDocument();
  });

  it("warns that permanent deletion cannot be undone", () => {
    show(factory.plan([factory.target({ id: "a" })], { mode: "permanent" }));

    expect(screen.getByText(/Permanent deletion cannot be undone/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Delete/ })).toBeInTheDocument();
    // The trash-only note is beside the point when everything is going permanently.
    expect(screen.queryByText(/already in the Trash/)).toBeNull();
  });

  it("warns when the plan is not all safe", () => {
    show(factory.plan([factory.target({ id: "a", risk: "medium" })]));
    expect(screen.getByText(/Includes medium-risk items/)).toBeInTheDocument();
  });

  it("stays quiet when there is nothing to warn about", () => {
    show(factory.plan([factory.target({ id: "a", risk: "safe" })]));
    expect(screen.queryByText(/cannot be undone/)).toBeNull();
    expect(screen.queryByText(/medium-risk/)).toBeNull();
    expect(screen.queryByText(/already in the Trash/)).toBeNull();
  });

  it("will not start on an empty plan", () => {
    show(factory.plan([]));
    expect(screen.getByRole("button", { name: /Move to Trash/ })).toBeDisabled();
  });

  it("cannot be dismissed while it is removing things", async () => {
    const user = userEvent.setup();
    const closePreview = vi.fn();
    show(factory.plan([factory.target({ id: "a" })]), { executing: true, closePreview });

    await user.keyboard("{Escape}");
    expect(closePreview).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
  });

  it("can be dismissed before anything starts", async () => {
    const user = userEvent.setup();
    const closePreview = vi.fn();
    show(factory.plan([factory.target({ id: "a" })]), { closePreview });

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(closePreview).toHaveBeenCalled();
  });
});
