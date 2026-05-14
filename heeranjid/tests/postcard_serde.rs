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

#[test]
fn serde_json_rejects_invalid_core_id_values() {
    assert!(serde_json::from_str::<HeerId>("-1").is_err());
    assert!(serde_json::from_str::<HeerIdDesc>("-1").is_err());

    let nil_uuid_json = serde_json::to_string(&Uuid::nil().to_string()).unwrap();
    assert!(serde_json::from_str::<RanjId>(&nil_uuid_json).is_err());
    assert!(serde_json::from_str::<RanjIdDesc>(&nil_uuid_json).is_err());
}
