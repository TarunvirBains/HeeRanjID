use serde::de::{self, Visitor};
use serde::{Deserializer, Serializer};
use std::fmt;
use std::str::FromStr;

pub fn serialize_display<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: fmt::Display,
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

pub fn deserialize_from_str_or_int<'de, T, D>(deserializer: D) -> Result<T, D::Error>
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
