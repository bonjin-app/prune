import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { GpuCard } from "./GpuCard";

it("shows each GPU's load and memory as reported", () => {
  render(
    <GpuCard
      gpus={[
        { name: "Apple M3 Pro", usagePercent: 17.4, memoryUsedBytes: 1.3e9, coreCount: 18 },
        { usagePercent: null, memoryUsedBytes: 1e9, memoryTotalBytes: 8e9 },
      ]}
    />,
  );
  expect(screen.getByText("Apple M3 Pro, 18 cores")).toBeInTheDocument();
  expect(screen.getByText("17%")).toBeInTheDocument();
  expect(screen.getByText(/memory in use$/)).toBeInTheDocument();
  // An unnamed adapter gets a number, and an unknown load is said, not shown as 0%.
  expect(screen.getByText("GPU 2")).toBeInTheDocument();
  expect(screen.getByText("load unknown")).toBeInTheDocument();
  expect(screen.queryByText("0%")).not.toBeInTheDocument();
});

it("says so when the system reports no GPU", () => {
  render(<GpuCard gpus={[]} />);
  expect(screen.getByText(/does not report its graphics processor/)).toBeInTheDocument();
});
