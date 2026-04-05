# Overview

HeeRanjID is a Snowflake-style ID system designed to work consistently across languages and databases.

It provides time-ordered, compact identifiers for internal use, along with a UUID-compatible representation for interoperability across systems.

---

## Motivation

Most systems today choose between a few common approaches for identifiers:

* **Auto-increment integers** — efficient and compact, but not globally unique
* **UUIDs** — globally unique and portable, but larger and less efficient for indexing
* **Snowflake-style IDs** — time-ordered and compact, but often tied to specific languages or infrastructure

In practice, this creates tradeoffs between performance, portability, and consistency across systems.

HeeRanjID is designed to address these tradeoffs by combining:

* A compact, time-ordered identifier for storage and indexing
* A portable, UUID-compatible identifier for external use
* A consistent format that can be used across multiple languages
* Support for both application-level and database-backed ID generation

---

## ID Model

HeeRanjID defines two related identifier formats:

### HeerId

A 64-bit, time-ordered integer identifier.

HeerId is optimized for:

* Database storage efficiency
* Index performance
* Ordered insertion patterns

It is intended for internal use within a system.

---

### RanjId

A 128-bit, UUID-compatible identifier.

RanjId is designed for:

* APIs and external interfaces
* Cross-system communication
* Interoperability with UUID-based tooling

---

## Dual Representation

HeerId and RanjId represent the same underlying identity in different forms.

This allows a system to:

* Store compact, efficient IDs internally (HeerId)
* Expose portable, standard identifiers externally (RanjId)

Conversion between the two formats is supported where possible.

This separation avoids forcing a single identifier format to satisfy all use cases.

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

This is particularly useful in systems where multiple processes or services need to generate IDs without collisions.

---

## Cross-language design

HeeRanjID defines a consistent identifier format and provides implementations across multiple ecosystems.

The core logic is implemented in Rust, with bindings and integrations for:

* Python (Django)
* TypeScript / Prisma
* .NET
* C (FFI)

This allows the same ID system to be used across different parts of a system without redefining behavior in each language.

---

## Design goals

HeeRanjID is designed with the following goals:

* **Efficiency** — compact identifiers with good indexing characteristics
* **Ordering** — time-based ordering for insert-heavy workloads
* **Interoperability** — compatibility with UUID-based systems
* **Consistency** — a shared format across languages and environments
* **Flexibility** — support for both application-level and database-backed generation

---

## Further Reading

* [HeerId format](./id-formats/heerid.md)
* [RanjId format](./id-formats/ranjid.md)
* [Conversion rules](./id-formats/conversion.md)
* [Generation algorithm](./generation/algorithm.md)
* [Database generation](./generation/database-generation.md)

