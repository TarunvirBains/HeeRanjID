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
