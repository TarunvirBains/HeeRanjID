# Database-backed Generation

HeeRanjID supports generating identifiers directly within the database.

This provides a centralized and consistent mechanism for ID allocation across multiple services and processes.

---

## Overview

In addition to application-level generation, IDs can be generated inside the database (e.g. PostgreSQL).

This allows:

* Multiple services to share a common ID generation source
* Consistent behavior across different languages
* Efficient allocation of IDs for batch operations

---

## Why database generation?

Traditional Snowflake-style systems typically generate IDs in application code.

While this works well in many cases, it introduces challenges:

* Coordinating node identifiers across services
* Ensuring consistent behavior across languages
* Managing high-throughput bulk inserts

Database-backed generation addresses these issues by moving coordination into the database.

---

## How it works

HeeRanjID provides database functions and queries that generate IDs using the same underlying format as the application-level implementation.

These functions:

* Use database-side state (e.g. sequences or counters)
* Generate time-ordered IDs
* Ensure uniqueness without requiring per-service coordination

The Rust `heeranjid-sqlx` crate provides access to these functions.

---

## Batch allocation

One of the primary benefits of database-backed generation is efficient batch allocation.

Instead of generating IDs one at a time, a client can request multiple IDs in a single operation.

This enables:

* Reduced round trips between application and database
* Faster bulk insert operations
* Better throughput under load

---

## Integration with application code

### Rust (SQLx)

The `heeranjid-sqlx` crate provides functions for:

* Generating individual IDs
* Generating batches of IDs
* Integrating with asynchronous workflows

---

### Django

The Django integration uses database-backed generation to support:

* Automatic primary key assignment
* Prefetching IDs for bulk operations
* Efficient `bulk_create` workflows

Custom managers can request batches of IDs and assign them before inserting records.

---

## Coordination model

Database-backed generation centralizes coordination:

* The database ensures uniqueness
* No need to manually assign node IDs per service
* Multiple services can safely generate IDs concurrently

This is particularly useful in distributed systems with multiple writers.

---

## Tradeoffs

### Advantages

* Centralized coordination
* Cross-language consistency
* Efficient batching
* Simplified application logic

---

### Considerations

* Introduces dependency on database availability
* May increase load on the database under high throughput
* Requires database-specific setup

---

## When to use database generation

Database-backed generation is a good fit when:

* Multiple services need to generate IDs
* Consistency across languages is important
* Bulk insert performance matters
* Central coordination is preferred

---

## When to use application-level generation

Application-level generation may be preferable when:

* Low-latency generation is required without database access
* Systems are loosely coupled
* Node-based coordination is acceptable

---

## Summary

HeeRanjID supports both application-level and database-backed ID generation.

Database-backed generation provides a centralized, consistent, and efficient approach, particularly for multi-service systems and bulk workloads.

For details on the generation algorithm, see [generation algorithm](./algorithm.md).
