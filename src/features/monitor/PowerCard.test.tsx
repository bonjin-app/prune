import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { PowerCard } from "./PowerCard";

it("shows charge, time left, wear and cycles from what the OS reports", () => {
  render(
    <PowerCard
      battery={{
        percent: 82,
        pluggedIn: false,
        charging: false,
        minutesRemaining: 312,
        cycleCount: 135,
        designCycleCount: 1000,
        capacityPercent: 92.4,
      }}
      temperatures={{ cpuCelsius: 56.6, batteryCelsius: 31, storageCelsius: null }}
    />,
  );
  expect(screen.getByText("82%")).toBeInTheDocument();
  expect(screen.getByText("On battery")).toBeInTheDocument();
  expect(screen.getByText("5h 12m left")).toBeInTheDocument();
  expect(screen.getByText("about 92% of new")).toBeInTheDocument();
  expect(screen.getByText("135 of 1000")).toBeInTheDocument();
  expect(screen.getByText("57 °C")).toBeInTheDocument();
  // A missing sensor is unknown, not zero.
  expect(screen.queryByText("0 °C")).not.toBeInTheDocument();
});

it("tells charging apart from merely plugged in", () => {
  const { rerender } = render(
    <PowerCard
      battery={{ percent: 40, pluggedIn: true, charging: true, minutesRemaining: 18 }}
      temperatures={{}}
    />,
  );
  expect(screen.getByText("Charging")).toBeInTheDocument();
  expect(screen.getByText("18m until full")).toBeInTheDocument();

  rerender(
    <PowerCard battery={{ percent: 80, pluggedIn: true, charging: false }} temperatures={{}} />,
  );
  expect(screen.getByText("Plugged in, not charging")).toBeInTheDocument();
});

it("says so plainly on a desktop without sensors", () => {
  render(<PowerCard battery={null} temperatures={{}} />);
  expect(screen.getByText("No battery.")).toBeInTheDocument();
  expect(screen.getByText(/does not report temperatures/)).toBeInTheDocument();
});
