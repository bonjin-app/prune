import { render, screen } from "@testing-library/react";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";
import { DashboardView } from "./DashboardView";
import * as factory from "@/test/factories";
import { resetStores } from "@/test/reset";
import { useScan } from "@/stores/scan";

describe("DashboardView", () => {
  beforeEach(() => resetStores());

  it("does not count protected items in what can be pruned", () => {
    // 2 GB that can go, and 50 GB that is only listed — a history Prune will never remove.
    act(() =>
      useScan.setState({
        session: factory.session([
          factory.result("caches", "application_cache", [
            factory.target({ sizeBytes: 2_000_000_000, risk: "safe" }),
          ]),
          factory.result("sessions", "developer_files", [
            factory.target({ sizeBytes: 50_000_000_000, risk: "protected" }),
          ]),
        ]),
      }),
    );
    render(<DashboardView />);
    expect(screen.getByText("2.00")).toBeInTheDocument();
    expect(screen.getByText("can be pruned")).toBeInTheDocument();
    expect(screen.queryByText("52.0")).not.toBeInTheDocument();
    // A category holding only protected items has nothing to offer, so it is not listed.
    expect(screen.queryByText("Developer Files")).not.toBeInTheDocument();
  });
});
