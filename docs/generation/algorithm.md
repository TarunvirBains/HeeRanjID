# Generation Algorithm

HeeRanjID uses a Snowflake-style composition model.

## HeerId

`HeerId` is packed from:

- timestamp
- node id
- sequence

The ordering properties come directly from that packed layout: timestamp first,
then node id, then sequence.

## RanjId

`RanjId` uses a UUIDv8-compatible layout with:

- a split timestamp field
- a 2-bit precision marker
- node id
- sequence

The precision marker makes the encoded timestamp self-describing.

## Database-backed Generation

The PostgreSQL integration keeps generation logic in SQL functions so multiple
applications can share the same allocation model.

See [database generation](./database-generation.md) for the operational
tradeoffs and [bit layout](../reference/bit-layout.md) for the exact field
mapping.
