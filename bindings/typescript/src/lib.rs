#[macro_use]
extern crate napi_derive;

use napi::bindgen_prelude::*;

// ── HeerId wrapper ──────────────────────────────────────────────────

#[napi]
pub struct HeerId {
    inner: heeranjid::HeerId,
}

#[napi]
impl HeerId {
    /// Create a HeerId from a BigInt (the raw i64 representation).
    #[napi(factory)]
    pub fn from_big_int(value: BigInt) -> Result<Self> {
        let (raw_value, lossless) = value.get_i64();
        if !lossless {
            return Err(Error::from_reason(
                "BigInt value does not fit losslessly in i64",
            ));
        }
        let inner = heeranjid::HeerId::from_i64(raw_value)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Create a HeerId from its string representation (decimal integer).
    #[napi(factory)]
    pub fn from_string(value: String) -> Result<Self> {
        let inner: heeranjid::HeerId = value
            .parse()
            .map_err(|e: heeranjid::Error| Error::from_reason(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Return the raw value as a BigInt.
    #[napi]
    pub fn to_big_int(&self, _env: Env) -> Result<BigInt> {
        let raw = self.inner.as_i64();
        // Create BigInt from i64 words
        Ok(BigInt {
            sign_bit: raw < 0,
            words: vec![raw as u64],
        })
    }

    /// The 41-bit timestamp in milliseconds (returned as f64 to avoid overflow).
    #[napi(getter)]
    pub fn timestamp_ms(&self) -> f64 {
        self.inner.timestamp_ms() as f64
    }

    /// The 9-bit node identifier.
    #[napi(getter)]
    pub fn node_id(&self) -> u16 {
        self.inner.node_id()
    }

    /// The 13-bit sequence number.
    #[napi(getter)]
    pub fn sequence(&self) -> u16 {
        self.inner.sequence()
    }

    /// Return the decimal string representation.
    #[napi]
    pub fn to_string_value(&self) -> String {
        self.inner.to_string()
    }
}

// ── RanjId wrapper ──────────────────────────────────────────────────

#[napi]
pub struct RanjId {
    inner: heeranjid::RanjId,
}

#[napi]
impl RanjId {
    /// Create a RanjId from its UUID string representation.
    #[napi(factory)]
    pub fn from_string(value: String) -> Result<Self> {
        let inner: heeranjid::RanjId = value
            .parse()
            .map_err(|e: heeranjid::Error| Error::from_reason(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Return the UUID string representation.
    #[napi]
    pub fn to_uuid(&self) -> String {
        self.inner.as_uuid().to_string()
    }

    /// The timestamp in microseconds (returned as f64 to avoid overflow).
    #[napi(getter)]
    pub fn timestamp_micros(&self) -> f64 {
        self.inner.timestamp_micros() as f64
    }

    /// The 15-bit node identifier (max 32767).
    #[napi(getter)]
    pub fn node_id(&self) -> u16 {
        self.inner.node_id()
    }

    /// The 16-bit sequence number.
    #[napi(getter)]
    pub fn sequence(&self) -> u16 {
        self.inner.sequence()
    }

    /// Return the UUID string representation (alias for toUuid).
    #[napi]
    pub fn to_string_value(&self) -> String {
        self.inner.to_string()
    }
}
