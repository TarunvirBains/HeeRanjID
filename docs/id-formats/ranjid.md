# RanjId Format

RanjId is a 128-bit, UUID-compatible identifier used for interoperability across systems.

It provides a portable representation of identity that can be used in APIs, external integrations, and environments where UUIDs are expected.

---

## Overview

RanjId is designed to:

* Be compatible with UUID-based systems
* Preserve identity across language and system boundaries
* Support conversion to and from HeerId where possible

It serves as the external-facing representation of IDs in HeeRanjID.

---

## UUID Compatibility

RanjId conforms to a 128-bit structure compatible with UUID storage and tooling.

This allows it to be:

* Stored in UUID columns (e.g. PostgreSQL)
* Serialized using standard UUID formats
* Used with libraries and frameworks that expect UUIDs

---

## Relationship to HeerId

RanjId can represent the same underlying identity as a HeerId.

Typical flow:

* Internal systems generate and store **HeerId**
* External interfaces expose **RanjId**

Conversion between the two formats allows systems to move between efficient internal storage and portable external representation.

See [conversion rules](./conversion.md) for details.

---

## Structure

RanjId is a 128-bit identifier.

It may encode:

* Timestamp information
* Node or generator information
* Sequence or uniqueness data

The exact encoding is designed to allow compatibility with HeerId where possible, while remaining usable as a standalone identifier.

See [bit layout reference](../reference/bit-layout.md) for details.

---

## Usage

RanjId is typically used for:

* Public APIs
* External integrations
* Cross-service communication
* Systems that require UUID-compatible identifiers

---

## Storage

Depending on the database:

* **PostgreSQL** — stored as `UUID`
* **MSSQL** — stored as `BINARY(16)`

This ensures efficient storage while maintaining compatibility.

---

## Conversion Considerations

Not all RanjId values can be converted back into HeerId.

Conversion depends on whether the RanjId preserves the necessary components and fits within HeerId constraints.

See [conversion rules](./conversion.md) for details.

---

## Advantages

* UUID-compatible and widely supported
* Portable across systems and languages
* Suitable for external-facing use
* Can interoperate with HeerId

---

## Considerations

* Larger than HeerId (128-bit vs 64-bit)
* Less efficient for indexing compared to HeerId
* May not always be convertible back to HeerId

---

## Summary

RanjId provides a portable, UUID-compatible identifier for use outside the core system.

It complements HeerId by enabling interoperability without sacrificing internal performance.

See [HeerId format](./heerid.md) for the internal representation.
