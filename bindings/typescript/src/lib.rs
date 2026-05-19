#[macro_use]
extern crate napi_derive;

use napi::bindgen_prelude::*;
use uuid::Uuid;

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

    /// Create a RanjId from a 16-byte big-endian buffer.
    ///
    /// This is the canonical BINARY(16) / MSSQL-safe wire format; it does NOT
    /// apply any mixed-endian swizzle. The bytes are validated to encode a
    /// well-formed UUIDv8 (RFC 4122 variant) via the heeranjid core decoder.
    ///
    /// Throws if the buffer is not exactly 16 bytes or if the bytes do not
    /// encode a valid UUIDv8 RanjId.
    ///
    /// Accepts both a Node `Buffer` and a bare `Uint8Array` (Buffer is a
    /// Uint8Array subclass). The wider `Uint8Array` parameter type is
    /// required to match Prisma 6+'s `Bytes` field shape: `@prisma/client`
    /// >= 6 returns a bare `Uint8Array` from the sqlserver adapter for
    /// `BINARY(16)` columns, and napi-rs's `Buffer` `FromNapiValue` impl
    /// rejects bare `Uint8Array` with "Expected a Buffer value". The
    /// underlying byte layout is identical for both, so callers passing a
    /// `Buffer` continue to work unchanged.
    #[napi(factory)]
    pub fn from_bytes(bytes: Uint8Array) -> Result<Self> {
        let slice: &[u8] = bytes.as_ref();
        if slice.len() != 16 {
            return Err(Error::from_reason(format!(
                "bytes must be exactly 16 bytes, got {}",
                slice.len()
            )));
        }
        // `Uuid::from_slice` copies the 16 bytes into an owned `[u8; 16]`,
        // and `RanjId` stores the `Uuid` by value. The caller's buffer is
        // never aliased after this call, so mutating it later cannot
        // corrupt the constructed RanjId.
        let uuid = Uuid::from_slice(slice)
            .map_err(|e| Error::from_reason(format!("invalid uuid bytes: {e}")))?;
        let inner = heeranjid::RanjId::from_uuid(uuid)
            .map_err(|e: heeranjid::Error| Error::from_reason(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Return a copy of the raw 16-byte big-endian representation.
    ///
    /// This is the canonical BINARY(16) / MSSQL-safe wire format; it does NOT
    /// apply the mixed-endian swizzle used by `toUuid` / Guid round-tripping.
    /// Each call returns a fresh allocation, so the returned array is safe
    /// to mutate without affecting the RanjId.
    ///
    /// Returns a `Uint8Array` for symmetry with `fromBytes` (which accepts
    /// the same JS type) and to match the runtime shape Prisma 6+ surfaces
    /// for `Bytes` / `BINARY(16)` columns on the sqlserver adapter — letting
    /// callers feed the result straight back into `prisma.model.create({
    /// data: { id: ranjId.toBytes() } })` without an intermediate conversion.
    /// If a caller needs `Buffer`-specific accessors (`readUInt8`,
    /// `toString("hex")`, etc.), wrapping with `Buffer.from(result)` is a
    /// zero-copy view because `Buffer` is a `Uint8Array` subclass on Node.
    #[napi]
    pub fn to_bytes(&self) -> Uint8Array {
        // `as_uuid()` is by-value (Uuid: Copy); bind it so `into_bytes()` has
        // a place to borrow from. The resulting `Vec<u8>` is moved into the
        // `Uint8Array`, which owns the allocation and is finalized
        // independently of any borrowed view the JS side keeps alive.
        let uuid: Uuid = self.inner.as_uuid();
        Uint8Array::from(uuid.into_bytes().to_vec())
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
