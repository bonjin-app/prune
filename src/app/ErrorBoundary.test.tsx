import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ErrorBoundary } from "./ErrorBoundary";

function Boom({ explode }: { explode: boolean }): React.ReactElement {
  if (explode) throw new Error("render went wrong");
  return <p>the app</p>;
}

/** Lets the test stop the component from throwing before asking the boundary to go back. */
function Harness() {
  const [explode, setExplode] = useState(true);
  return (
    <>
      <button type="button" onClick={() => setExplode(false)}>
        Fix it
      </button>
      <ErrorBoundary>
        <Boom explode={explode} />
      </ErrorBoundary>
    </>
  );
}

beforeEach(() => {
  // React logs the caught error itself; the test is about what the user sees.
  vi.spyOn(console, "error").mockImplementation(() => {});
});
afterEach(() => vi.restoreAllMocks());

describe("ErrorBoundary", () => {
  it("stays out of the way when nothing is wrong", () => {
    render(
      <ErrorBoundary>
        <Boom explode={false} />
      </ErrorBoundary>,
    );
    expect(screen.getByText("the app")).toBeInTheDocument();
  });

  it("shows something to read instead of a blank window", () => {
    render(
      <ErrorBoundary>
        <Boom explode={true} />
      </ErrorBoundary>,
    );

    expect(screen.getByText("Something in the interface broke")).toBeInTheDocument();
    // The one thing the user needs to know about a cleanup tool that just broke.
    expect(screen.getByText(/Nothing was removed/)).toBeInTheDocument();
    expect(screen.getByText(/render went wrong/)).toBeInTheDocument();
  });

  it("can go back once the cause is gone", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    expect(screen.getByText("Something in the interface broke")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Fix it" }));
    await user.click(screen.getByRole("button", { name: /Go back/ }));

    expect(screen.getByText("the app")).toBeInTheDocument();
    expect(screen.queryByText("Something in the interface broke")).not.toBeInTheDocument();
  });
});
