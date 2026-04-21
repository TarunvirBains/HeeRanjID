//! Verify that the `sqlx` feature gates the codecs correctly.
//!
//! This file always compiles (it has no feature gate at the file level), but
//! the actual assertions are only meaningful when the `sqlx` feature is
//! enabled. Without the feature the types do not implement `sqlx::Type`, so
//! the test body is skipped via `cfg`.

#[cfg(feature = "sqlx")]
#[test]
fn sqlx_codecs_are_available_with_sqlx_feature() {
    use heeranjid::{HeerId, RanjId};
    use sqlx::Type;
    use sqlx::postgres::Postgres;
    use uuid::Uuid;

    assert_eq!(
        <HeerId as Type<Postgres>>::type_info(),
        <i64 as Type<Postgres>>::type_info(),
    );
    assert_eq!(
        <RanjId as Type<Postgres>>::type_info(),
        <Uuid as Type<Postgres>>::type_info(),
    );
}
