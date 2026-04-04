import { describe, it, expect } from "vitest";
import { HeerId } from "../index.js";

describe("HeerId", () => {
  // Known HeerId: timestamp_ms=1_234_567, node_id=42, sequence=777
  // Layout: [41-bit timestamp][9-bit node][13-bit sequence]
  // raw = (1_234_567 << 22) | (42 << 13) | 777
  const KNOWN_RAW = (1_234_567n << 22n) | (42n << 13n) | 777n;

  describe("fromBigInt / toBigInt", () => {
    it("round-trips a known value", () => {
      const id = HeerId.fromBigInt(KNOWN_RAW);
      expect(id.toBigInt()).toBe(KNOWN_RAW);
    });

    it("rejects negative values", () => {
      expect(() => HeerId.fromBigInt(-1n)).toThrow();
    });
  });

  describe("fromString / toStringValue", () => {
    it("round-trips via decimal string", () => {
      const id = HeerId.fromBigInt(KNOWN_RAW);
      const str = id.toStringValue();
      const parsed = HeerId.fromString(str);
      expect(parsed.toBigInt()).toBe(KNOWN_RAW);
    });

    it("rejects garbage input", () => {
      expect(() => HeerId.fromString("not_a_number")).toThrow();
    });

    it("rejects negative string", () => {
      expect(() => HeerId.fromString("-1")).toThrow();
    });
  });

  describe("field getters", () => {
    it("extracts timestamp_ms", () => {
      const id = HeerId.fromBigInt(KNOWN_RAW);
      expect(id.timestampMs).toBe(1_234_567);
    });

    it("extracts node_id", () => {
      const id = HeerId.fromBigInt(KNOWN_RAW);
      expect(id.nodeId).toBe(42);
    });

    it("extracts sequence", () => {
      const id = HeerId.fromBigInt(KNOWN_RAW);
      expect(id.sequence).toBe(777);
    });
  });

  describe("zero value", () => {
    it("round-trips zero", () => {
      const id = HeerId.fromBigInt(0n);
      expect(id.timestampMs).toBe(0);
      expect(id.nodeId).toBe(0);
      expect(id.sequence).toBe(0);
      expect(id.toBigInt()).toBe(0n);
    });
  });

  describe("max field values", () => {
    // max timestamp = 2^41 - 1 = 2199023255551
    // max node_id   = 2^9  - 1 = 511
    // max sequence  = 2^13 - 1 = 8191
    it("handles maximum values", () => {
      const maxTs = (1n << 41n) - 1n;   // 2199023255551
      const maxNode = (1n << 9n) - 1n;  // 511
      const maxSeq = (1n << 13n) - 1n;  // 8191
      const maxRaw = (maxTs << 22n) | (maxNode << 13n) | maxSeq;
      const id = HeerId.fromBigInt(maxRaw);
      expect(id.timestampMs).toBe(Number(maxTs));
      expect(id.nodeId).toBe(Number(maxNode));
      expect(id.sequence).toBe(Number(maxSeq));
    });
  });
});
