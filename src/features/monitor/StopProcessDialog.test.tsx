import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { StopProcessDialog } from "./StopProcessDialog";
import { useSystem } from "@/stores/system";
import type { ProcessInfo } from "@/types/models";

/**
 * Stopping a program is the one thing Prune does that destroys something without touching a
 * file: whatever the program had not saved. These hold the promises the dialog makes about it.
 */
function proc(overrides: Partial<ProcessInfo> = {}): ProcessInfo {
  return {
    pid: 4321,
    name: "node",
    cpuPercent: 12.5,
    memoryBytes: 900_000_000,
    user: "developer",
    parentPid: 1,
    canTerminate: true,
    ...overrides,
  };
}

describe("StopProcessDialog", () => {
  beforeEach(() => {
    act(() => useSystem.setState({ error: null }));
  });

  it("shows nothing when no process was chosen", () => {
    const { container } = render(<StopProcessDialog process={null} onClose={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("names the program and says what is at stake", () => {
    render(<StopProcessDialog process={proc()} onClose={vi.fn()} />);

    expect(screen.getByRole("heading", { name: "Stop node?" })).toBeInTheDocument();
    expect(screen.getByText(/Anything it has not saved will be lost/)).toBeInTheDocument();
    // Quitting and forcing are different things, and the difference is said out loud.
    expect(screen.getByText(/Quitting lets it save first; forcing does not/)).toBeInTheDocument();
    expect(screen.getByText(/PID 4321/)).toBeInTheDocument();
  });

  it("offers asking and forcing as separate choices", () => {
    render(<StopProcessDialog process={proc()} onClose={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Quit" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Force stop" })).toBeEnabled();
  });

  it("sends the name it showed, so a recycled id cannot stop something else", async () => {
    const user = userEvent.setup();
    const stopProcess = vi.fn().mockResolvedValue(true);
    act(() => useSystem.setState({ stopProcess }));

    render(<StopProcessDialog process={proc()} onClose={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "Quit" }));

    expect(stopProcess).toHaveBeenCalledWith(4321, "ask", "node");
  });

  it("forcing asks for forcing, not for a polite request", async () => {
    const user = userEvent.setup();
    const stopProcess = vi.fn().mockResolvedValue(true);
    act(() => useSystem.setState({ stopProcess }));

    render(<StopProcessDialog process={proc()} onClose={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "Force stop" }));

    expect(stopProcess).toHaveBeenCalledWith(4321, "force", "node");
  });

  it("closes only when something was actually stopped", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    act(() => useSystem.setState({ stopProcess: vi.fn().mockResolvedValue(true) }));

    render(<StopProcessDialog process={proc()} onClose={onClose} />);
    await user.click(screen.getByRole("button", { name: "Quit" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("stays open when the system refused, so the other option is still there", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    // What Windows answers to a polite request: there are no signals to send.
    act(() => useSystem.setState({ stopProcess: vi.fn().mockResolvedValue(false) }));

    render(<StopProcessDialog process={proc()} onClose={onClose} />);
    await user.click(screen.getByRole("button", { name: "Quit" }));

    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Force stop" })).toBeEnabled();
  });
});
