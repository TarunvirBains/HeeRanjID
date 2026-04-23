# Bit Layout Reference

This document defines the exact bit-level structure of HeerId and RanjId.

It serves as the canonical reference for encoding, decoding, and cross-language consistency.

---

## HeerId (64-bit)

HeerId is a 64-bit integer composed of three components:

```text
|  Timestamp (41)  |  Node ID (9)  |  Sequence (13)  |
|------------------|---------------|-----------------|
|      bits        |     bits      |      bits       |
```

---

## Bit Allocation

```text
Timestamp: 41 bits
Node ID:   9 bits
Sequence:  13 bits
```

---

## Bit Positions

```text
|63 ................. 0|
| Timestamp | Node | Seq |
```

---

## Packing

A HeerId is constructed as:

```text
id =
    (timestamp << 22) |
    (node_id  << 13) |
    (sequence)
```

---

## Masks and Shifts

```text
TIMESTAMP_BITS = 41
NODE_BITS      = 9
SEQUENCE_BITS  = 13

NODE_SHIFT      = 13
TIMESTAMP_SHIFT = 22
```

```text
SEQUENCE_MASK = (1 << 13) - 1 = 0x1FFF
NODE_MASK     = (1 << 9)  - 1 = 0x1FF
```

---

## Extraction

Given a HeerId:

```text
sequence  = id & 0x1FFF
node_id   = (id >> 13) & 0x1FF
timestamp = id >> 22
```

---

## Limits

```text
max_timestamp = (1 << 41) - 1 = 2199023255551
max_node_id   = (1 << 9)  - 1 = 511
max_sequence  = (1 << 13) - 1 = 8191
```

---

## Timestamp

HeerId stores a millisecond timestamp value.

The exact source of the timestamp (e.g. system time, database time, or custom epoch handling) depends on the generation method.

---

## Ordering

HeerId values are ordered by:

1. Timestamp
2. Node ID
3. Sequence

This provides monotonic ordering per node and time-based ordering across the system.

---

## Descending (flip-mask) variant

`HeerIdDesc` is the reverse-chronologically-sorted sibling of `HeerId`. It is produced by XOR-ing a `HeerId`'s stored bits against a constant flip mask that covers the timestamp and sequence fields and leaves the node field and bit 63 untouched:

```text
HEER_FLIP_MASK = (((1 << 41) - 1) << 22) | ((1 << 13) - 1)
               = 0x7FFFFFFFFFC00000 | 0x0000000000001FFF
               = 0x7FFFFFFFFFC01FFF
               = 9223372036850589695   (decimal, i64)
```

Properties:

- Bit 63 of the mask is **zero**. XORing any value whose bit 63 is zero against this mask yields another value with bit 63 zero, so `HeerIdDesc` values always round-trip cleanly through Postgres signed `BIGINT` and Rust `Ord` on `i64` agrees with Postgres signed comparison bit-for-bit.
- XOR is symmetric: `heerid_to_desc(heerid_to_asc(x)) = x` for every `x`.
- The 9-bit node field is preserved, not flipped, so a descending column still exposes the generating node directly in the stored bits.

---

# RanjId (128-bit)

RanjId is a 128-bit, UUID-compatible identifier that can encode HeerId information while remaining usable as a standard UUID.

---

## Bit Allocation

```text
Timestamp: 89 bits
Precision: 2 bits
Node ID:   15 bits
Sequence:  16 bits

UUID Version: 4 bits (fixed, version 8)
UUID Variant: 2 bits (RFC 4122)
```

### Precision field values

The 2-bit precision field encodes the timestamp unit:

```text
00  microseconds
01  nanoseconds   (default)
10  picoseconds
11  femtoseconds
```

The default is nanoseconds, overridable via the `RANJID_PRECISION` environment variable (`us`, `ns`, `ps`, `fs`). The precision is embedded in every RanjId, so the timestamp can always be decoded without external configuration.

---

## Structure Overview

RanjId distributes its fields across the 128-bit UUID layout.

The timestamp is split into multiple segments to align with UUID structure:

```text
| Timestamp (high) | Timestamp (mid) | Version | Timestamp (low) | Precision | Variant | Node ID | Sequence |
```

This layout allows RanjId to:

* Preserve ordering information
* Remain UUID-compatible
* Encode additional metadata

---

## Limits

```text
max_timestamp = (1 << 89) - 1
max_node_id   = (1 << 15) - 1 = 32767
max_sequence  = (1 << 16) - 1 = 65535
```

---

## UUID Compatibility

RanjId conforms to UUID standards:

* Version: **8** (custom UUID format)
* Variant: **RFC 4122**

This allows it to be stored and handled as a standard UUID in most systems.

---

## Conversion Constraints

When converting **RanjId → HeerId**:

* Timestamp must fit within 41 bits
* Node ID must fit within 9 bits
* Sequence must fit within 13 bits

If any of these exceed HeerId limits, conversion fails.

---

## Implementation Notes

* Bit layout must remain consistent across all language implementations
* Any change to bit allocation is a breaking change
* All bindings must follow the same encoding and decoding rules

---

## Descending (flip-mask) variant

`RanjIdDesc` is the reverse-chronologically-sorted sibling of `RanjId`. Bit positions are numbered from MSB (127) to LSB (0) below:

| Bits       | Width | Field                           | Flipped? |
|------------|-------|---------------------------------|----------|
| `127..80`  | 48    | `ts_high`                       | **Yes**  |
| `79..76`   | 4     | `version` (must be `0b1000`)    | No       |
| `75..64`   | 12    | `ts_mid`                        | **Yes**  |
| `63..62`   | 2     | `variant` (must be `0b10`)      | No       |
| `61..60`   | 2     | `precision`                     | No       |
| `59..31`   | 29    | `ts_low`                        | **Yes**  |
| `30..16`   | 15    | `node`                          | No       |
| `15..0`    | 16    | `sequence`                      | **Yes**  |

Flipped bits sum to `48 + 12 + 29 + 16 = 105` (`89` timestamp + `16` sequence). Preserved bits sum to `4 + 2 + 2 + 15 = 23`.

**Exact flip mask (128-bit):**

```text
RANJ_FLIP_MASK = 0xFFFFFFFFFFFF0FFF0FFFFFFF8000FFFF
```

Broken into nibble groups aligned to field boundaries (MSB on the left, LSB on the right):

```text
ts_high(48)          ver(4)  ts_mid(12)  var+prec(4)  ts_low(29)             node(15)        seq(16)
FFFFFFFFFFFF         0       FFF         0            FFFFFFF8                000             FFFF
```

The byte at bit position `31..24` is `F8` because bit 31 (top bit of `ts_low`) is flipped while bits 30..28 (top of `node`) are preserved, giving `1000` binary = `8` hex for that nibble. The byte at `23..16` is `00` because those bits are entirely within the preserved `node` field.

**UUIDv8 conformance preserved.** Because the `version` (4 bits) and `variant` (2 bits) fields are not flipped, a `RanjIdDesc` when stringified is still a valid UUIDv8 — any UUID-aware tool accepts it. The fact that the encoded timestamp is reverse-chronologically ordered is invisible to generic UUID tooling, which is what lets `RanjIdDesc` live in existing `uuid` columns.

---

## Summary

HeerId provides a compact, time-ordered 64-bit identifier optimized for storage and indexing.

RanjId provides a UUID-compatible 128-bit representation for interoperability.

The `HeerIdDesc` / `RanjIdDesc` siblings sort reverse-chronologically by raw bits while preserving the node field and (for RanjId) UUIDv8 compatibility.

This document defines the canonical encoding used across all HeeRanjID implementations.
