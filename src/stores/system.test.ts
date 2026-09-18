import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { startPolling } from "./system";

/** jsdom has no visibility control, so drive `document.hidden` directly. */
function setHidden(hidden: boolean) {
  Object.defineProperty(document, "hidden", { value: hidden, configurable: true });
  document.dispatchEvent(new Event("visibilitychange"));
}

describe("startPolling", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    Object.defineProperty(document, "hidden", { value: false, configurable: true });
  });
  afterEach(() => vi.useRealTimers());

  it("ticks immediately and then on the interval", () => {
    const tick = vi.fn();
    const stop = startPolling(tick, 1000);
    expect(tick).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(2000);
    expect(tick).toHaveBeenCalledTimes(3);
    stop();
  });

  it("stops while the window is hidden, so a view nobody is looking at costs nothing", () => {
    const tick = vi.fn();
    const stop = startPolling(tick, 1000);
    tick.mockClear();

    setHidden(true);
    vi.advanceTimersByTime(10_000);
    expect(tick).not.toHaveBeenCalled();

    stop();
  });

  it("shows something current the moment the window comes back", () => {
    const tick = vi.fn();
    const stop = startPolling(tick, 1000);
    setHidden(true);
    vi.advanceTimersByTime(10_000);
    tick.mockClear();

    setHidden(false);
    // Not one interval later: straight away, so the first thing seen is not ten seconds old.
    expect(tick).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(1000);
    expect(tick).toHaveBeenCalledTimes(2);
    stop();
  });

  it("does not start a second timer if visibility flaps", () => {
    const tick = vi.fn();
    const stop = startPolling(tick, 1000);
    setHidden(false);
    setHidden(false);
    tick.mockClear();
    vi.advanceTimersByTime(1000);
    expect(tick).toHaveBeenCalledTimes(1);
    stop();
  });

  it("leaves no timer or listener behind", () => {
    const tick = vi.fn();
    const stop = startPolling(tick, 1000);
    stop();
    tick.mockClear();
    vi.advanceTimersByTime(5000);
    setHidden(false);
    vi.advanceTimersByTime(5000);
    expect(tick).not.toHaveBeenCalled();
  });
});
