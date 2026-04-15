# HeerId Format

HeerId is a 64-bit, time-ordered identifier used for efficient storage and indexing within a system.

It follows a Snowflake-style structure, combining time, node identity, and a sequence counter into a single integer.

---

## Overview

A HeerId consists of three main components:

* **Timestamp** — provides ordering
* **Node identifier** — distinguishes generators
* **Sequence counter** — ensures uniqueness within the same timestamp

These components are packed into a 64-bit integer.

---

## Structure

```text
| Timestamp (41) | Node ID (9) | Sequence (13) |
```

Packed as:

```text
id = (timestamp_ms << 22) | (node_id << 13) | sequence
```

- **Timestamp**: 41 bits, millisecond value relative to the configured epoch. Max: 2,199,023,255,551 ms.
- **Node ID**: 9 bits. Max: 511.
- **Sequence**: 13 bits. Max: 8,191 IDs per node per millisecond.

See [bit layout reference](../reference/bit-layout.md) for masks, shifts, and extraction formulas.

---

## Timestamp

The timestamp component provides time-based ordering.

* Typically derived from system time
* Ensures that IDs generated later are greater than earlier ones
* Enables efficient indexing and range queries

The timestamp is relative to a defined epoch.

---

## Node Identifier

The node identifier distinguishes different ID generators.

* Allows multiple processes or machines to generate IDs independently
* Prevents collisions across distributed systems

Node IDs must be coordinated to avoid overlap.

---

## Sequence Counter

The sequence counter ensures uniqueness within the same timestamp.

* Incremented for each ID generated within a time unit
* Resets when the timestamp advances

If the sequence limit is reached within a single time unit, generation must wait for the next timestamp.

---

## Ordering Guarantees

HeerId provides **monotonic ordering per node**:

* IDs generated on the same node are strictly increasing
* Across nodes, ordering is generally time-based but not globally strict

This behavior is typical of Snowflake-style systems.

---

## Limits

The structure of HeerId imposes limits on:

* Maximum timestamp range
* Number of nodes
* Number of IDs per time unit

These limits depend on the bit allocation.

See [limits](../reference/limits.md) for exact values.

---

## Generation Model

HeerId can be generated in two ways:

### Application-level generation

* Generated directly in application code
* Requires node ID configuration
* Suitable for distributed systems without central coordination

---

### Database-backed generation

* Generated within the database (e.g. PostgreSQL)
* Centralizes coordination
* Enables batching and consistent allocation across services

---

## Advantages

* Compact (64-bit) representation
* Efficient indexing and storage
* Time-ordered for better write performance
* Suitable for high-throughput systems

---

## Considerations

* Requires coordination of node identifiers
* Sequence limits may constrain throughput per node per time unit
* Not globally strictly ordered across all nodes

---

## Summary

HeerId is a compact, time-ordered identifier designed for efficient internal use.

It provides a balance between:

* Performance (small size, ordered inserts)
* Scalability (distributed generation via node IDs)
* Simplicity (single 64-bit value)

For external interoperability, HeerId can be converted into RanjId.

See [conversion rules](./conversion.md) for details.
