# heeranjid

> **HeerRanjId** ([ɦiːɾ.ɾaːnd͡ʒ.ɪd])

Node.js / TypeScript bindings for HeerRanjId — a Snowflake-style distributed ID system.

The package exposes:

- `HeerId` for compact 64-bit time-ordered identifiers
- `RanjId` for UUIDv8-compatible 128-bit identifiers with sub-millisecond precision

```typescript
import { HeerId, RanjId } from 'heeranjid'

const hid = HeerId.fromString('137438953472')
console.log(hid.timestampMs, hid.nodeId, hid.sequence)

const rid = RanjId.fromString('00000000-0000-8000-8007-a120006400c8')
console.log(rid.toUuid(), rid.nodeId)
```

When building from a git checkout, initialize submodules first:

```bash
git submodule update --init --recursive
```
