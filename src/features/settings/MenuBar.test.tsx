import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it } from "vitest";
import { useSettings } from "@/stores/settings";
import { MenuBar } from "./MenuBar";

beforeEach(async () => {
  // The mock backend keeps its settings between tests; start each from the defaults.
  useSettings.setState({ view: null, saving: false, error: null });
  await useSettings.getState().load();
  await useSettings.getState().setMenuBar(true, false);
});

it("shows the icon by default and quits on close unless asked to stay", async () => {
  render(<MenuBar platform="macos" />);
  const show = await screen.findByRole("switch", { name: "Show Prune in the menu bar" });
  const keep = screen.getByRole("switch", { name: "Keep Prune there when its window is closed" });
  expect(show).toHaveAttribute("aria-checked", "true");
  expect(keep).toHaveAttribute("aria-checked", "false");

  fireEvent.click(keep);
  await waitFor(() => expect(keep).toHaveAttribute("aria-checked", "true"));
  expect(useSettings.getState().view?.settings.keepInMenuBar).toBe(true);
});

it("cannot keep Prune in a menu bar it is not shown in", async () => {
  render(<MenuBar platform="windows" />);
  const show = await screen.findByRole("switch", { name: "Show Prune in the notification area" });
  fireEvent.click(show);
  const keep = screen.getByRole("switch", { name: "Keep Prune there when its window is closed" });
  await waitFor(() => expect(keep).toBeDisabled());
  expect(keep).toHaveAttribute("aria-checked", "false");
});
