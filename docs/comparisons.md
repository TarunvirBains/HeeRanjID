# Comparisons

When choosing a database primary key strategy, you have several options. This document compares HeeRanjID to other common identifier formats to help you understand the practical tradeoffs for ordering, indexing, distributed generation, and cross-language compatibility.

## HeeRanjID vs Snowflake
Classic Twitter Snowflake IDs and their derivatives are 64-bit integers combining a timestamp, a worker/node ID, and a sequence number.

* **Similarities:** `HeerId` is essentially a modernized Snowflake format optimized for Postgres. Both use 64 bits, embed timestamps, and allow distributed generation.
* **Differences:** `HeerId` uses a specific bit layout standardized across multiple languages. Unlike classic Snowflake which only offers 64-bit IDs, HeeRanjID offers a lossless upgrade path to 128-bit `RanjId` (UUIDv8) when you need more nodes, higher precision, or UUID compatibility. HeeRanjID also provides `HeerIdDesc` for newest-first sorting at the primary key index level without secondary indexes.

## HeeRanjID vs UUIDv7
UUIDv7 is a widely adopted standard for time-ordered 128-bit identifiers.

* **Similarities:** Both `RanjId` and UUIDv7 are 128 bits, time-ordered, and can be generated efficiently by distributed workers without database round trips.
* **Differences:** UUIDv7 uses random data for the lower bits to avoid collisions, whereas `RanjId` uses an explicit `node_id` and `sequence`. This makes `RanjId` collision-proof by design (as long as `node_id`s are correctly provisioned) without relying on probability. Furthermore, `HeerId` allows you to get similar benefits in only 64 bits (`bigint`), saving storage and index size for Postgres compared to 128-bit UUIDs.
* **MSSQL byte order:** UUIDv7 on MSSQL's native `uniqueidentifier` type hits a mixed-endian byte-swap that breaks the timestamp prefix's natural sort order. HeeRanjID stores `RanjId` as `BINARY(16)` on MSSQL to preserve raw big-endian bytes, so time-ordered scans work correctly on both Postgres and MSSQL without per-vendor tricks.

## HeeRanjID vs ULID
ULID is a 128-bit format that is lexicographically sortable and base32 encoded by default.

* **Similarities:** Both provide time-ordered distributed generation.
* **Differences:** ULID relies on randomness for its lower 80 bits. HeeRanjID explicitly encodes the worker node ID, removing collision probability. Furthermore, HeeRanjID offers a compact 64-bit `HeerId` variant, while ULID is fixed at 128 bits. If you want URL-safe compact string representations, HeeRanjID recommends using a presentation-layer encoding like Sqids on top of the canonical integer IDs.

## HeeRanjID vs auto-increment bigint IDs
Auto-incrementing IDs (`BIGSERIAL`, `IDENTITY`) are the default choice in many traditional database setups.

* **Similarities:** Both produce 64-bit integers (`bigint` in Postgres), which are highly efficient for storage and B-tree indexing.
* **Differences:** Auto-increment IDs require a database round-trip to generate, bottlenecking inserts on a central sequence. They don't support distributed multi-master writes without complex range coordination. They also expose the exact order and volume of creations. HeeRanjID lets clients generate IDs independently without a database round-trip, supporting horizontal scale within the provisioned node-id and per-timestamp sequence capacity while keeping the storage size at 64 bits.
* **Cross-vendor parity:** `BIGSERIAL` (Postgres) and `IDENTITY` (MSSQL) have similar central-sequence failure modes but different syntax, reseed behavior, and migration paths between them. HeeRanjID generates identical bit-layout IDs on either vendor — the same `HeerId` value is valid as either a Postgres `bigint` or an MSSQL `bigint`, and the asc↔desc migration playbooks (`docs/migrations/asc-to-desc.md`, `docs/migrations/asc-to-desc-mssql.md`) work the same way on both.

## HeeRanjID vs random UUIDs
Random UUIDs (UUIDv4) are 128-bit identifiers entirely based on random numbers.

* **Differences:** Random UUIDs are terrible for database indexing due to fragmentation. Since they are not time-ordered, inserting them into a B-tree index causes massive write amplification and page splits. HeeRanjID (`HeerId` and `RanjId`) are time-ordered, so new inserts append to the right edge of the B-tree, keeping index performance highly optimal.