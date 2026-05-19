/**
 * Lightweight stub of the native `heeranjid` NAPI module.
 * Used by vitest so shape/SQL tests can run without a compiled binary.
 *
 * The mock RanjId stores its value as the canonical UUID string regardless
 * of which factory was used to construct it; `fromBytes` converts the input
 * 16-byte buffer to that canonical hyphenated form so equality and round-trip
 * assertions hold across the postgres/mssql code paths.
 */

export class HeerId {
  constructor(private readonly value: bigint) {}

  static fromBigInt(value: bigint): HeerId {
    return new HeerId(value);
  }

  toBigInt(): bigint {
    return this.value;
  }
}

export class RanjId {
  constructor(private readonly value: string) {}

  /**
   * Mock equivalent of the native `fromString` factory.
   *
   * Validates the same surface as `heeranjid::RanjId::from_uuid`:
   *   1. Canonical 8-4-4-4-12 hyphenated UUID shape (length 36 with
   *      hyphens at positions 8, 13, 18, 23 and hex digits elsewhere).
   *   2. UUIDv8 version nibble at position 14 (the first hex digit of
   *      the third group, which encodes the version) is `"8"`.
   *   3. RFC 4122 variant bits at position 19 (the first hex digit of
   *      the fourth group) are `0b10xx`, i.e. high nibble is one of
   *      `"8"`, `"9"`, `"a"`, `"b"`.
   *
   * Without these checks, mock-based tests would silently accept
   * malformed UUIDs that production code (the real NAPI binding at
   * `bindings/typescript/src/lib.rs`'s `from_string` / `from_uuid`)
   * would throw on, hiding bugs that only surface in the integration /
   * native test pass. This mirrors the same hollow-test failure mode
   * that motivated the byte-level validation in {@link fromBytes}.
   *
   * Error messages are deliberately phrased to mirror the
   * `heeranjid::Error` variants the native binding surfaces:
   *   - `"invalid RanjId string: ..."` for shape failures (matches
   *     `Error::InvalidRanjIdString`).
   *   - `"uuid version must be 8 (UUIDv8)"` for version failures
   *     (matches `Error::InvalidRanjIdVersion`).
   *   - `"uuid variant must be RFC 4122"` for variant failures
   *     (matches `Error::InvalidRanjIdVariant`).
   */
  static fromString(value: string): RanjId {
    if (typeof value !== "string" || value.length !== 36) {
      throw new Error(`invalid RanjId string: ${value}`);
    }
    // Hyphens at positions 8, 13, 18, 23 (canonical 8-4-4-4-12 layout).
    if (
      value[8] !== "-" ||
      value[13] !== "-" ||
      value[18] !== "-" ||
      value[23] !== "-"
    ) {
      throw new Error(`invalid RanjId string: ${value}`);
    }
    // Every other character must be a lowercase hex digit. The native
    // parser accepts upper-case too; uuid::Uuid::parse_str is
    // case-insensitive, but the canonical Display form is lowercase.
    // Mirror that: accept both cases, normalize to lowercase below for
    // version/variant nibble checks so the comparisons are uniform.
    const normalized = value.toLowerCase();
    for (let i = 0; i < normalized.length; i++) {
      if (i === 8 || i === 13 || i === 18 || i === 23) continue;
      const c = normalized.charCodeAt(i);
      const isHex =
        (c >= 0x30 && c <= 0x39) || // 0-9
        (c >= 0x61 && c <= 0x66); // a-f
      if (!isHex) {
        throw new Error(`invalid RanjId string: ${value}`);
      }
    }
    // Version nibble: position 14 (the first hex digit of the third
    // group, e.g. the `8` in `xxxxxxxx-xxxx-8xxx-...`). UUIDv8 only.
    if (normalized[14] !== "8") {
      throw new Error("uuid version must be 8 (UUIDv8)");
    }
    // Variant bits: position 19 (the first hex digit of the fourth
    // group). High two bits must be 0b10, i.e. nibble ∈ {8, 9, a, b}.
    const variantNibble = normalized[19];
    if (
      variantNibble !== "8" &&
      variantNibble !== "9" &&
      variantNibble !== "a" &&
      variantNibble !== "b"
    ) {
      throw new Error("uuid variant must be RFC 4122");
    }
    // Store the canonical lowercase form so equality / round-trip tests
    // hold against the postgres `::text` cast (which lowercases) and
    // the bytes-decoded path (which produces lowercase via `.padStart(2, "0")`).
    return new RanjId(normalized);
  }

  /**
   * Mock equivalent of the native `fromBytes` factory.
   *
   * Validates length, UUIDv8 version nibble, and RFC 4122 variant bits
   * so the mock rejects the same inputs the real NAPI binding rejects.
   * Without these checks, mock-based tests would silently accept
   * malformed bytes that production code would throw on, hiding bugs
   * that only surface in the integration / native test pass.
   *
   * Parameter type matches the real NAPI binding, which now declares
   * `static fromBytes(bytes: Uint8Array): RanjId`. (Node `Buffer` is a
   * `Uint8Array` subclass, so callers passing a `Buffer` still satisfy
   * this signature.)
   */
  static fromBytes(bytes: Uint8Array): RanjId {
    if (!bytes || bytes.length !== 16) {
      throw new Error(
        `RanjId.fromBytes: bytes must be exactly 16 bytes, got ${bytes?.length ?? "null"}`
      );
    }
    // UUIDv8 version nibble: byte[6] high nibble must be 0x80
    // (binary 1000). Mirrors heeranjid::RanjId::from_uuid version
    // check.
    if ((bytes[6] & 0xf0) !== 0x80) {
      throw new Error(
        `RanjId.fromBytes: invalid UUIDv8 version nibble (byte[6] high nibble = 0x${(bytes[6] & 0xf0).toString(16)}, expected 0x80)`
      );
    }
    // RFC 4122 variant: byte[8] high two bits must be 0b10 (binary),
    // i.e. byte[8] & 0xc0 == 0x80. Mirrors heeranjid::RanjId::from_uuid
    // variant check.
    if ((bytes[8] & 0xc0) !== 0x80) {
      throw new Error(
        `RanjId.fromBytes: invalid UUID variant bits (byte[8] high two bits = 0x${(bytes[8] & 0xc0).toString(16)}, expected 0x80)`
      );
    }
    // Convert 16 bytes to canonical 8-4-4-4-12 lowercase hyphenated form.
    // Explicit `b: number` annotation: when @types/node is absent the
    // strict-mode inferrer cannot see Uint8Array's element type and falls
    // back to `unknown`.
    const hex = Array.from(bytes, (b: number) =>
      b.toString(16).padStart(2, "0")
    ).join("");
    const uuid = `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;
    return new RanjId(uuid);
  }

  /**
   * Mock equivalent of the native `toBytes` method.
   * Decodes the stored UUID string back into 16 big-endian bytes.
   */
  toBytes(): Uint8Array {
    const hex = this.value.replace(/-/g, "");
    if (hex.length !== 32) {
      throw new Error(`RanjId.toBytes: stored value is not a 32-hex UUID: ${this.value}`);
    }
    const out = new Uint8Array(16);
    for (let i = 0; i < 16; i++) {
      out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
    }
    return out;
  }

  toString(): string {
    return this.value;
  }
}
