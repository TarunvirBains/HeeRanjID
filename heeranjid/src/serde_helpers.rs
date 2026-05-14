use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Serialize a wrapper either as its `Display` form (for human-readable
/// formats like JSON) or as the typed inner value (for binary formats like
/// postcard / bincode).
///
/// The non-human-readable branch delegates to `Inner`'s own `Serialize`
/// impl, producing a typed binary token (i64, byte array, etc.) rather
/// than a self-describing variant. This keeps the wire form compact and
/// works with `deserialize_any`-free decoders such as
/// [postcard](https://docs.rs/postcard).
pub fn serialize_display_or_inner<DisplayValue, Inner, S>(
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
        serializer.serialize_str(&display_value.to_string())
    } else {
        inner.serialize(serializer)
    }
}

/// Deserialize a wrapper from either a human-readable string-or-int form
/// (for JSON, TOML, etc.) or a typed binary form (for postcard / bincode).
///
/// * **Human-readable formats** accept either a quoted string parsed via
///   `Value::from_str` or a JSON integer that is first stringified then
///   parsed — preserving the long-standing "string-or-int" behaviour.
/// * **Non-human-readable formats** decode `Inner` directly (e.g. `i64`
///   or `Uuid`) without invoking `deserialize_any`, then run `validate`
///   to enforce the wrapper's type-level invariants (e.g. non-negative
///   `HeerId`, UUIDv8-shaped `RanjId`).
///
/// Splitting the binary path off `deserialize_any` is what unlocks
/// non-self-describing formats: postcard rejects `deserialize_any` at
/// runtime, so any wrapper that wants to round-trip through postcard
/// must take a typed branch in non-human-readable mode.
///
/// `validate` is invoked at most once and only on the binary path; the
/// human-readable path relies on `Value`'s `FromStr` impl to enforce
/// the same invariants. Both paths therefore produce values that
/// satisfy the wrapper's contract before they are returned to the caller.
pub fn deserialize_from_str_or_int_or_inner<'de, Value, Inner, D, F, E>(
    deserializer: D,
    validate: F,
) -> Result<Value, D::Error>
where
    Value: FromStr<Err = E>,
    Inner: Deserialize<'de>,
    F: FnOnce(Inner) -> Result<Value, E>,
    E: fmt::Display,
    D: Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        let inner = Inner::deserialize(deserializer)?;
        return validate(inner).map_err(de::Error::custom);
    }

    deserialize_from_str_or_int(deserializer)
}

fn deserialize_from_str_or_int<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: FromStr,
    T::Err: fmt::Display,
    D: Deserializer<'de>,
{
    struct StringOrIntVisitor<T>(std::marker::PhantomData<T>);

    impl<'de, T> Visitor<'de> for StringOrIntVisitor<T>
    where
        T: FromStr,
        T::Err: fmt::Display,
    {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a string or integer identifier")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            T::from_str(value).map_err(E::custom)
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_str(&value)
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_string(value.to_string())
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_string(value.to_string())
        }
    }

    deserializer.deserialize_any(StringOrIntVisitor(std::marker::PhantomData))
}
