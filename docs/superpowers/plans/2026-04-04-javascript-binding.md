# JavaScript/TypeScript Binding (heeranjid-node) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a NAPI-RS-based npm package (`heeranjid`) that wraps the core Rust types (HeerId, RanjId) and provides a Prisma client extension for automatic type wrapping.

**Architecture:** NAPI-RS compiles the Rust core directly into a Node.js native addon. The native module exposes `HeerId` and `RanjId` as TypeScript classes with read-only properties. A TypeScript layer provides Prisma integration via a client extension. No ID generation in Rust — Prisma schema uses `dbgenerated()` defaults that call Postgres functions directly. All JS-side code is written in TypeScript.

**Tech Stack:** NAPI-RS, Node.js >= 18, TypeScript, vitest, Prisma (optional peer dep)

**Prerequisites:** Workspace restructure (from workspace-restructure plan) must be complete.

---

### Task 1: Scaffold the Node binding crate

**Files:**
- Create: `heeranjid-node/Cargo.toml`
- Create: `heeranjid-node/package.json`
- Create: `heeranjid-node/tsconfig.json`
- Create: `heeranjid-node/src/lib.rs`
- Create: `heeranjid-node/js/index.ts`
- Modify: root `Cargo.toml` (add to workspace members)

- [ ] **Step 1: Add heeranjid-node to workspace members**

In root `Cargo.toml`, add `"heeranjid-node"` to the members list:

```toml
[workspace]
members = ["heeranjid", "heeranjid-python", "heeranjid-node"]
resolver = "2"
```

- [ ] **Step 2: Create heeranjid-node/Cargo.toml**

```toml
[package]
name = "heeranjid-node"
version = "0.1.0"
edition = "2024"
license = "MIT"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
heeranjid = { path = "../heeranjid", default-features = false }
napi = { version = "2", features = ["napi9"] }
napi-derive = "2"
uuid = "1"

[build-dependencies]
napi-build = "2"
```

- [ ] **Step 3: Create build.rs**

Create `heeranjid-node/build.rs`:

```rust
extern crate napi_build;

fn main() {
    napi_build::setup();
}
```

- [ ] **Step 4: Create package.json**

```json
{
  "name": "heeranjid",
  "version": "0.1.0",
  "description": "Distributed ID generation — HeerId (64-bit) and RanjId (128-bit UUIDv7)",
  "main": "js/index.ts",
  "types": "js/index.d.ts",
  "license": "MIT",
  "napi": {
    "name": "heeranjid",
    "triples": {
      "defaults": true
    }
  },
  "scripts": {
    "build": "napi build --platform --release",
    "build:debug": "napi build --platform",
    "test": "vitest run"
  },
  "devDependencies": {
    "@napi-rs/cli": "^3",
    "vitest": "^3",
    "typescript": "^5"
  },
  "peerDependencies": {
    "@prisma/client": ">=5.0.0"
  },
  "peerDependenciesMeta": {
    "@prisma/client": {
      "optional": true
    }
  },
  "files": [
    "js/**/*.ts",
    "js/**/*.d.ts",
    "js/**/*.js",
    "sql/**/*.sql"
  ]
}
```

- [ ] **Step 5: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "Node16",
    "moduleResolution": "Node16",
    "strict": true,
    "esModuleInterop": true,
    "declaration": true,
    "outDir": "dist",
    "rootDir": "."
  },
  "include": ["js/**/*.ts", "tests/**/*.ts"]
}
```

- [ ] **Step 6: Create minimal src/lib.rs**

```rust
#[macro_use]
extern crate napi_derive;

use napi::bindgen_prelude::*;
```

- [ ] **Step 7: Create js/index.ts**

```typescript
export { HeerId } from './heerid';
export { RanjId } from './ranjid';
```

Note: This will fail to compile until we create the heerid.ts and ranjid.ts files in the next tasks. That's expected.

- [ ] **Step 8: Install dependencies and verify the scaffold builds**

```bash
cd heeranjid-node && npm install && npm run build:debug
```

Expected: Native addon `.node` file is produced (with warnings about missing exports — that's fine)

- [ ] **Step 9: Commit**

```bash
git add heeranjid-node/ Cargo.toml
git commit -m "feat: scaffold heeranjid-node crate with NAPI-RS"
```

---

### Task 2: Implement HeerId NAPI wrapper

**Files:**
- Modify: `heeranjid-node/src/lib.rs`
- Create: `heeranjid-node/js/heerid.ts`
- Create: `heeranjid-node/tests/heerid.test.ts`

- [ ] **Step 1: Write the failing test**

Create `heeranjid-node/tests/heerid.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';
import { HeerId } from '../js/index';

