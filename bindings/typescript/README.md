# heeranjid

> **HeerRanjId** ([ɦiːɾ.ɾaːnd͡ʒ.ɪd])

Node.js / TypeScript bindings for HeerRanjId — a Rust Snowflake-style distributed ID generator with Postgres and MSSQL support.

The package exposes:

- `HeerId`: compact 64-bit time-ordered `bigint` identifier (stored as `bigint` on both Postgres and MSSQL)
- `RanjId`: UUIDv8-compatible 128-bit identifier with sub-millisecond precision (stored as `uuid` on Postgres, `BINARY(16)` on MSSQL to preserve big-endian sort order)

```typescript
import { HeerId, RanjId } from 'heeranjid'

const hid = HeerId.fromString('137438953472')
console.log(hid.timestampMs, hid.nodeId, hid.sequence)

const rid = RanjId.fromString('00000000-0000-8000-8007-a120006400c8')
console.log(rid.toUuid(), rid.nodeId)
```

## Installation

```bash
npm install heeranjid
```

When building from a git checkout, initialize submodules first:

```bash
git submodule update --init --recursive
```

## Notes on MSSQL support

Schema installation via the `heeranjid-prisma` extension supports both Postgres and MSSQL (pass `"mssql"` to `install()`). However, the `generateRanjId` / `generateRanjIds` methods in the Prisma extension are currently Postgres-only — they use Postgres `::text` cast syntax and parse UUID strings, which does not work against an MSSQL backend. MSSQL `BINARY(16)` round-trip for RanjId generation is not yet implemented in the TypeScript bindings. See the Rust crate and `.NET` bindings for full MSSQL parity today.
