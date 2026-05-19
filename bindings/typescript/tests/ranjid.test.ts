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

  // ---------------------------------------------------------------------------
  // MSSQL BINARY(16) / byte-order-preservation tests
  //
  // Parallel to the .NET RanjIdTests in
  // bindings/dotnet/tests/HeeRanjID.Tests/RanjIdTests.cs. The TS binding
  // exposes fromBytes/toBytes for the same reason: SQL Server's BINARY(16)
  // column returns raw big-endian bytes via Prisma's sqlserver adapter, and
  // round-tripping through a Guid would apply mixed-endian swizzle and
  // scramble the sort key embedded in the high bits of a RanjId.
  // ---------------------------------------------------------------------------
  describe("fromBytes / toBytes", () => {
    // Big-endian bytes for "00000000-0000-8000-8000-0000006400c8":
    //   timestamp=0, precision=us(0), node=100 (0x0064), sequence=200 (0x00c8)
    const validUuidString = "00000000-0000-8000-8000-0000006400c8";
    const validUuidBytes = new Uint8Array([
      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00,
      0x80, 0x00, 0x00, 0x00, 0x00, 0x64, 0x00, 0xc8,
    ]);

    it("round-trips bytes via fromBytes -> toBytes", () => {
      const id = RanjId.fromBytes(Buffer.from(validUuidBytes));
      const roundTripped = id.toBytes();
      expect(Array.from(roundTripped)).toEqual(Array.from(validUuidBytes));
    });

    it("fromBytes yields the same identity as fromString for the same UUID", () => {
      const fromBytes = RanjId.fromBytes(Buffer.from(validUuidBytes));
      const fromString = RanjId.fromString(validUuidString);
      expect(fromBytes.toUuid()).toBe(fromString.toUuid());
    });

    it("fromBytes decodes node_id and sequence correctly", () => {
      const id = RanjId.fromBytes(Buffer.from(validUuidBytes));
      expect(id.nodeId).toBe(100);
      expect(id.sequence).toBe(200);
    });

    it("rejects buffers shorter than 16 bytes", () => {
      expect(() => RanjId.fromBytes(Buffer.alloc(15))).toThrow();
    });

    it("rejects buffers longer than 16 bytes", () => {
      expect(() => RanjId.fromBytes(Buffer.alloc(17))).toThrow();
    });

    it("rejects empty buffers", () => {
      expect(() => RanjId.fromBytes(Buffer.alloc(0))).toThrow();
    });

    it("rejects bytes that encode a non-UUIDv8 version", () => {
      // Flip byte[6] high nibble from 0x80 (v8) to 0x40 (v4).
      const v4Bytes = new Uint8Array(validUuidBytes);
      v4Bytes[6] = 0x40;
      expect(() => RanjId.fromBytes(Buffer.from(v4Bytes))).toThrow();
    });

    it("toBytes returns a fresh copy each call (mutation does not corrupt state)", () => {
      const id = RanjId.fromBytes(Buffer.from(validUuidBytes));
      const a = id.toBytes();
      a[0] = 0xff; // mutate the returned buffer
      const b = id.toBytes();
      // The next call must still see the original byte sequence.
      expect(Array.from(b)).toEqual(Array.from(validUuidBytes));
    });

    // ------------------------------------------------------------------
    // Bare Uint8Array parity (Prisma 6+ wire shape)
    //
    // Prisma 6's sqlserver adapter returns `BINARY(16)` columns as a
    // bare `Uint8Array`, not a Node `Buffer`. napi-rs's `Buffer`
    // `FromNapiValue` impl rejects bare `Uint8Array` with "Expected a
    // Buffer value", so we widened the native `fromBytes` signature to
    // accept `Uint8Array` (which `Buffer` also satisfies, since Buffer
    // is a Uint8Array subclass). The tests below mirror every
    // Buffer-based case above using a bare `Uint8Array` input.
    // ------------------------------------------------------------------
    it("round-trips with bare Uint8Array input (Prisma 6+ shape)", () => {
      const id = RanjId.fromBytes(validUuidBytes);
      const roundTripped = id.toBytes();
      expect(Array.from(roundTripped)).toEqual(Array.from(validUuidBytes));
    });

    it("fromBytes(Uint8Array) yields the same identity as fromString", () => {
      const fromBytes = RanjId.fromBytes(validUuidBytes);
      const fromString = RanjId.fromString(validUuidString);
      expect(fromBytes.toUuid()).toBe(fromString.toUuid());
    });

    it("fromBytes(Uint8Array) decodes node_id and sequence correctly", () => {
      const id = RanjId.fromBytes(validUuidBytes);
      expect(id.nodeId).toBe(100);
      expect(id.sequence).toBe(200);
    });

    it("rejects Uint8Array shorter than 16 bytes", () => {
      expect(() => RanjId.fromBytes(new Uint8Array(15))).toThrow();
    });

    it("rejects Uint8Array longer than 16 bytes", () => {
      expect(() => RanjId.fromBytes(new Uint8Array(17))).toThrow();
    });

    it("rejects empty Uint8Array", () => {
      expect(() => RanjId.fromBytes(new Uint8Array(0))).toThrow();
    });

    it("rejects Uint8Array that encodes a non-UUIDv8 version", () => {
      const v4Bytes = new Uint8Array(validUuidBytes);
      v4Bytes[6] = 0x40;
      expect(() => RanjId.fromBytes(v4Bytes)).toThrow();
    });

    // ------------------------------------------------------------------
    // RFC 4122 variant-bit rejection
    //
    // Parallel to the version-nibble test above. The variant bits live
    // in byte[8]'s high two bits and must be 0b10 (binary), i.e.
    // byte[8] & 0xc0 == 0x80. Flipping them to 0b01 must reject.
    // Mirrors the .NET RanjIdTests `FromBytes_RejectsInvalidVariant`.
    // ------------------------------------------------------------------
    it("rejects bytes with invalid RFC 4122 variant (byte[8] high two bits != 0b10)", () => {
      const badVariant = new Uint8Array(validUuidBytes);
      // Original byte[8] = 0x80 (high two bits = 0b10). Set them to
      // 0b01 → 0x40, which is the RFC 4122 "reserved for legacy NCS"
      // variant and is not accepted by heeranjid::RanjId::from_uuid.
      badVariant[8] = 0x40;
      expect(() => RanjId.fromBytes(badVariant)).toThrow();
    });

    // ------------------------------------------------------------------
    // Defensive-copy parity with the .NET binding
    //
    // The .NET `RanjId.FromBytes` constructor takes a defensive copy of
    // the caller-supplied array so that post-call mutations of the
    // input cannot corrupt the constructed RanjId. The Rust side
    // achieves this for free because `Uuid::from_slice` copies the
    // bytes into an owned `[u8; 16]`. This test asserts the invariant
    // at the JS boundary so that any future refactor to a borrowed
    // view (`Uint8ArraySlice`, etc.) fails loudly here rather than
    // shipping silent state corruption.
    // ------------------------------------------------------------------
    it("fromBytes takes a defensive copy of the input (mutation after construction does not corrupt state)", () => {
      const input = new Uint8Array(validUuidBytes);
      const id = RanjId.fromBytes(input);
      // Snapshot the constructed identity before tampering.
      const snapshotBytes = id.toBytes();
      const snapshotUuid = id.toUuid();

      // Corrupt the caller-owned input AFTER fromBytes has returned.
      input[0] = input[0] ^ 0xff;
      input[6] = 0x40; // would invalidate the version nibble
      input[8] = 0x40; // would invalidate the variant bits

      // The RanjId must still report the original bytes / UUID, proving
      // it does not alias the caller's array.
      expect(Array.from(id.toBytes())).toEqual(Array.from(snapshotBytes));
      expect(id.toUuid()).toBe(snapshotUuid);
    });
  });
});
