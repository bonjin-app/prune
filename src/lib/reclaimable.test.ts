import { describe, expect, it } from "vitest";
import { reclaimableBytes, sessionReclaimableBytes } from "./reclaimable";
import * as factory from "@/test/factories";

describe("reclaimable bytes", () => {
  it("leaves out what is protected", () => {
    // A model cache that can go beside a conversation history that cannot.
    const r = factory.result("ai", "developer_files", [
      factory.target({ sizeBytes: 10, risk: "medium" }),
      factory.target({ sizeBytes: 90, risk: "protected" }),
    ]);
    expect(r.totalBytes).toBe(100);
    expect(reclaimableBytes(r)).toBe(10);
  });

  it("adds every result of a session", () => {
    const s = factory.session([
      factory.result("a", "logs", [factory.target({ sizeBytes: 5, risk: "safe" })]),
      factory.result("b", "developer_files", [factory.target({ sizeBytes: 7, risk: "protected" })]),
    ]);
    expect(s.totalBytes).toBe(12);
    expect(sessionReclaimableBytes(s)).toBe(5);
  });

  it("is everything when nothing is protected", () => {
    const r = factory.result("a", "logs", [factory.target({ sizeBytes: 3, risk: "low" })]);
    expect(reclaimableBytes(r)).toBe(r.totalBytes);
  });
});
