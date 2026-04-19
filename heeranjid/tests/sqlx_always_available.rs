use heeranjid::{HeerId, RanjId};
use sqlx::Type;
use sqlx::postgres::Postgres;
use uuid::Uuid;

#[test]
fn sqlx_codecs_are_available_without_a_feature_flag() {
    assert_eq!(
        <HeerId as Type<Postgres>>::type_info(),
        <i64 as Type<Postgres>>::type_info(),
    );
    assert_eq!(
        <RanjId as Type<Postgres>>::type_info(),
        <Uuid as Type<Postgres>>::type_info(),
    );
}
