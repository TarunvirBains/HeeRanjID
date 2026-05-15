//! Internal serde glue shared by the four ID wrapper types.
//!
//! ## Wire-format invariant
//!
//! Each helper enforces a two-mode contract that the wrapper `Serialize` /
//! `Deserialize` impls rely on:
//!
//! - **Human-readable formats** (e.g. `serde_json`): the serializer streams
//!   the wrapper's `Display` form as a string; the deserializer accepts a
//!   string (parsed through the wrapper's `FromStr`) and -- for i64-backed
//!   wrappers only -- a JSON integer routed directly through `from_i64`.
//! - **Non-human-readable formats** (e.g. `postcard`, `bincode`): the
//!   serializer emits the inner primitive (`i64` or `Uuid`); the
//!   deserializer decodes that exact inner primitive and revalidates it
//!   through the wrapper's `from_i64` / `from_uuid` constructor. The
//!   non-human-readable path never goes through `deserialize_any`, so
//!   schemaless wire formats like postcard remain compatible.
//!
//! The helpers are intentionally private to the crate; the four wrapper
//! types call them from their `Serialize` / `Deserialize` impls.

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;
use uuid::Uuid;

/// Serialize a wrapper as its `Display` form for human-readable formats, or
/// as the inner primitive for non-human-readable formats.
///
/// Uses `Serializer::collect_str`, which lets format-specific serializers
/// stream the `Display` output directly into their output buffer without an
/// intermediate `String` allocation (e.g. `serde_json::Serializer` writes
/// the formatter output straight to its writer).
pub(crate) fn serialize_display_or_inner<DisplayValue, Inner, S>(
    display_value: &DisplayValue,
    inner: &Inner,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    DisplayValue: fmt::Display + ?Sized,
    Inner: Serialize,
    S: Serializer,
{
    if serializer.is_human_readable() {
        serializer.collect_str(display_value)
    } else {
        inner.serialize(serializer)
    }
}

/// Deserialize an i64-backed wrapper (`HeerId`, `HeerIdDesc`).
///
/// - **Non-human-readable**: decodes an `i64` directly from the wire and
///   revalidates it through `validate` (which is the wrapper's `from_i64`).
/// - **Human-readable**: accepts a JSON string (parsed through the wrapper's
///   `FromStr`) or a JSON integer routed directly through `validate` -- no
///   intermediate string allocation for the integer case.
pub(crate) fn deserialize_i64_wrapper<'de, Value, D, F, E>(
    deserializer: D,
    validate: F,
) -> Result<Value, D::Error>
where
    Value: FromStr<Err = E>,
    F: FnOnce(i64) -> Result<Value, E>,
    E: fmt::Display,
    D: Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        let raw = i64::deserialize(deserializer)?;
        return validate(raw).map_err(de::Error::custom);
    }

    deserializer.deserialize_any(I64WrapperVisitor::<Value, F> {
        validate,
        _marker: PhantomData,
    })
}

struct I64WrapperVisitor<Value, F> {
    validate: F,
    _marker: PhantomData<Value>,
}

impl<'de, Value, F, E> Visitor<'de> for I64WrapperVisitor<Value, F>
where
    Value: FromStr<Err = E>,
    F: FnOnce(i64) -> Result<Value, E>,
    E: fmt::Display,
{
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a string or integer identifier")
    }

    fn visit_str<DE>(self, value: &str) -> Result<Self::Value, DE>
    where
        DE: de::Error,
    {
        Value::from_str(value).map_err(DE::custom)
    }

    fn visit_i64<DE>(self, value: i64) -> Result<Self::Value, DE>
    where
        DE: de::Error,
    {
        (self.validate)(value).map_err(DE::custom)
    }

    fn visit_u64<DE>(self, value: u64) -> Result<Self::Value, DE>
    where
        DE: de::Error,
    {
        let signed = i64::try_from(value)
            .map_err(|_| DE::custom(format_args!("integer {value} exceeds i64 range")))?;
        (self.validate)(signed).map_err(DE::custom)
    }
}

/// Deserialize a `Uuid`-backed wrapper (`RanjId`, `RanjIdDesc`).
///
/// - **Non-human-readable**: decodes a `Uuid` directly from the wire and
///   revalidates it through `validate` (which is the wrapper's `from_uuid`).
/// - **Human-readable**: accepts a JSON string only (parsed through the
///   wrapper's `FromStr`). JSON integer input is rejected with a serde
///   `invalid_type` error because the visitor does not implement
///   `visit_i64` / `visit_u64`; UUIDs are not integers.
pub(crate) fn deserialize_uuid_wrapper<'de, Value, D, F, E>(
    deserializer: D,
    validate: F,
) -> Result<Value, D::Error>
where
    Value: FromStr<Err = E>,
    F: FnOnce(Uuid) -> Result<Value, E>,
    E: fmt::Display,
    D: Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        let raw = Uuid::deserialize(deserializer)?;
        return validate(raw).map_err(de::Error::custom);
    }

    deserializer.deserialize_str(UuidWrapperVisitor::<Value>(PhantomData))
}

struct UuidWrapperVisitor<Value>(PhantomData<Value>);

impl<'de, Value> Visitor<'de> for UuidWrapperVisitor<Value>
where
    Value: FromStr,
    Value::Err: fmt::Display,
{
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a UUID string")
    }

    fn visit_str<DE>(self, value: &str) -> Result<Self::Value, DE>
    where
        DE: de::Error,
    {
        Value::from_str(value).map_err(DE::custom)
    }
}
