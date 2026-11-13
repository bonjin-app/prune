import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { CleanerWorkspace } from "./CleanerWorkspace";
import { useScan } from "@/stores/scan";
import { resetStores } from "@/test/reset";
import { result, session, target } from "@/test/factories";

vi.mock("@/lib/tauri", async () => {
  const actual = await vi.importActual<typeof import("@/lib/tauri")>("@/lib/tauri");
  return { ...actual, backend: { cleanerCancelScan: vi.fn().mockResolvedValue(undefined) } };
});

beforeEach(() => resetStores());

it("disables repeat cancellation and announces waiting until completion", async () => {
  useScan.setState({ scanId: "scan1", scanning: true });
  render(
    <CleanerWorkspace
      scope="cleaner"
      categories={["application_cache"]}
      emptyTitle="Scan caches"
      emptyDescription="Find caches"
    />,
  );
  await userEvent.setup().click(screen.getByRole("button", { name: "Cancel" }));
  expect(screen.getByRole("button", { name: "Cancelling…" })).toBeDisabled();
  expect(screen.getByRole("progressbar")).toHaveAttribute(
    "aria-valuetext",
    expect.stringContaining("Cancelling"),
  );
  expect(screen.queryByText(/Scan finished/)).not.toBeInTheDocument();
  act(() => useScan.getState().onScanCompleted({ ...session([]), status: "cancelled" }));
  expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  expect(screen.getByText(/results are partial/)).toBeInTheDocument();
});

it("keeps what is protected out of the amount there is to review", () => {
  useScan.setState({
    session: session([
      result("ai_hugging_face", "ai_data", [target({ sizeBytes: 3_000_000_000, risk: "medium" })]),
      result("ai_ollama", "ai_data", [target({ sizeBytes: 40_000_000_000, risk: "protected" })]),
    ]),
  });
  render(
    <CleanerWorkspace
      scope="developer"
      categories={["ai_data"]}
      emptyTitle="Prune your toolchain"
      emptyDescription="Find caches"
    />,
  );
  expect(screen.getByText(/3\.00 GB to review, 40\.0 GB protected/)).toBeInTheDocument();
  expect(screen.queryByText(/43\.0 GB/)).not.toBeInTheDocument();
});
