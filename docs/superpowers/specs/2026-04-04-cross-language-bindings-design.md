# HeeRanjID Cross-Language Bindings Design

## Overview

Extend HeeRanjID from a Rust-only crate into a cross-language library publishable to PyPI, npm, NuGet, and crates.io. The Rust core remains the single source of truth for type behavior (parsing, decoding, validation). ID generation stays in Postgres — each binding calls `heer_generate_id()` / `ranj_generate_id()` through its own native database driver. No database connections cross FFI boundaries.

## Workspace Structure

Cargo workspace monorepo. Each binding is a separate crate/package with its own ecosystem-specific packaging.

```
HeeRanjID/
├── Cargo.toml                  # workspace manifest
├── heeranjid/                  # core Rust crate (moved from root)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── heer.rs
│   │   ├── ranj.rs
│   │   ├── postgres.rs
│   │   ├── error.rs
│   │   └── serde_helpers.rs
│   └── tests/
│       ├── postgres.rs
│       └── concurrency.rs
├── heeranjid-python/           # PyO3 binding → PyPI
│   ├── Cargo.toml
│   ├── pyproject.toml
│   ├── src/
│   │   └── lib.rs
│   ├── python/
│   │   └── heeranjid/
│   │       ├── __init__.py
│   │       ├── django/
│   │       │   ├── __init__.py
│   │       │   ├── fields.py
│   │       │   └── migrations/
│   │       └── py.typed
│   └── tests/
│       ├── test_types.py
│       └── test_django.py
├── heeranjid-node/             # NAPI-RS binding → npm
│   ├── Cargo.toml
│   ├── package.json
│   ├── src/
│   │   └── lib.rs
│   ├── js/
│   │   ├── index.ts
│   │   └── prisma/
│   │       ├── index.ts
│   │       └── setup.ts
│   ├── tests/
│   │   ├── types.test.ts
│   │   └── prisma.test.ts
│   └── tsconfig.json
├── heeranjid-ffi/              # C API → shared library
│   ├── Cargo.toml
│   ├── cbindgen.toml
│   ├── src/
│   │   └── lib.rs
│   └── tests/
│       └── test_ffi.c
├── heeranjid-dotnet/           # .NET wrapper → NuGet
│   ├── src/
│   │   └── HeeRanjID/
│   │       ├── HeeRanjID.csproj
│   │       ├── HeerId.cs
│   │       ├── RanjId.cs
│   │       ├── NativeMethods.cs
│   │       ├── EntityFramework/
│   │       │   ├── HeerIdAttribute.cs
│   │       │   └── ValueConverters.cs
│   │       └── sql/            # bundled SQL from submodule
│   ├── tests/
│   │   └── HeeRanjID.Tests/
│   │       ├── HeerIdTests.cs
│   │       └── EfCoreTests.cs
│   └── nuget/
└── sql/                        # git submodule (shared SQL source of truth)
```

## Responsibility Split

