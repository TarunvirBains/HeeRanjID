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

## Summary

HeerId provides a compact, time-ordered 64-bit identifier optimized for storage and indexing.

RanjId provides a UUID-compatible 128-bit representation for interoperability.

This document defines the canonical encoding used across all HeeRanjID implementations.
