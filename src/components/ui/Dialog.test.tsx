import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Dialog } from "./Dialog";

function Harness({ closeOnBackdrop = true }: { closeOnBackdrop?: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        Open
      </button>
      <Dialog
        open={open}
        onClose={() => setOpen(false)}
        closeOnBackdrop={closeOnBackdrop}
        label="Preview cleanup"
      >
        <button type="button">First</button>
        <button type="button">Second</button>
      </Dialog>
    </>
  );
}

describe("Dialog", () => {
  it("has an accessible name and moves focus to its first control", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Open" }));

    expect(screen.getByRole("dialog", { name: "Preview cleanup" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "First" })).toHaveFocus();
  });

  it("keeps Tab inside the dialog", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Open" }));

    const first = screen.getByRole("button", { name: "First" });
    const second = screen.getByRole("button", { name: "Second" });

    await user.tab();
    expect(second).toHaveFocus();
    // Past the last control, focus wraps rather than escaping to the page behind.
    await user.tab();
    expect(first).toHaveFocus();
    await user.tab({ shift: true });
    expect(second).toHaveFocus();
  });

  it("returns focus to whatever opened it", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const opener = screen.getByRole("button", { name: "Open" });
    await user.click(opener);
    await user.keyboard("{Escape}");

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
  });

  it("closes on a backdrop click only when that is allowed", async () => {
    const user = userEvent.setup();
    const { unmount } = render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Open" }));
    const backdrop = screen.getByRole("dialog").parentElement!;
    await user.click(backdrop);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    unmount();

    render(<Harness closeOnBackdrop={false} />);
    await user.click(screen.getByRole("button", { name: "Open" }));
    await user.click(screen.getByRole("dialog").parentElement!);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("renders nothing while closed", () => {
    const onClose = vi.fn();
    render(
      <Dialog open={false} onClose={onClose}>
        <button type="button">Hidden</button>
      </Dialog>,
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });
});
