import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProviderGroup } from "./ProviderGroup";
import { useScan } from "@/stores/scan";
import { resetStores } from "@/test/reset";
import * as factory from "@/test/factories";

vi.mock("@/lib/tauri", async () => {
  const actual = await vi.importActual<typeof import("@/lib/tauri")>("@/lib/tauri");
  return { ...actual, backend: { fsReveal: vi.fn() } };
});

const safe = factory.target({ id: "safe", label: "com.example.app", risk: "safe" });
const locked = factory.target({ id: "locked", label: "com.apple.bird", risk: "protected" });
const group = factory.result("user_cache", "application_cache", [safe, locked]);

beforeEach(() => {
  resetStores();
  vi.clearAllMocks();
});

describe("ProviderGroup", () => {
  it("lists its targets with their risk", () => {
    render(<ProviderGroup result={group} />);
    expect(screen.getByText("com.example.app")).toBeInTheDocument();
    expect(screen.getByText("Safe")).toBeInTheDocument();
    expect(screen.getByText("Protected")).toBeInTheDocument();
  });

  it("selects a target when its row is clicked", async () => {
    const user = userEvent.setup();
    render(<ProviderGroup result={group} />);
    await user.click(screen.getByText("com.example.app"));
    expect([...useScan.getState().selected]).toEqual(["safe"]);
  });

  it("never selects a protected target, by row click or by checkbox", async () => {
    const user = userEvent.setup();
    render(<ProviderGroup result={group} />);

    await user.click(screen.getByText("com.apple.bird"));
    expect(useScan.getState().selected.size).toBe(0);

    const checkbox = screen.getByRole("checkbox", { name: "com.apple.bird" });
    expect(checkbox).toBeDisabled();
    await user.click(checkbox);
    expect(useScan.getState().selected.size).toBe(0);
  });

  it("selects every removable target from the group checkbox, leaving protected ones out", async () => {
    const user = userEvent.setup();
    render(<ProviderGroup result={group} />);
    await user.click(screen.getByRole("checkbox", { name: "Select all in user_cache" }));
    expect([...useScan.getState().selected]).toEqual(["safe"]);
  });

  it("can be collapsed and expanded from the keyboard", async () => {
    const user = userEvent.setup();
    render(<ProviderGroup result={group} />);
    const header = screen.getByRole("button", { name: /Collapse user_cache/ });

    header.focus();
    await user.keyboard("{Enter}");
    expect(screen.queryByText("com.example.app")).not.toBeInTheDocument();

    await user.keyboard(" ");
    expect(screen.getByText("com.example.app")).toBeInTheDocument();
  });

  it("shows why a location was skipped instead of hiding it", async () => {
    const user = userEvent.setup();
    const withIssue = {
      ...group,
      issues: [
        { path: "/home/u/Library/Caches/com.apple.Safari", message: "Operation not permitted" },
      ],
    };
    render(<ProviderGroup result={withIssue} />);

    await user.click(screen.getByTitle("Some locations could not be read"));
    expect(screen.getByText("Operation not permitted")).toBeInTheDocument();
  });
});
