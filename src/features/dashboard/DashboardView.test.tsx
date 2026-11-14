import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";
import { DashboardView } from "./DashboardView";
import * as factory from "@/test/factories";
import { resetStores } from "@/test/reset";
import { useScan } from "@/stores/scan";
import { useUi } from "@/stores/ui";

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

  it("sends AI models to the Developer view, where they are listed with the toolchain", async () => {
    const user = userEvent.setup();
    act(() =>
      useScan.setState({
        session: factory.session([
          factory.result("ai_hugging_face", "ai_data", [
            factory.target({ sizeBytes: 9_000_000_000, risk: "medium" }),
          ]),
        ]),
      }),
    );
    render(<DashboardView />);
    await user.click(screen.getByRole("button", { name: /AI Models & Data/ }));
    expect(useUi.getState().view).toBe("developer");
  });

  it("says how much more is protected, without counting it", () => {
    act(() =>
      useScan.setState({
        session: factory.session([
          factory.result("ai_ollama", "ai_data", [
            factory.target({ sizeBytes: 65_000_000_000, risk: "protected" }),
          ]),
          factory.result("caches", "application_cache", [
            factory.target({ sizeBytes: 1_000_000_000, risk: "safe" }),
          ]),
        ]),
      }),
    );
    render(<DashboardView />);
    expect(screen.getByText(/65\.0 GB more is protected and only shown/)).toBeInTheDocument();
  });

  it("keeps to one screen by folding the smallest categories", () => {
    const categories = [
      "developer_files",
      "ai_data",
      "application_cache",
      "browser_cache",
      "logs",
      "temporary_files",
      "trash",
      "old_installers",
    ] as const;
    act(() =>
      useScan.setState({
        session: factory.session(
          categories.map((category, i) =>
            factory.result(`p${i}`, category, [
              factory.target({ sizeBytes: (10 - i) * 1_000_000_000, risk: "safe" }),
            ]),
          ),
        ),
      }),
    );
    render(<DashboardView />);
    const list = screen.getByRole("list", { name: "By category" });
    // Six largest, then one line for the rest.
    expect(within(list).getAllByRole("button")).toHaveLength(7);
    expect(within(list).getByText("and 2 more categories")).toBeInTheDocument();
    expect(within(list).queryByText("Old Installers")).not.toBeInTheDocument();
  });

  it("opens the Monitor from the system readings", async () => {
    const user = userEvent.setup();
    render(<DashboardView />);
    await user.click(screen.getByRole("button", { name: "Open the Monitor" }));
    expect(useUi.getState().view).toBe("monitor");
  });
});
