# Overview

HeeRanjID is a Snowflake-style ID system designed to work consistently across languages and databases.

It provides time-ordered, compact identifiers with a built-in upgrade path: start with `HeerId` and migrate to `RanjId` when you outgrow its limits — without data loss or schema disruption.

---

## Motivation

Most systems today choose between a few common approaches for identifiers:

* **Auto-increment integers** — efficient and compact, but not globally unique across nodes
* **UUIDs** — globally unique and portable, but random — they fragment indexes and carry no timing information
* **Snowflake-style IDs** — time-ordered and compact, but often tied to specific languages or infrastructure

HeeRanjID provides a Snowflake-style system that addresses these issues:

* Time-ordered IDs for efficient indexing and range queries
* Distributed generation without central coordination
* A consistent encoding across multiple languages and database backends
* A built-in upgrade path from a compact integer format to a UUID-compatible format

---

## ID Model

HeeRanjID defines two related identifier formats.

### HeerId

A 64-bit, time-ordered integer identifier (stored as `bigint`).

HeerId is the default starting point. It is optimized for:

* Compact storage — 8 bytes, fits in any `bigint` column
* Index efficiency — time-ordered, sequential inserts
* Simplicity — standard integer primary key

HeerId supports up to **511 nodes** and **8,191 IDs per node per millisecond**.

---

### RanjId

A 128-bit, UUIDv8-compatible identifier (stored as `uuid` on PostgreSQL, `BINARY(16)` on SQL Server).

RanjId is the upgrade format. It provides:

* **Higher capacity** — up to 32,767 nodes and 65,535 IDs per node per timestamp unit
* **Sub-millisecond precision** — timestamp resolution down to microseconds, nanoseconds, picoseconds, or femtoseconds
* **UUID compatibility** — accepted anywhere a UUID is expected, no column type changes needed on PostgreSQL

---

## The Upgrade Path

HeerId and RanjId share the same Snowflake structure — timestamp, node ID, and sequence — so a HeerId can always be converted into a RanjId without data loss.

This means a system can start on HeerId and migrate to RanjId when needed (node count grows, throughput exceeds HeerId's sequence limit, or sub-millisecond precision becomes necessary), without replacing existing IDs or disrupting running systems.

Conversion in the reverse direction — RanjId back to HeerId — is conditional: it succeeds only if the RanjId's values fit within HeerId's narrower limits.

See [conversion rules](./id-formats/conversion.md) for exact conditions.

---

## Generation Model

HeeRanjID follows a Snowflake-style generation approach.

Identifiers are composed using:

* A time component (for ordering)
* A node or worker identifier
* A sequence counter

This allows IDs to be generated efficiently without central coordination, while preserving ordering.

---

## Database-backed generation

In addition to application-level generation, HeeRanjID supports generating IDs directly in the database.

This enables:

* Consistent ID generation across multiple services
* Centralized coordination when needed
* Efficient batch allocation for bulk operations

---

## Cross-language design

HeeRanjID defines a consistent identifier format and provides implementations across multiple ecosystems.

The core logic is implemented in Rust, with bindings and integrations for:

* Python (Django)
* TypeScript / Prisma
* .NET
* C (FFI)

---

## Further Reading

* [HeerId format](./id-formats/heerid.md)
* [RanjId format](./id-formats/ranjid.md)
* [Conversion rules](./id-formats/conversion.md)
* [Design tradeoffs](./design/tradeoffs.md)
* [Generation algorithm](./generation/algorithm.md)
* [Database generation](./generation/database-generation.md)
