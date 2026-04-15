# heeranjid

> **HeerRanjId** ([ɦiːɾ.ɾaːnd͡ʒ.ɪd])

Core Rust types for HeerRanjId.

- `HeerId`: compact 64-bit Snowflake-style identifier, stored as `bigint`
- `RanjId`: UUIDv8-compatible 128-bit upgrade format with sub-millisecond precision, stored as `uuid`
- conversion helpers between the two formats

```rust
use heeranjid::{HeerId, RanjId, RanjPrecision};

let heer = HeerId::new(1_000, 7, 42)?;
let ranj = RanjId::new(1_000_000, RanjPrecision::Microseconds, 7, 42)?;
# Ok::<(), heeranjid::Error>(())
```

For project-level documentation and database integrations, see the repository
README and docs directory.
