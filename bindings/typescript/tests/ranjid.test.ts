import { describe, it, expect } from "vitest";
import { RanjId } from "../index.js";

describe("RanjId", () => {
  // New UUIDv8 bit layout:
  //   bits 127-80: timestamp_high (48 bits)
  //   bits 79-76:  version = 1000 (UUIDv8)
  //   bits 75-64:  timestamp_mid (12 bits)
  //   bits 63-62:  variant = 10
  //   bits 61-60:  precision (2 bits): 00=us, 01=ns, 10=ps, 11=fs
  //   bits 59-31:  timestamp_low (29 bits)
  //   bits 30-16:  node_id (15 bits)
  //   bits 15-0:   sequence (16 bits)
  //
  // timestamp = ts_high(48) | ts_mid(12) | ts_low(29) = 89 bits

  // Helper to construct a known UUIDv8 with the new layout
  function makeKnownId(ts: bigint, precision: bigint, node: bigint, seq: bigint): string {
    const th = (ts >> 41n) & ((1n << 48n) - 1n);
    const tm = (ts >> 29n) & ((1n << 12n) - 1n);
    const tl = ts & ((1n << 29n) - 1n);
    const raw =
      (th << 80n) |
      (8n << 76n) |
      (tm << 64n) |
      (2n << 62n) |
      (precision << 60n) |
      (tl << 31n) |
      (node << 16n) |
      seq;
    const hex = raw.toString(16).padStart(32, "0");
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;
  }

  describe("fromString / toUuid / toStringValue", () => {
    it("round-trips through UUID string", () => {
      // timestamp=1_000_000, precision=us(0), node=100, seq=200
      const uuid = makeKnownId(1_000_000n, 0n, 100n, 200n);
      const id = RanjId.fromString(uuid);
      expect(id.toUuid()).toBe(uuid);
      expect(id.toStringValue()).toBe(uuid);
    });

    it("rejects non-UUIDv8 strings", () => {
      // UUID v4 (random) should be rejected
      expect(() => RanjId.fromString("550e8400-e29b-41d4-a716-446655440000")).toThrow();
    });

    it("rejects garbage strings", () => {
      expect(() => RanjId.fromString("not-a-uuid")).toThrow();
    });
  });

  describe("field getters", () => {
    it("extracts timestamp_micros", () => {
      // timestamp=1_234_567_890_123 in microseconds (precision=us=0)
      const uuid = makeKnownId(1_234_567_890_123n, 0n, 100n, 4096n);
      const id = RanjId.fromString(uuid);
      expect(id.timestampMicros).toBe(1_234_567_890_123);
    });

    it("extracts node_id", () => {
      // node_id=100 (fits in 15 bits, max 32767)
      const uuid = makeKnownId(1_234_567_890_123n, 0n, 100n, 4096n);
      const id = RanjId.fromString(uuid);
      expect(id.nodeId).toBe(100);
    });

    it("extracts sequence", () => {
      const uuid = makeKnownId(1_234_567_890_123n, 0n, 100n, 4096n);
      const id = RanjId.fromString(uuid);
      expect(id.sequence).toBe(4096);
    });

    it("extracts max node_id (32767)", () => {
      const uuid = makeKnownId(1_000_000n, 0n, 32767n, 0n);
      const id = RanjId.fromString(uuid);
      expect(id.nodeId).toBe(32767);
    });
  });

  describe("zero value", () => {
    it("round-trips zero fields", () => {
      // timestamp=0, precision=us(0), node=0, seq=0
      const uuid = makeKnownId(0n, 0n, 0n, 0n);
      const id = RanjId.fromString(uuid);
      expect(id.timestampMicros).toBe(0);
      expect(id.nodeId).toBe(0);
      expect(id.sequence).toBe(0);
    });
  });

  describe("precision values", () => {
    it("microsecond precision produces correct UUID", () => {
      const uuid = makeKnownId(1000n, 0n, 1n, 0n);
      const id = RanjId.fromString(uuid);
      expect(id.timestampMicros).toBe(1000);
    });

    it("nanosecond precision produces correct UUID", () => {
      // 1000 nanoseconds = 1 microsecond
      const uuid = makeKnownId(1000n, 1n, 1n, 0n);
      const id = RanjId.fromString(uuid);
      // timestamp_micros should convert: 1000ns / 1000 = 1us
      expect(id.timestampMicros).toBe(1);
    });
  });
});
