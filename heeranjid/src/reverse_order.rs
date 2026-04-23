//! Bulk cross-direction conversion for `HeerId` ↔ `HeerIdDesc` and
//! `RanjId` ↔ `RanjIdDesc`. O(n) time, O(1) extra memory — the input
//! `Vec`'s allocation is reused for the output. See spec §4.6.

pub mod heer {
    use crate::heer::{HEER_FLIP_MASK, HeerId};
    use crate::heer_desc::HeerIdDesc;

    pub fn to_desc(mut v: Vec<HeerId>) -> Vec<HeerIdDesc> {
        for id in v.iter_mut() {
            let raw = id.as_i64() ^ HEER_FLIP_MASK;
            *id = HeerId::from_i64_raw(raw);
        }
        let mut v = std::mem::ManuallyDrop::new(v);
        let (ptr, len, cap) = (v.as_mut_ptr(), v.len(), v.capacity());
        // SAFETY: `HeerIdDesc` is #[repr(transparent)] over `i64` — identical
        // layout and alignment to `HeerId`. The post-XOR bit pattern is a
        // valid stored-form `HeerIdDesc` because `HEER_FLIP_MASK`'s bit 63
        // is zero, so XOR preserves the bit-63 = 0 invariant. `ManuallyDrop`
        // prevents the source `Vec`'s destructor from running on bits now
        // owned by the output.
        unsafe { Vec::from_raw_parts(ptr as *mut HeerIdDesc, len, cap) }
    }

    pub fn to_asc(mut v: Vec<HeerIdDesc>) -> Vec<HeerId> {
        for id in v.iter_mut() {
            let raw = id.as_i64() ^ HEER_FLIP_MASK;
            *id = HeerIdDesc::from_i64(raw).expect("XOR preserves bit 63 = 0");
        }
        let mut v = std::mem::ManuallyDrop::new(v);
        let (ptr, len, cap) = (v.as_mut_ptr(), v.len(), v.capacity());
        // SAFETY: symmetric to `to_desc`.
        unsafe { Vec::from_raw_parts(ptr as *mut HeerId, len, cap) }
    }
}

pub mod ranj {
    use crate::ranj::{RANJ_FLIP_MASK, RanjId};
    use crate::ranj_desc::RanjIdDesc;
    use uuid::Uuid;

    pub fn to_desc(mut v: Vec<RanjId>) -> Vec<RanjIdDesc> {
        for id in v.iter_mut() {
            let raw = id.as_uuid().as_u128() ^ RANJ_FLIP_MASK;
            *id = RanjId::from_uuid_raw(Uuid::from_u128(raw));
        }
        let mut v = std::mem::ManuallyDrop::new(v);
        let (ptr, len, cap) = (v.as_mut_ptr(), v.len(), v.capacity());
        // SAFETY: `RanjIdDesc` is #[repr(transparent)] over `Uuid` — identical
        // layout. The mask preserves version and variant nibbles, so every
        // post-XOR value is a valid UUIDv8 RFC-4122 uuid and therefore a
        // valid stored-form `RanjIdDesc`. `ManuallyDrop` handles ownership.
        unsafe { Vec::from_raw_parts(ptr as *mut RanjIdDesc, len, cap) }
    }

    pub fn to_asc(mut v: Vec<RanjIdDesc>) -> Vec<RanjId> {
        for id in v.iter_mut() {
            let raw = id.as_uuid().as_u128() ^ RANJ_FLIP_MASK;
            *id = RanjIdDesc::from_uuid(Uuid::from_u128(raw))
                .expect("XOR preserves version and variant");
        }
        let mut v = std::mem::ManuallyDrop::new(v);
        let (ptr, len, cap) = (v.as_mut_ptr(), v.len(), v.capacity());
        // SAFETY: symmetric to `to_desc`.
        unsafe { Vec::from_raw_parts(ptr as *mut RanjId, len, cap) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::precision::RanjPrecision;
    use crate::{HeerId, HeerIdDesc, RanjId};

    #[test]
    fn heer_to_desc_to_asc_is_identity() {
        let original: Vec<HeerId> = (0..1000u64)
            .map(|i| HeerId::new(i * 1000, (i as u16) % 500, (i as u16) % 8000).unwrap())
            .collect();
        let copy = original.clone();
        let descs = heer::to_desc(copy);
        assert_eq!(descs.len(), original.len());
        let back = heer::to_asc(descs);
        assert_eq!(back, original);
    }

    #[test]
    fn heer_to_desc_preserves_logical_fields() {
        let id = HeerId::new(1_234_567, 42, 777).unwrap();
        let descs = heer::to_desc(vec![id]);
        assert_eq!(descs[0].timestamp_ms(), 1_234_567);
        assert_eq!(descs[0].node_id(), 42);
        assert_eq!(descs[0].sequence(), 777);
    }

    #[test]
    fn heer_bulk_sort_matches_per_element_desc_sort() {
        let asc: Vec<HeerId> = (0..100u64)
            .rev()
            .map(|i| HeerId::new(i * 1000, 1, 0).unwrap())
            .collect();
        let mut bulk = heer::to_desc(asc.clone());
        bulk.sort();
        let mut per_elem: Vec<HeerIdDesc> = asc
            .iter()
            .map(|h| {
                HeerIdDesc::new(
                    h.into_parts().timestamp_ms,
                    h.into_parts().node_id,
                    h.into_parts().sequence,
                )
                .unwrap()
            })
            .collect();
        per_elem.sort();
        assert_eq!(bulk, per_elem);
    }

    #[test]
    fn ranj_to_desc_to_asc_is_identity() {
        let original: Vec<RanjId> = (0..1000u128)
            .map(|i| {
                RanjId::new(
                    i * 1000,
                    RanjPrecision::Microseconds,
                    (i as u16) % 1000,
                    i as u16,
                )
                .unwrap()
            })
            .collect();
        let copy = original.clone();
        let descs = ranj::to_desc(copy);
        let back = ranj::to_asc(descs);
        assert_eq!(back, original);
    }

    #[test]
    fn reuses_allocation() {
        let v: Vec<HeerId> = (0..10u64).map(|i| HeerId::new(i, 0, 0).unwrap()).collect();
        let cap_before = v.capacity();
        let ptr_before = v.as_ptr() as usize;
        let descs = heer::to_desc(v);
        // Same capacity and same backing pointer — allocation was moved, not copied.
        assert_eq!(descs.capacity(), cap_before);
        assert_eq!(descs.as_ptr() as usize, ptr_before);
    }
}
