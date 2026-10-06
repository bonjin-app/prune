import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";
import { PermissionBanner } from "./PermissionBanner";
import { useSystem } from "@/stores/system";
import { useUi } from "@/stores/ui";

/**
 * The banner is how a person learns that every total on screen is smaller than the truth. It
 * has to be there when it applies, and closing it has to mean something.
 */
function setPermissions(state: "denied" | "granted") {
  act(() =>
    useSystem.setState({
      permissions: {
        fullDiskAccess: state,
        blocked: ["Trash", "Safari data"],
        howToGrant: "Add Prune under Full Disk Access.",
      },
    }),
  );
}

describe("PermissionBanner", () => {
  beforeEach(() => {
    act(() => useUi.setState({ permissionNoticeDismissed: false }));
  });

  it("names what is hidden and how to fix it", () => {
    setPermissions("denied");
    render(<PermissionBanner />);
    expect(screen.getByText(/Trash, Safari data/)).toBeInTheDocument();
    expect(screen.getByText(/Add Prune under Full Disk Access/)).toBeInTheDocument();
  });

  it("says nothing when nothing is hidden", () => {
    setPermissions("granted");
    const { container } = render(<PermissionBanner />);
    expect(container).toBeEmptyDOMElement();
  });

  it("stays closed when the screen is opened again", async () => {
    // Every screen mounts its own banner, so moving to another one and back is a fresh
    // mount. Closing it on the dashboard and meeting it again on the cleaner a click later
    // is what this guards against.
    const user = userEvent.setup();
    setPermissions("denied");
    const first = render(<PermissionBanner />);
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(first.container).toBeEmptyDOMElement();
    first.unmount();

    const second = render(<PermissionBanner />);
    expect(second.container).toBeEmptyDOMElement();
  });

  it("comes back on the next run", () => {
    // Dismissal is held in memory only. A notice that says every total is too small must not
    // be silenced for good by one click.
    setPermissions("denied");
    act(() => useUi.getState().dismissPermissionNotice());
    const closed = render(<PermissionBanner />);
    expect(closed.container).toBeEmptyDOMElement();
    closed.unmount();

    // A new run starts from the store's initial state.
    act(() => useUi.setState({ permissionNoticeDismissed: false }));
    render(<PermissionBanner />);
    expect(screen.getByText("Some locations are hidden from Prune")).toBeInTheDocument();
  });
});
