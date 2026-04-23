# HeerId Format

HeerId is the default HeeRanjID format: a 64-bit, time-ordered integer that packs a millisecond timestamp, node ID, and sequence counter into a single `bigint` value.

It follows a Snowflake-style structure and is the natural starting point for most systems.

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

## Upgrade path

HeerId supports up to 511 nodes and 8,191 IDs per node per millisecond. When a system grows beyond these limits — or requires sub-millisecond precision — it can migrate to RanjId. The conversion is lossless: every HeerId maps to exactly one RanjId.

See [conversion rules](./conversion.md) for details.

---

## Descending variant

`HeerIdDesc` is the reverse-chronologically-sorted sibling of `HeerId`. Use it when the natural read pattern for a table is "newest first" and you want `ORDER BY id DESC` to become a plain `ORDER BY id` that a B-tree index can serve without a reverse scan — for example, on audit logs, activity feeds, or event streams where the most recent rows dominate reads.

`HeerIdDesc` is a separate type, not a mode flag: a column is asc or desc at schema time and never mixed. Conversion between the two directions is a pure XOR against a flip mask that preserves the node field and bit 63, so values round-trip losslessly and Rust `Ord` on `HeerIdDesc` agrees bit-for-bit with Postgres `BIGINT` signed comparison. `Vec<HeerIdDesc>::sort()` therefore produces reverse-chronological order that matches `SELECT ... ORDER BY id` from a desc column.

See the bit-layout reference for the exact mask: [`docs/reference/bit-layout.md`](../reference/bit-layout.md#descending-flip-mask-variant). The design spec (local-only, gitignored) lives at `docs/superpowers/specs/2026-04-22-descending-sort-ids-design.md`. For converting an existing asc column to desc under live writes, follow the playbook at [`docs/migrations/asc-to-desc.md`](../migrations/asc-to-desc.md).
