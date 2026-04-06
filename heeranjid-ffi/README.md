# heeranjid-ffi

C FFI bindings for HeeRanjID.

This crate builds a shared library that exposes the core identifier types to C
and other FFI consumers.

The generated header is produced during the build with `cbindgen`.

For the Rust-native API, use `heeranjid`. For PostgreSQL helpers, use
`heeranjid-sqlx`.