| Concern | Where it lives |
|---------|---------------|
| ID bit layout, encoding, decoding, validation | Rust core (`heeranjid/`) |
| ID generation | Postgres SQL functions (called via each ecosystem's native DB driver) |
| Type wrappers for Python | `heeranjid-python/` via PyO3 |
| Type wrappers for JS/TS | `heeranjid-node/` via NAPI-RS |
| Type wrappers for .NET | `heeranjid-dotnet/` via P/Invoke into C API |
| C ABI surface | `heeranjid-ffi/` via cbindgen |
| SQL schema & functions | `sql/` submodule, bundled into each binding's migrations |

## Binding 1: Python (heeranjid-python)

### Build Tooling

- **PyO3** compiles Rust directly into a Python native extension
- **maturin** as the PEP 517 build backend — handles wheel building and `pip install` from source
- Published to **PyPI** as `heeranjid`

### Python API

```python
from heeranjid import HeerId, RanjId

# Parse and inspect
hid = HeerId(7289942584137728001)
hid.timestamp     # datetime
hid.node_id       # int
hid.sequence      # int
str(hid)          # "7289942584137728001"

rid = RanjId("0196038a-5e6c-7001-8000-000000000001")
rid.timestamp     # datetime
rid.node_id       # int
rid.sequence      # int
rid.to_uuid()     # uuid.UUID
```

The `HeerId` and `RanjId` Python classes are PyO3 wrappers around the Rust types. They expose read-only properties for decoded fields. No generation methods — generation happens through the ORM/DB driver.

### Django Integration

```python
from heeranjid.django import HeerIdField, RanjIdField

class Order(models.Model):
    id = HeerIdField(primary_key=True)

class Event(models.Model):
    id = RanjIdField(primary_key=True)
```

- `HeerIdField` extends `BigIntegerField` — stores as `BIGINT`, returns `HeerId` instances, default is `heer_generate_id()`
- `RanjIdField` extends `UUIDField` — stores as `UUID`, returns `RanjId` instances, default is `ranj_generate_id()`
- Django migration installs the Postgres schema and functions from bundled SQL
- Django is an **optional dependency**: `pip install heeranjid[django]`

### Version Targets

- Python >= 3.10
- Django >= 4.2 LTS (optional)
- Type stubs included (`py.typed`, PEP 561)

### Package Config

```toml
# pyproject.toml
[build-system]
requires = ["maturin>=1.0"]
build-backend = "maturin"

[project]
name = "heeranjid"
requires-python = ">=3.10"

[project.optional-dependencies]
django = ["django>=4.2"]
```

## Binding 2: JavaScript/TypeScript (heeranjid-node)

### Build Tooling

- **NAPI-RS** compiles Rust into a Node.js native addon (`.node` file)
- All JS-side code written in **TypeScript**
- Published to **npm** as `heeranjid`
- Prebuilt binaries for linux-x64, linux-arm64, darwin-x64, darwin-arm64, win32-x64

### TypeScript API

```typescript
import { HeerId, RanjId } from 'heeranjid';

// Parse and inspect
const hid = HeerId.fromBigInt(7289942584137728001n);
hid.timestamp;   // Date
hid.nodeId;      // number
hid.sequence;    // number
hid.toString();  // "7289942584137728001"
hid.toBigInt();  // BigInt

const rid = RanjId.fromString("0196038a-5e6c-7001-8000-000000000001");
rid.timestamp;   // Date
rid.nodeId;      // number
rid.sequence;    // number
rid.toUUID();    // string
```

HeerId raw values use `BigInt` since 64-bit integers exceed `Number.MAX_SAFE_INTEGER`. String serialization by default.

### Prisma Integration

Prisma lacks custom field types, so integration is a **client extension** that wraps query results:

```prisma
model Order {
  id BigInt @id @default(dbgenerated("heer_generate_id()"))
}

model Event {
  id String @id @default(dbgenerated("ranj_generate_id()")) @db.Uuid
}
```

```typescript
import { heeranjidExtension } from 'heeranjid/prisma';

const prisma = new PrismaClient().$extends(heeranjidExtension());
```

- SQL setup helper script to install Postgres schema and functions
- Prisma is an **optional peer dependency**

## Binding 3: C API (heeranjid-ffi)

### Purpose

Expose core type operations via a stable C ABI. Primary consumer is the .NET binding via P/Invoke. Also usable by any language with C FFI support.

### C API Surface

```c
// heeranjid.h (auto-generated via cbindgen)

// Types
typedef int64_t heer_id_t;
typedef struct { uint8_t bytes[16]; } ranj_id_t;

// HeerId operations
int heer_id_decode(heer_id_t id, int64_t *timestamp_ms, int32_t *node_id, int32_t *sequence);
int heer_id_from_string(const char *s, heer_id_t *out);
int heer_id_to_string(heer_id_t id, char *buf, size_t buf_len);

// RanjId operations
int ranj_id_decode(const ranj_id_t *id, int64_t *timestamp_us, int32_t *node_id, int32_t *sequence);
int ranj_id_from_string(const char *s, ranj_id_t *out);
int ranj_id_to_string(const ranj_id_t *id, char *buf, size_t buf_len);

// Error handling
const char *heer_last_error(void);
```

No generator types, no connection handles. Decode/encode only.

### Key Decisions

- **cbindgen** auto-generates the `.h` header from Rust source
- Functions return `0` for success, negative for error
- Thread-local error string via `heer_last_error()`
- Builds as shared library (`libheeranjid.so` / `.dylib` / `.dll`)

## Binding 4: .NET (heeranjid-dotnet)

### Build Tooling

- P/Invoke calls into `heeranjid-ffi` shared library
- Native library bundled in NuGet package via `runtimes/` folders
- Published to **NuGet** as `HeeRanjID`
- Target **.NET 8** (LTS, supported until November 2026)

### C# API

```csharp
using HeeRanjID;

var hid = new HeerId(7289942584137728001L);
hid.Timestamp;   // DateTimeOffset
hid.NodeId;      // int
hid.Sequence;    // int

var rid = RanjId.Parse("0196038a-5e6c-7001-8000-000000000001");
rid.Timestamp;   // DateTimeOffset
rid.ToGuid();    // Guid
```

### EF Core Integration

```csharp
public class Order
{
    [HeerId]
    public long Id { get; set; }
}

public class Event
{
    [RanjId]
    public Guid Id { get; set; }
}

modelBuilder.Entity<Order>()
    .Property(e => e.Id)
    .HasDefaultValueSql("heer_generate_id()");
```

- Value converters for EF Core to round-trip `HeerId`/`RanjId` types
- SQL migration helper to install Postgres schema and functions
- EF Core integration included in the main package

## Implementation Order

Parallel execution via subagents after workspace restructure:

```
1. Workspace Restructure (sequential, prerequisite)
   └── Move core crate into heeranjid/, set up workspace Cargo.toml

2. Parallel (independent subagents):
   ├── Agent 1: heeranjid-python (PyO3 + Django)
   ├── Agent 2: heeranjid-node (NAPI-RS + Prisma)
   └── Agent 3: heeranjid-ffi (C API) + heeranjid-dotnet (.NET + EF Core)
```

## What Bindings Do NOT Do

- No ID generation through Rust — each binding calls Postgres functions via its own DB driver
- No `sqlx` or Rust database connections crossing FFI boundaries
- No reimplementation of bit layouts — all decoding delegates to the Rust core