describe('HeerId', () => {
  it('constructs from BigInt', () => {
    const hid = HeerId.fromBigInt(0n);
    expect(hid.toBigInt()).toBe(0n);
  });

  it('rejects negative values', () => {
    expect(() => HeerId.fromBigInt(-1n)).toThrow('non-negative');
  });

  it('decodes parts', () => {
    // timestamp=1000, node=5, sequence=42
    const raw = BigInt((1000 << 22) | (5 << 13) | 42);
    const hid = HeerId.fromBigInt(raw);
    expect(hid.timestampMs).toBe(1000);
    expect(hid.nodeId).toBe(5);
    expect(hid.sequence).toBe(42);
  });

  it('converts to string', () => {
    const hid = HeerId.fromBigInt(12345n);
    expect(hid.toString()).toBe('12345');
  });

  it('parses from string', () => {
    const hid = HeerId.fromString('12345');
    expect(hid.toBigInt()).toBe(12345n);
  });

  it('rejects garbage strings', () => {
    expect(() => HeerId.fromString('not_a_number')).toThrow();
  });

  it('supports equality check', () => {
    const a = HeerId.fromBigInt(100n);
    const b = HeerId.fromBigInt(100n);
    expect(a.toBigInt()).toBe(b.toBigInt());
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd heeranjid-node && npm run build:debug && npx vitest run tests/heerid.test.ts`
Expected: FAIL — HeerId not yet implemented

- [ ] **Step 3: Implement HeerId in Rust**

Update `heeranjid-node/src/lib.rs`:

```rust
#[macro_use]
extern crate napi_derive;

use napi::bindgen_prelude::*;

#[napi]
pub struct HeerId {
    inner: heeranjid::HeerId,
}

#[napi]
impl HeerId {
    #[napi(factory)]
    pub fn from_big_int(value: BigInt) -> Result<Self> {
        let (signed, raw, _) = value.get_i64();
        if signed {
            return Err(Error::from_reason("heerid must be non-negative"));
        }
        let inner = heeranjid::HeerId::from_i64(raw)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(Self { inner })
    }

    #[napi(factory)]
    pub fn from_string(s: String) -> Result<Self> {
        let inner: heeranjid::HeerId = s
            .parse()
            .map_err(|e: heeranjid::Error| Error::from_reason(e.to_string()))?;
        Ok(Self { inner })
    }

    #[napi]
    pub fn to_big_int(&self, env: Env) -> Result<BigInt> {
        env.create_bigint_from_i64(self.inner.as_i64())
    }

    #[napi(getter)]
    pub fn timestamp_ms(&self) -> f64 {
        self.inner.timestamp_ms() as f64
    }

    #[napi(getter)]
    pub fn node_id(&self) -> u16 {
        self.inner.node_id()
    }

    #[napi(getter)]
    pub fn sequence(&self) -> u16 {
        self.inner.sequence()
    }

    #[napi]
    pub fn to_string_value(&self) -> String {
        self.inner.to_string()
    }
}
```

- [ ] **Step 4: Create the TypeScript re-export**

Create `heeranjid-node/js/heerid.ts`:

```typescript
// Re-export HeerId from the native module
// The native module is loaded by NAPI-RS at the package root
const native = require('../heeranjid.node');

export const HeerId = native.HeerId;
export type HeerId = InstanceType<typeof native.HeerId>;
```

Note: The exact import path for the native module may vary depending on NAPI-RS version. Adjust the require path based on what `npm run build:debug` produces.

- [ ] **Step 5: Run test to verify it passes**

Run: `cd heeranjid-node && npm run build:debug && npx vitest run tests/heerid.test.ts`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add heeranjid-node/src/lib.rs heeranjid-node/js/heerid.ts heeranjid-node/tests/heerid.test.ts
git commit -m "feat(node): implement HeerId wrapper with NAPI-RS"
```

---

### Task 3: Implement RanjId NAPI wrapper

**Files:**
- Modify: `heeranjid-node/src/lib.rs`
- Create: `heeranjid-node/js/ranjid.ts`
- Create: `heeranjid-node/tests/ranjid.test.ts`

- [ ] **Step 1: Write the failing test**

Create `heeranjid-node/tests/ranjid.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';
import { RanjId } from '../js/index';

describe('RanjId', () => {
  const VALID_UUID = '00000000-0f42-7040-8000-006400c8';

  it('parses from string', () => {
    const rid = RanjId.fromString(VALID_UUID);
    expect(rid).toBeDefined();
  });

  it('rejects non-v7 UUIDs', () => {
    expect(() => RanjId.fromString('550e8400-e29b-41d4-a716-446655440000')).toThrow('version');
  });

  it('decodes parts', () => {
    const rid = RanjId.fromString(VALID_UUID);
    expect(rid.timestampMicros).toBeCloseTo(1_000_000);
    expect(rid.nodeId).toBe(100);
    expect(rid.sequence).toBe(200);
  });

  it('converts to UUID string', () => {
    const rid = RanjId.fromString(VALID_UUID);
    expect(rid.toUUID()).toBe(VALID_UUID);
  });

  it('converts to string', () => {
    const rid = RanjId.fromString(VALID_UUID);
    expect(rid.toStringValue()).toBe(VALID_UUID);
  });

  it('rejects garbage strings', () => {
    expect(() => RanjId.fromString('not-a-uuid')).toThrow();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd heeranjid-node && npm run build:debug && npx vitest run tests/ranjid.test.ts`
Expected: FAIL — RanjId not yet implemented

- [ ] **Step 3: Implement RanjId in Rust**

Add to `heeranjid-node/src/lib.rs`:

```rust
#[napi]
pub struct RanjId {
    inner: heeranjid::RanjId,
}

#[napi]
impl RanjId {
    #[napi(factory)]
    pub fn from_string(s: String) -> Result<Self> {
        let inner: heeranjid::RanjId = s
            .parse()
            .map_err(|e: heeranjid::Error| Error::from_reason(e.to_string()))?;
        Ok(Self { inner })
    }

    #[napi]
    pub fn to_uuid(&self) -> String {
        self.inner.as_uuid().to_string()
    }

    #[napi(getter)]
    pub fn timestamp_micros(&self) -> f64 {
        self.inner.timestamp_micros() as f64
    }

    #[napi(getter)]
    pub fn node_id(&self) -> u16 {
        self.inner.node_id()
    }

    #[napi(getter)]
    pub fn sequence(&self) -> u16 {
        self.inner.sequence()
    }

    #[napi]
    pub fn to_string_value(&self) -> String {
        self.inner.to_string()
    }
}
```

- [ ] **Step 4: Create the TypeScript re-export**

Create `heeranjid-node/js/ranjid.ts`:

```typescript
const native = require('../heeranjid.node');

export const RanjId = native.RanjId;
export type RanjId = InstanceType<typeof native.RanjId>;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd heeranjid-node && npm run build:debug && npx vitest run`
Expected: All tests PASS (both heerid and ranjid)

- [ ] **Step 6: Commit**

```bash
git add heeranjid-node/src/lib.rs heeranjid-node/js/ranjid.ts heeranjid-node/tests/ranjid.test.ts
git commit -m "feat(node): implement RanjId wrapper with NAPI-RS"
```

---

### Task 4: Prisma client extension

**Files:**
- Create: `heeranjid-node/js/prisma/index.ts`
- Create: `heeranjid-node/js/prisma/setup.ts`
- Create: `heeranjid-node/tests/prisma.test.ts`

- [ ] **Step 1: Write the failing test**

Create `heeranjid-node/tests/prisma.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';

// Test the extension factory without a real Prisma client
describe('Prisma extension', () => {
  it('exports heeranjidExtension function', async () => {
    const { heeranjidExtension } = await import('../js/prisma/index');
    expect(typeof heeranjidExtension).toBe('function');
  });

  it('extension returns a Prisma extension config', async () => {
    const { heeranjidExtension } = await import('../js/prisma/index');
    const ext = heeranjidExtension();
    expect(ext).toBeDefined();
    expect(ext.name).toBe('heeranjid');
  });
});

describe('SQL setup helper', () => {
  it('exports getInstallSQL function', async () => {
    const { getInstallSQL } = await import('../js/prisma/setup');
    expect(typeof getInstallSQL).toBe('function');
  });

  it('returns SQL string', async () => {
    const { getInstallSQL } = await import('../js/prisma/setup');
    const sql = getInstallSQL();
    expect(sql).toContain('CREATE TABLE');
    expect(sql).toContain('heer_nodes');
    expect(sql).toContain('generate_id');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd heeranjid-node && npm run build:debug && npx vitest run tests/prisma.test.ts`
Expected: FAIL — modules don't exist yet

- [ ] **Step 3: Implement Prisma extension**

Create `heeranjid-node/js/prisma/index.ts`:

```typescript
import { HeerId } from '../heerid';
import { RanjId } from '../ranjid';

/**
 * Prisma client extension that wraps BigInt columns as HeerId
 * and UUID columns as RanjId in query results.
 *
 * Usage:
 * ```typescript
 * import { PrismaClient } from '@prisma/client';
 * import { heeranjidExtension } from 'heeranjid/prisma';
 *
 * const prisma = new PrismaClient().$extends(heeranjidExtension());
 * ```
 */
export function heeranjidExtension() {
  return {
    name: 'heeranjid' as const,
    result: {
      $allModels: {
        toHeerId: {
          compute(data: Record<string, unknown>) {
            return (field: string): HeerId | null => {
              const value = data[field];
              if (value == null) return null;
              return HeerId.fromBigInt(BigInt(value as string | number | bigint));
            };
          },
        },
        toRanjId: {
          compute(data: Record<string, unknown>) {
            return (field: string): RanjId | null => {
              const value = data[field];
              if (value == null) return null;
              return RanjId.fromString(String(value));
            };
          },
        },
      },
    },
  };
}
```

- [ ] **Step 4: Implement SQL setup helper**

Bundle the SQL files into the npm package and provide a helper to read them.

```bash
mkdir -p heeranjid-node/sql
cp sql/postgres/schema.sql heeranjid-node/sql/
cp sql/postgres/functions/session.sql heeranjid-node/sql/
cp sql/postgres/functions/generate_heerid.sql heeranjid-node/sql/
cp sql/postgres/functions/generate_ranjid.sql heeranjid-node/sql/
cp sql/postgres/seed.sql heeranjid-node/sql/
```

Create `heeranjid-node/js/prisma/setup.ts`:

```typescript
import { readFileSync } from 'fs';
import { join } from 'path';

const SQL_DIR = join(__dirname, '..', '..', 'sql');

/**
 * Returns the full SQL script to install the HeeRanjID schema and functions.
 * Execute this against your Postgres database before using HeeRanjID.
 *
 * Usage:
 * ```typescript
 * import { getInstallSQL } from 'heeranjid/prisma/setup';
 * import { PrismaClient } from '@prisma/client';
 *
 * const prisma = new PrismaClient();
 * await prisma.$executeRawUnsafe(getInstallSQL());
 * ```
 */
export function getInstallSQL(): string {
  const files = [
    'schema.sql',
    'session.sql',
    'generate_heerid.sql',
    'generate_ranjid.sql',
  ];
  return files
    .map((f) => readFileSync(join(SQL_DIR, f), 'utf-8'))
    .join('\n');
}

/**
 * Returns the SQL to seed a default node (node_id=1).
 */
export function getSeedSQL(): string {
  return readFileSync(join(SQL_DIR, 'seed.sql'), 'utf-8');
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd heeranjid-node && npm run build:debug && npx vitest run tests/prisma.test.ts`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add heeranjid-node/js/prisma/ heeranjid-node/sql/ heeranjid-node/tests/prisma.test.ts
git commit -m "feat(node): add Prisma client extension and SQL setup helper"
```

---

### Task 5: Final verification and cleanup

**Files:**
- No new files

- [ ] **Step 1: Run all Node tests**

Run: `cd heeranjid-node && npm run build:debug && npx vitest run`
Expected: All tests PASS

- [ ] **Step 2: Run Rust workspace checks**

Run: `cargo clippy --workspace -- -D warnings`
Expected: SUCCESS

Run: `cargo fmt --all --check`
Expected: SUCCESS

- [ ] **Step 3: Verify the package builds for release**

Run: `cd heeranjid-node && npm run build`
Expected: Release-mode native addon produced

- [ ] **Step 4: Commit any remaining changes**

```bash
git add -A
git commit -m "chore(node): final cleanup for heeranjid-node package"
```
