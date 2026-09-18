import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";
import { ErrorToast } from "./ErrorToast";
import { useApps } from "@/stores/apps";
import { useDisk } from "@/stores/disk";
import { useScan } from "@/stores/scan";
import { useStartup } from "@/stores/startup";
import { useSystem } from "@/stores/system";

function clearAll() {
  act(() => {
    useScan.setState({ error: null });
    useDisk.setState({ error: null });
    useApps.setState({ error: null });
    useStartup.setState({ error: null });
    useSystem.setState({ error: null });
  });
}

describe("ErrorToast", () => {
  beforeEach(clearAll);

  it("shows nothing when nothing has failed", () => {
    render(<ErrorToast />);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("announces a failure rather than only drawing it", () => {
    render(<ErrorToast />);
    act(() => useScan.setState({ error: "could not read that folder" }));
    // role=alert is what tells a screen reader an action did not happen. Without it the only
    // signal is the absence of the result.
    expect(screen.getByRole("alert")).toHaveTextContent("could not read that folder");
  });

  it("clears the store it came from when dismissed", async () => {
    const user = userEvent.setup();
    render(<ErrorToast />);
    act(() => useDisk.setState({ error: "no such directory" }));

    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(useDisk.getState().error).toBeNull();
  });

  it("closes on Escape", async () => {
    const user = userEvent.setup();
    render(<ErrorToast />);
    act(() => useApps.setState({ error: "could not list applications" }));

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("stays dismissed when a timer keeps reporting the same failure", async () => {
    const user = userEvent.setup();
    render(<ErrorToast />);
    // The Monitor polls every two seconds. An unreachable backend sets this again and again.
    act(() => useSystem.setState({ error: "backend unavailable" }));
    await user.click(screen.getByRole("button", { name: "Dismiss" }));

    act(() => useSystem.setState({ error: "backend unavailable" }));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("still speaks up when something different fails", async () => {
    const user = userEvent.setup();
    render(<ErrorToast />);
    act(() => useSystem.setState({ error: "backend unavailable" }));
    await user.click(screen.getByRole("button", { name: "Dismiss" }));

    act(() => useStartup.setState({ error: "could not change that item" }));
    expect(screen.getByRole("alert")).toHaveTextContent("could not change that item");
  });
});
