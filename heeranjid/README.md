# heeranjid

> **HeerRanjId** ([ɦiːɾ.ɾaːnd͡ʒ.ɪd])

Core Rust types for HeerRanjId.

- `HeerId`: compact 64-bit identifier for internal storage and indexing
- `RanjId`: UUIDv8-compatible 128-bit identifier for external interfaces
- conversion helpers between the two formats

```rust
use heeranjid::{HeerId, RanjId, RanjPrecision};

let heer = HeerId::new(1_000, 7, 42)?;
let ranj = RanjId::new(1_000_000, RanjPrecision::Microseconds, 7, 42)?;
# Ok::<(), heeranjid::Error>(())
```

For project-level documentation and database integrations, see the repository
README and docs directory.
