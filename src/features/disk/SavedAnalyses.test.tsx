import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { backend } from "@/lib/tauri";
import { SavedAnalyses } from "./SavedAnalyses";

vi.mock("@/lib/tauri", () => ({ backend: { diskListSaved: vi.fn() } }));

beforeEach(() => vi.resetAllMocks());

const saved = {
  root: "/home/u",
  finishedAt: new Date(Date.now() - 3 * 86400e3).toISOString(),
  totalBytes: 412_000_000_000,
  fileCount: 1_240_000,
  dirCount: 210_000,
  durationMs: 142_000,
  issueCount: 2,
  folders: [
    { name: "Library", sizeBytes: 168_000_000_000 },
    { name: "Projects", sizeBytes: 96_000_000_000 },
  ],
};

it("shows what an earlier analysis found, when, and offers only to analyze again", async () => {
  vi.mocked(backend.diskListSaved).mockResolvedValue([saved]);
  const onAnalyze = vi.fn();
  render(<SavedAnalyses home="/home/u" disabled={false} onAnalyze={onAnalyze} />);

  const card = await screen.findByRole("article", { name: "Earlier analysis of /home/u" });
  expect(within(card).getByText("412 GB")).toBeInTheDocument();
  expect(within(card).getByText(/analyzed 3d ago/)).toBeInTheDocument();
  expect(within(card).getByText("Library")).toBeInTheDocument();
  expect(within(card).getByText(/could not be read then/)).toBeInTheDocument();
  // A record, not a place to remove things from.
  expect(within(card).getAllByRole("button")).toHaveLength(1);

  await userEvent.click(within(card).getByRole("button", { name: /Analyze again/ }));
  expect(onAnalyze).toHaveBeenCalledWith("/home/u");
});

it("shows nothing when there is no history, or it cannot be read", async () => {
  vi.mocked(backend.diskListSaved).mockRejectedValue(new Error("unreadable"));
  const { container } = render(
    <SavedAnalyses home="/home/u" disabled={false} onAnalyze={vi.fn()} />,
  );
  await Promise.resolve();
  expect(container).toBeEmptyDOMElement();
});
