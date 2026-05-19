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

## Prisma support window

`heeranjid-prisma` supports **Prisma 6.x and 7.x** (peer dep
`^6.0.0 || ^7.0.0`). Prisma 7 is the primary tested target; Prisma 6 is
supported because its runtime wire shape for `Bytes` columns is
identical (both surface `BINARY(16)` rows as a bare `Uint8Array`).
**Prisma 5 is no longer supported.** If you are on Prisma 5, pin
`heeranjid-prisma` to the previous minor and upgrade your Prisma client
on your own cadence.

## Module system

The `heeranjid-prisma` extension is **ESM-only**: the package ships
`"type": "module"` with only an ESM `main:` entry. Consumers using
CommonJS cannot `require("heeranjid-prisma")` directly — Node will raise
`ERR_REQUIRE_ESM`. CJS consumers must use dynamic `import()`:

```js
// CommonJS consumer
const { heeranjidExtension } = await import("heeranjid-prisma");
```

The base `heeranjid` package (the native NAPI binding) supports **both**
module systems via napi-rs's dual interop, so unrelated CJS code that
only touches `HeerId` / `RanjId` can keep using `require("heeranjid")`.

## Notes on MSSQL support

Full Postgres and MSSQL parity is supported. Construct the Prisma extension with the matching backend:

```typescript
import { PrismaClient } from '@prisma/client'
import { heeranjidExtension, withAutoIds } from 'heeranjid-prisma'

// Postgres
const prisma = new PrismaClient()
  .$extends(heeranjidExtension())
  .$extends(withAutoIds({
    backend: 'postgres',
    models: { User: 'heerid', Post: 'ranjid' },
  }))

// MSSQL — uses EXEC stored-procedure dispatch and BINARY(16) decoding.
const prismaMssql = new PrismaClient()
  .$extends(heeranjidExtension({ backend: 'mssql' }))
  .$extends(withAutoIds({
    backend: 'mssql',
    models: { User: 'heerid', Post: 'ranjid' },
  }))
```

The `backend` option is **required** on `withAutoIds` (and matches the
backend you pass to `heeranjidExtension`). It controls the wire shape used
when injecting generated ids into `create()` / `createMany()`: UUID string
on Postgres, `Uint8Array` (16 big-endian bytes) on MSSQL `BINARY(16)`. We
deliberately do not default this to `'postgres'` — a silent default would
let a `mssql` extension paired with a default-`postgres` `withAutoIds`
write UUID strings into a `BINARY(16)` column, and the resulting
sqlserver-driver type error would not point at the misconfiguration.
Spelling the backend out forces the mismatch to be a TypeScript compile
error instead.

For MSSQL, the extension issues `EXEC heer_set_node_id @P1`, `EXEC generate_ranjids @in_node_id = @P1, @requested_count = @P2`, etc., and decodes the returned `BINARY(16)` columns via `RanjId.fromBytes` to preserve the canonical big-endian sort order. The `RanjId.fromBytes(bytes)` / `id.toBytes()` factory and method pair on the native NAPI surface mirrors the `.NET` binding's `RanjId.FromBytes` / `ToBytes` shape and bypasses the mixed-endian swizzle that `uniqueidentifier`/`Guid` round-trips would otherwise apply.

The MSSQL `install()` path splits the bundled `.sql` scripts on `GO` batch
separators and issues each batch as a separate `$executeRawUnsafe` call,
because `GO` is a `sqlcmd`/SSMS-only batch separator and is not understood
by the ODBC/OLE DB driver Prisma uses for the `sqlserver` provider.

### Backend label convention

`heeranjid-prisma` uses the labels `"postgres"` and `"mssql"` for the
`backend` option. These intentionally **diverge from Prisma's datasource
`provider` names** (`"postgresql"` and `"sqlserver"`):

| Prisma `provider` | `heeranjid-prisma` `backend` |
| ----------------- | ---------------------------- |
| `postgresql`      | `postgres`                   |
| `sqlserver`       | `mssql`                      |

The shorter labels match the convention used throughout the HeeRanjID
workspace — the Rust crate's `mssql_schema` / `postgres_schema` modules,
the bundled SQL submodule layout (`sql/postgres/`, `sql/mssql/`), and the
.NET sibling binding's internal backend discriminator. Consumers must
translate from the Prisma provider name themselves when configuring the
extension.
