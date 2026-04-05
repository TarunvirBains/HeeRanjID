import { describe, it, expect } from "vitest";
import { RanjId } from "../index.js";

describe("RanjId", () => {
  // We construct a known RanjId by using Rust's RanjId::new(1_000_000, 100, 200)
  // and round-tripping through fromString. We'll use a helper to get a known UUID.
  // Instead, let's just test the string-based API.

  describe("fromString / toUuid / toStringValue", () => {
    it("round-trips through UUID string", () => {
      // First, create via fromString with a valid UUIDv8.
      // We need a real UUIDv8 to test with. Let's construct one.
      // UUIDv8 format: tttttttt-tttt-8ttt-Vxxx-xxxxxxxxxxxx
      // For our custom layout:
      //   bits 127-80: timestamp_high (48 bits)
      //   bits 79-76:  version = 0111
      //   bits 75-64:  timestamp_mid (12 bits)
      //   bits 63-62:  variant = 10
      //   bits 61-32:  timestamp_low (30 bits)
      //   bits 31-16:  node_id (16 bits)
      //   bits 15-0:   sequence (16 bits)
      //
      // timestamp_micros = 1_000_000, node_id = 100, sequence = 200
      // timestamp = 1_000_000
      // timestamp_high = (1_000_000 >> 42) & 0xFFFFFFFFFFFF = 0
      // timestamp_mid  = (1_000_000 >> 30) & 0xFFF = 0
      // timestamp_low  = 1_000_000 & 0x3FFFFFFF = 1_000_000
      //
      // raw_u128 = (0 << 80) | (7 << 76) | (0 << 64) | (2 << 62) | (1_000_000 << 32) | (100 << 16) | 200
      //          = 0x0000_0000_0000_7000_8000_000F_4240_0064_00C8
      // Hmm, let me compute this properly...
      // Actually, let's just build the native binary, create an ID via Rust test,
      // and capture the UUID. But easier: let's use Rust to print it.
      // For now, let's just use a computation approach.

      const timestamp_micros = 1_000_000n;
      const node_id = 100n;
      const sequence = 200n;

      const timestamp_high = (timestamp_micros >> 42n) & ((1n << 48n) - 1n);
      const timestamp_mid = (timestamp_micros >> 30n) & ((1n << 12n) - 1n);
      const timestamp_low = timestamp_micros & ((1n << 30n) - 1n);

      const raw =
        (timestamp_high << 80n) |
        (8n << 76n) |
        (timestamp_mid << 64n) |
        (2n << 62n) |
        (timestamp_low << 32n) |
        (node_id << 16n) |
        sequence;

      // Convert to UUID string format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
      const hex = raw.toString(16).padStart(32, "0");
      const uuid = `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;

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
    // Build a known UUIDv8
    function makeKnownId(ts: bigint, node: bigint, seq: bigint): string {
      const th = (ts >> 42n) & ((1n << 48n) - 1n);
      const tm = (ts >> 30n) & ((1n << 12n) - 1n);
      const tl = ts & ((1n << 30n) - 1n);
      const raw =
        (th << 80n) |
        (8n << 76n) |
        (tm << 64n) |
        (2n << 62n) |
        (tl << 32n) |
        (node << 16n) |
        seq;
      const hex = raw.toString(16).padStart(32, "0");
      return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;
    }

    it("extracts timestamp_micros", () => {
      const uuid = makeKnownId(1_234_567_890_123n, 513n, 4096n);
      const id = RanjId.fromString(uuid);
      expect(id.timestampMicros).toBe(1_234_567_890_123);
    });

    it("extracts node_id", () => {
      const uuid = makeKnownId(1_234_567_890_123n, 513n, 4096n);
      const id = RanjId.fromString(uuid);
      expect(id.nodeId).toBe(513);
    });

    it("extracts sequence", () => {
      const uuid = makeKnownId(1_234_567_890_123n, 513n, 4096n);
      const id = RanjId.fromString(uuid);
      expect(id.sequence).toBe(4096);
    });
  });

  describe("zero value", () => {
    it("round-trips zero fields", () => {
      // timestamp=0, node=0, seq=0
      // raw = (8 << 76) | (2 << 62)
      const raw = (8n << 76n) | (2n << 62n);
      const hex = raw.toString(16).padStart(32, "0");
      const uuid = `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;
      const id = RanjId.fromString(uuid);
      expect(id.timestampMicros).toBe(0);
      expect(id.nodeId).toBe(0);
      expect(id.sequence).toBe(0);
    });
  });
});
