use heeranjid::{HeerId, HeerIdDesc, RanjId, RanjIdDesc, RanjPrecision};
use uuid::Uuid;

fn assert_postcard_roundtrip<T>(value: T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + Eq + std::fmt::Debug,
{
    let bytes = postcard::to_allocvec(&value).unwrap();
    let decoded: T = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn postcard_round_trips_all_core_id_types() {
    assert_postcard_roundtrip(HeerId::new(55, 7, 9).unwrap());
    assert_postcard_roundtrip(HeerIdDesc::new(55, 7, 9).unwrap());
    assert_postcard_roundtrip(RanjId::new(55, RanjPrecision::Microseconds, 7, 9).unwrap());
    assert_postcard_roundtrip(RanjIdDesc::new(55, RanjPrecision::Microseconds, 7, 9).unwrap());
}

#[test]
fn postcard_rejects_negative_heer_id_values() {
    let bytes = postcard::to_allocvec(&-1_i64).unwrap();

    assert!(postcard::from_bytes::<HeerId>(&bytes).is_err());
    assert!(postcard::from_bytes::<HeerIdDesc>(&bytes).is_err());
}

#[test]
fn postcard_rejects_non_uuidv8_ranj_id_values() {
    let bytes = postcard::to_allocvec(&Uuid::nil()).unwrap();

    assert!(postcard::from_bytes::<RanjId>(&bytes).is_err());
    assert!(postcard::from_bytes::<RanjIdDesc>(&bytes).is_err());
}

/// UUID with a valid UUIDv8 version nibble but a non-RFC 4122 variant.
///
/// `0x8u128 << 76` sets bit 79 (the high bit of the version nibble at bits
/// 76-79) and leaves the variant bits (62-63) at `0b00`. That gives:
/// * version nibble = `0b1000` = 8 (passes the version check in
///   `RanjId::from_uuid` / `RanjIdDesc::from_uuid`), and
/// * variant bits = `0b00` (RFC 4122 requires `0b10`, so the variant check
///   rejects it).
///
/// Constructed by hand rather than via `Uuid::new_v8` so this crate's test
/// suite does not require the optional `v8` feature of the `uuid` crate.
const RANJ_WRONG_VARIANT_RAW: u128 = 0x8u128 << 76;

#[test]
fn postcard_rejects_wrong_variant_ranj_id_values() {
    let wrong_variant = Uuid::from_u128(RANJ_WRONG_VARIANT_RAW);
    // Guard the construction so a future change to the bit layout fails
    // here rather than silently turning this into a version-branch test.
    assert_eq!(wrong_variant.get_version_num(), 8);
    assert_ne!(wrong_variant.get_variant(), uuid::Variant::RFC4122);

    let bytes = postcard::to_allocvec(&wrong_variant).unwrap();

    assert!(postcard::from_bytes::<RanjId>(&bytes).is_err());
    assert!(postcard::from_bytes::<RanjIdDesc>(&bytes).is_err());
}

#[test]
fn serde_json_rejects_invalid_core_id_values() {
    assert!(serde_json::from_str::<HeerId>("-1").is_err());
    assert!(serde_json::from_str::<HeerIdDesc>("-1").is_err());

    let nil_uuid_json = serde_json::to_string(&Uuid::nil().to_string()).unwrap();
    assert!(serde_json::from_str::<RanjId>(&nil_uuid_json).is_err());
    assert!(serde_json::from_str::<RanjIdDesc>(&nil_uuid_json).is_err());
}

/// JSON integers must be rejected for the UUID-backed wrappers: their
/// deserialize visitor only implements `visit_str`, so a JSON number is
/// surfaced as a serde `invalid_type` error. `serde_json` classifies
/// such errors as `Category::Data` (semantic failure inside a
/// syntactically valid JSON value), which is a strictly tighter
/// assertion than `is_err()` -- it rules out a Syntax / EOF / IO error
/// silently passing the test.
#[test]
fn serde_json_rejects_integer_for_ranj_id_wrappers() {
    let ranj_err = serde_json::from_str::<RanjId>("42").unwrap_err();
    assert_eq!(ranj_err.classify(), serde_json::error::Category::Data);

    let ranj_desc_err = serde_json::from_str::<RanjIdDesc>("42").unwrap_err();
    assert_eq!(ranj_desc_err.classify(), serde_json::error::Category::Data);
}

/// `HeerIdDesc`'s human-readable deserializer accepts both JSON strings
/// and JSON integers; the integer path skips `Display` / `FromStr` and
/// routes directly through `HeerIdDesc::from_i64`. This mirrors the
/// existing `serde_deserializes_heerid_from_integer` test for `HeerId`
/// in `lib.rs` and prevents that path from silently regressing for the
/// desc-encoded sibling.
#[test]
fn serde_json_accepts_integer_for_heer_id_desc() {
    let id = HeerIdDesc::new(42, 7, 11).unwrap();
    let json_integer = id.as_i64().to_string();
    let parsed: HeerIdDesc = serde_json::from_str(&json_integer).unwrap();
    assert_eq!(parsed, id);
}

#[test]
fn serde_json_round_trips_ranj_id_as_string() {
    let id = RanjId::new(42, RanjPrecision::Microseconds, 7, 11).unwrap();
    let json = serde_json::to_string(&id).unwrap();

    assert_eq!(json, serde_json::to_string(&id.to_string()).unwrap());

    let parsed: RanjId = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, id);
}

#[test]
fn serde_json_round_trips_ranj_id_desc_as_string() {
    let id = RanjIdDesc::new(42, RanjPrecision::Microseconds, 7, 11).unwrap();
    let json = serde_json::to_string(&id).unwrap();

    assert_eq!(json, serde_json::to_string(&id.to_string()).unwrap());

    let parsed: RanjIdDesc = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, id);
}

#[test]
fn serde_json_rejects_wrong_variant_ranj_id_strings() {
    let wrong_variant = Uuid::from_u128(RANJ_WRONG_VARIANT_RAW);
    assert_eq!(wrong_variant.get_version_num(), 8);
    assert_ne!(wrong_variant.get_variant(), uuid::Variant::RFC4122);

    let json = serde_json::to_string(&wrong_variant.to_string()).unwrap();

    // Wrong-variant strings parse as UUIDs (syntactically valid JSON),
    // then fail semantic validation inside `from_uuid`. The visitor
    // wraps that failure via `serde::de::Error::custom`, which
    // `serde_json` exposes as `Category::Data`. Asserting on the
    // category is a clean tightening over `is_err()` without falling
    // back to brittle Display-string matching on the inner error.
    let ranj_err = serde_json::from_str::<RanjId>(&json).unwrap_err();
    assert_eq!(ranj_err.classify(), serde_json::error::Category::Data);

    let ranj_desc_err = serde_json::from_str::<RanjIdDesc>(&json).unwrap_err();
    assert_eq!(ranj_desc_err.classify(), serde_json::error::Category::Data);
}
