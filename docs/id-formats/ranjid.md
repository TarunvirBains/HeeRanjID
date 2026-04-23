# RanjId Format

RanjId is the scaling upgrade format in HeeRanjID: a 128-bit, UUIDv8-compatible identifier that provides higher node and sequence capacity, sub-millisecond timestamp precision, and UUID-compatible storage.

It shares the same Snowflake structure as HeerId — timestamp, node ID, sequence — so a production system can migrate from HeerId to RanjId without data loss.

---

## Capacity

| Property | HeerId | RanjId |
|---|---|---|
| Timestamp resolution | milliseconds | μs / ns / ps / fs |
| Max nodes | 511 | 32,767 |
| Max sequence / node / tick | 8,191 | 65,535 |
| Storage | `bigint` | `uuid` (Postgres), `BINARY(16)` (SQL Server) |

---

## UUID Compatibility

RanjId conforms to a 128-bit structure compatible with UUID storage and tooling.

This allows it to be:

* Stored in `uuid` columns on PostgreSQL — no column type change from `UUIDField`
* Serialized using standard UUID string format
* Used with libraries and frameworks that expect UUIDs

The UUID version is fixed at **8** (UUIDv8). Parsers that validate version bits will distinguish RanjId values from random UUIDs (v4).

---

## Relationship to HeerId

RanjId and HeerId share the same field structure, so HeerId values can always be converted into RanjId values losslessly.

The reverse — RanjId back to HeerId — succeeds only when the RanjId's node ID, timestamp, and sequence fit within HeerId's narrower limits.

See [conversion rules](./conversion.md) for exact failure conditions.

---

## Structure

RanjId is a 128-bit value with a fixed field layout that fits within the UUID wire format:

```text
| ts_high (48) | version (4) | ts_mid (12) | variant (2) | precision (2) | ts_low (29) | node_id (15) | sequence (16) |
```

- **ts_high / ts_mid / ts_low**: an 89-bit timestamp split across three UUID fields. The timestamp unit is given by the precision field.
- **version**: fixed at `0b1000` (8), marking this as UUIDv8.
- **variant**: fixed at `0b10`, RFC 4122 variant.
- **precision**: 2 bits encoding the timestamp unit — `00`=microseconds, `01`=nanoseconds (default), `10`=picoseconds, `11`=femtoseconds.
- **node_id**: 15 bits, max 32,767.
- **sequence**: 16 bits, max 65,535.

See [bit layout reference](../reference/bit-layout.md) for exact field positions and masks.

---

## Storage

Depending on the database:

* **PostgreSQL** — stored as `uuid`
* **SQL Server** — stored as `BINARY(16)` (to preserve big-endian byte order; `uniqueidentifier` would corrupt the bit layout)

---

## Conversion constraints

Not all RanjId values can be converted back to HeerId. Conversion fails if:

* `node_id > 511` (exceeds HeerId's 9-bit node field)
* timestamp in milliseconds exceeds HeerId's 41-bit range
* more than 8,192 RanjIds share the same (timestamp_ms, node_id) pair after truncating to milliseconds

See [conversion rules](./conversion.md) for details.

---

## Descending variant

`RanjIdDesc` is the reverse-chronologically-sorted sibling of `RanjId`. Use it when the natural read pattern for a UUID-stored table is "newest first" and you want `ORDER BY id DESC` to become a plain `ORDER BY id` that a B-tree index can serve without a reverse scan. Because the flip mask preserves the `version` (8) and `variant` (RFC 4122) bits, a `RanjIdDesc` stringified is still a valid UUIDv8 and lives in existing `uuid` columns alongside existing tooling.

`RanjIdDesc` is a separate type, not a mode flag: a column is asc or desc at schema time and never mixed. Conversion between the two directions is a pure XOR against a flip mask that preserves version, variant, precision, and node — so values round-trip losslessly.

**Precision-uniformity sort caveat.** The RanjId flip mask preserves the 2 precision bits (they sit between variant and `ts_low`), so those bits participate in raw-bit ordering. `Vec<RanjIdDesc>::sort()` therefore matches reverse-chronological order **only when all values share the same precision**. Mixed-precision values do not sort chronologically by raw bits. This is identical to the existing `RanjId` semantics and is the expected case under a single `RANJID_PRECISION` setting; it is called out here so callers do not assume otherwise.

See the bit-layout reference for the exact mask and nibble-field table: [`docs/reference/bit-layout.md`](../reference/bit-layout.md#descending-flip-mask-variant-1). The design spec (local-only, gitignored) lives at `docs/superpowers/specs/2026-04-22-descending-sort-ids-design.md`. For converting an existing asc column to desc under live writes, follow the playbook at [`docs/migrations/asc-to-desc.md`](../migrations/asc-to-desc.md) (Postgres) or [`docs/migrations/asc-to-desc-mssql.md`](../migrations/asc-to-desc-mssql.md) (MSSQL).

**Cross-vendor support.** Postgres (via `ranjid_next_desc()` returning `uuid`) and MSSQL (via `EXEC ranjid_next_desc @in_node_id` returning `BINARY(16)` — raw big-endian bytes, not `uniqueidentifier`, to avoid its mixed-endian byte-swap) produce bit-for-bit identical descending values. The same `RanjIdDesc` type decodes both; the v0.3.1 cross-vendor equivalence test suite enforces this invariant on every CI run.
