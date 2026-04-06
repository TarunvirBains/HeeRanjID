# heeranjid-ffi

> **Pronunciation:** *"Heer-Ranj-Id"* — named after Heer and Ranjha, the star-crossed lovers of the classic Punjabi folk tale. `HeerId` from Heer; `RanjId` from Ranjha.

C FFI bindings for HeerRanjId.

This crate builds a shared library that exposes the core identifier types to C
and other FFI consumers.

The generated header is produced during the build with `cbindgen`.

For the Rust-native API, use `heeranjid`. For PostgreSQL helpers, use
`heeranjid-sqlx`.
