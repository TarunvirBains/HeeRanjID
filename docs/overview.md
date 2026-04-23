# Overview

**HeeRanjID is designed to let a project start on a single Postgres node with a compact 8-byte integer PK, then migrate to distributed writers later without rewriting a single ID.**

`HeerId` is the primary type: a 64-bit time-ordered integer whose bit layout already carries a `node_id` field on day one, even when there's only one writer. When you later need multiple writers, you allocate more `node_id` values in `heer_nodes` and bind each service's session — no schema change, no ID-format change, no application migration. Existing IDs remain valid forever.

`RanjId` is the natural extension when `HeerId`'s capacity stops being enough: a 128-bit UUIDv8-compatible identifier with wider node and sequence fields plus sub-millisecond precision. Converting `HeerId → RanjId` is lossless, so you can grow through both tiers without rewriting data.

Both formats ship with reverse-chronologically-sorted siblings — `HeerIdDesc` and `RanjIdDesc` — whose raw-bit ordering matches a `DESC` scan, so newest-first `ORDER BY id` is served directly by the PK index without a secondary index.

---

## Motivation

Most systems today choose between a few common approaches for identifiers:

* **Auto-increment integers** — efficient and compact, but locked to a single writer. Adding more writers later is a forklift migration that touches every ID in the database.
* **UUIDs** — globally unique and portable, but 16 bytes forever and random sub-ms tie-breaking that doesn't match any domain semantics.
* **Snowflake-style IDs** — time-ordered and compact, but usually tied to one stack or language, and usually require distributed infrastructure on day one.

HeeRanjID is a Snowflake-style system explicitly engineered for the single-node greenfield case, with a painless path to distributed:

* Start with `HeerId` on one node — behaves like a time-ordered sequence
* Scale out to multiple writers later by adding node entries — no ID or schema changes
* Upgrade to `RanjId` when 8 bytes isn't enough — lossless conversion
* Consistent encoding across Rust, Python/Django, TypeScript, .NET, and C FFI

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
