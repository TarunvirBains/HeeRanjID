# heeranjid-sqlx

PostgreSQL and SQLx integration for HeerRanjId.

This crate exposes:

- embedded SQL for schema installation and ID generation
- helpers for startup validation
- helpers for generating `HeerId` and `RanjId` values from PostgreSQL

```rust
use heeranjid_sqlx::install_schema;

# async fn demo(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
install_schema(pool).await?;
# Ok(())
# }
```

The repository uses a `sql/` submodule for the embedded SQL sources. Make sure
you clone with submodules or run `git submodule update --init --recursive`
before building from a git checkout.
