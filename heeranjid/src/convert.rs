use std::collections::HashMap;

use crate::heer::HeerId;
use crate::precision::generation_precision;
use crate::ranj::RanjId;

#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("timestamp {value} exceeds HeerId max ({max} ms)")]
    TimestampOverflow { value: u128, max: u64 },

    #[error("node_id {value} exceeds HeerId max ({max})")]
    NodeIdOverflow { value: u16, max: u16 },

    #[error(
        "{count} IDs share (timestamp_ms={timestamp_ms}, node_id={node_id}) after squashing, exceeding sequence max {max}"
    )]
    SequenceOverflow {
        timestamp_ms: u64,
        node_id: u16,
        count: usize,
        max: usize,
    },

    #[error("HeerId construction failed: {0}")]
    HeerIdError(#[from] crate::Error),
}

#[derive(Debug, Clone)]
pub struct ConversionConflict {
    pub kind: ConflictKind,
    pub ranj_ids: Vec<RanjId>,
}

#[derive(Debug, Clone)]
pub enum ConflictKind {
    NodeIdOverflow {
        node_id: u16,
    },
    TimestampOverflow {
        timestamp_ms: u64,
    },
    SequenceOverflow {
        timestamp_ms: u64,
        node_id: u16,
        count: usize,
        max: usize,
    },
}

// ── HeerId → RanjId (always succeeds) ──

impl HeerId {
    pub fn check_ranjid_convertibility(_ids: &[HeerId]) -> Vec<ConversionConflict> {
        Vec::new() // HeerId always fits in RanjId
    }

    pub fn batch_to_ranjids(ids: &[HeerId]) -> Vec<(HeerId, RanjId)> {
        let precision = generation_precision();
        let factor = precision.from_millis_multiplier();
        ids.iter()
            .map(|hid| {
                let parts = hid.into_parts();
                let rid = RanjId::new(
                    u128::from(parts.timestamp_ms) * factor,
                    precision,
                    parts.node_id,
                    parts.sequence,
                )
                .expect("HeerId always fits in RanjId");
                (*hid, rid)
            })
            .collect()
    }
}

// ── RanjId → HeerId (can fail, batch-aware) ──

impl RanjId {
    pub fn check_heerid_convertibility(ids: &[RanjId]) -> Vec<ConversionConflict> {
        let mut conflicts = Vec::new();

        // Check individual hard failures
        let mut ts_overflow: HashMap<u64, Vec<RanjId>> = HashMap::new();
        let mut node_overflow: HashMap<u16, Vec<RanjId>> = HashMap::new();
        let mut groups: HashMap<(u64, u16), Vec<RanjId>> = HashMap::new();

        for &rid in ids {
            let parts = rid.into_parts();
            let timestamp_ms = (parts.timestamp / parts.precision.from_millis_multiplier()) as u64;

            if parts.node_id > HeerId::MAX_NODE_ID {
                node_overflow.entry(parts.node_id).or_default().push(rid);
                continue;
            }

            if timestamp_ms > HeerId::MAX_TIMESTAMP_MS {
                ts_overflow.entry(timestamp_ms).or_default().push(rid);
                continue;
            }

            groups
                .entry((timestamp_ms, parts.node_id))
                .or_default()
                .push(rid);
        }

        for (node_id, ranj_ids) in node_overflow {
            conflicts.push(ConversionConflict {
                kind: ConflictKind::NodeIdOverflow { node_id },
                ranj_ids,
            });
        }

        for (timestamp_ms, ranj_ids) in ts_overflow {
            conflicts.push(ConversionConflict {
                kind: ConflictKind::TimestampOverflow { timestamp_ms },
                ranj_ids,
            });
        }

        let max_seq = HeerId::MAX_SEQUENCE as usize + 1;
        for ((timestamp_ms, node_id), ranj_ids) in &groups {
            if ranj_ids.len() > max_seq {
                conflicts.push(ConversionConflict {
                    kind: ConflictKind::SequenceOverflow {
                        timestamp_ms: *timestamp_ms,
                        node_id: *node_id,
                        count: ranj_ids.len(),
                        max: max_seq,
                    },
                    ranj_ids: ranj_ids.clone(),
                });
            }
        }

        conflicts
    }

    pub fn batch_to_heerids(ids: &[RanjId]) -> Result<Vec<(RanjId, HeerId)>, ConversionError> {
        let max_seq = HeerId::MAX_SEQUENCE as usize + 1;

        // Group by (timestamp_ms, node_id), preserving original RanjId for each
        let mut groups: HashMap<(u64, u16), Vec<RanjId>> = HashMap::new();

        for &rid in ids {
            let parts = rid.into_parts();
            let timestamp_ms = (parts.timestamp / parts.precision.from_millis_multiplier()) as u64;

            if parts.node_id > HeerId::MAX_NODE_ID {
                return Err(ConversionError::NodeIdOverflow {
                    value: parts.node_id,
                    max: HeerId::MAX_NODE_ID,
                });
            }

            if timestamp_ms > HeerId::MAX_TIMESTAMP_MS {
                return Err(ConversionError::TimestampOverflow {
                    value: u128::from(timestamp_ms),
                    max: HeerId::MAX_TIMESTAMP_MS,
                });
            }

            groups
                .entry((timestamp_ms, parts.node_id))
                .or_default()
                .push(rid);
        }

        let mut results = Vec::with_capacity(ids.len());

        for ((timestamp_ms, node_id), mut group) in groups {
            if group.len() > max_seq {
                return Err(ConversionError::SequenceOverflow {
                    timestamp_ms,
                    node_id,
                    count: group.len(),
                    max: max_seq,
                });
            }

            // Sort by original RanjId to preserve temporal ordering
            group.sort();

            for (seq, rid) in group.into_iter().enumerate() {
                let hid = HeerId::new(timestamp_ms, node_id, seq as u16)?;
                results.push((rid, hid));
            }
        }

        results.sort_by_key(|(rid, _)| *rid);

        Ok(results)
    }
}

// ── Single-value `From` / `TryFrom` wrappers over the batch APIs ──
//
// These expose the batch conversion paths above for one-off callers. The
// desc-typed conversions below reuse them to avoid duplicating the scaling
// and precision-coercion logic.

impl From<HeerId> for RanjId {
    fn from(hid: HeerId) -> Self {
        // `batch_to_ranjids` is infallible; a single-element call cannot fail.
        HeerId::batch_to_ranjids(&[hid])
            .into_iter()
            .next()
            .expect("batch_to_ranjids returns one entry per input")
            .1
    }
}

impl TryFrom<RanjId> for HeerId {
    type Error = ConversionError;
    fn try_from(rid: RanjId) -> Result<Self, Self::Error> {
        // A single-element batch has no squashing, so any failure is a hard
        // per-value error (node/timestamp overflow) — sequence overflow cannot
        // occur with n=1.
        let mut results = RanjId::batch_to_heerids(&[rid])?;
        Ok(results
            .pop()
            .expect("batch_to_heerids returns one entry per input")
            .1)
    }
}

// ── HeerIdDesc ↔ RanjIdDesc ──
//
// Mirror of the asc conversions above. Identical failure modes on the
// `TryFrom` direction (NodeIdOverflow, TimestampOverflow, SequenceOverflow);
// see spec §4.5.

impl From<crate::HeerIdDesc> for crate::RanjIdDesc {
    fn from(hd: crate::HeerIdDesc) -> Self {
        // Go logical → logical through the asc types to reuse the existing
        // scaling + precision-coercion path. One XOR on each side.
        let asc = crate::HeerId::new(hd.timestamp_ms(), hd.node_id(), hd.sequence())
            .expect("HeerIdDesc always carries valid logical fields");
        let r_asc: crate::RanjId = asc.into();
        let parts = r_asc.into_parts();
        crate::RanjIdDesc::new(
            parts.timestamp,
            parts.precision,
            parts.node_id,
            parts.sequence,
        )
        .expect("HeerId → RanjId is always lossless")
    }
}

impl TryFrom<crate::RanjIdDesc> for crate::HeerIdDesc {
    type Error = ConversionError;
    fn try_from(rd: crate::RanjIdDesc) -> Result<Self, Self::Error> {
        let r_asc = crate::RanjId::new(rd.timestamp(), rd.precision(), rd.node_id(), rd.sequence())
            .expect("RanjIdDesc always carries valid logical fields");
        let asc: crate::HeerId = crate::HeerId::try_from(r_asc)?;
        let parts = asc.into_parts();
        Ok(
            crate::HeerIdDesc::new(parts.timestamp_ms, parts.node_id, parts.sequence)
                .expect("fields validated by HeerId::try_from"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::precision::RanjPrecision;

    // ── HeerId → RanjId tests ──

    #[test]
    fn batch_to_ranjids_scales_timestamps_and_preserves_fields() {
        let ids = vec![
            HeerId::new(1000, 5, 0).unwrap(),
            HeerId::new(2000, 10, 100).unwrap(),
            HeerId::new(3000, 15, 200).unwrap(),
        ];

        let results = HeerId::batch_to_ranjids(&ids);
        assert_eq!(results.len(), 3);

        let precision = generation_precision();
        let factor = precision.from_millis_multiplier();

        for (hid, rid) in &results {
            let hparts = hid.into_parts();
            let rparts = rid.into_parts();

            assert_eq!(rparts.timestamp, u128::from(hparts.timestamp_ms) * factor);
            assert_eq!(rparts.node_id, hparts.node_id);
            assert_eq!(rparts.sequence, hparts.sequence);
            assert_eq!(rparts.precision, precision);
        }
    }

    #[test]
    fn batch_to_ranjids_returns_correct_tuples() {
        let ids = vec![
            HeerId::new(100, 1, 0).unwrap(),
            HeerId::new(200, 2, 1).unwrap(),
            HeerId::new(300, 3, 2).unwrap(),
        ];

        let results = HeerId::batch_to_ranjids(&ids);

        for (i, (hid, _rid)) in results.iter().enumerate() {
            assert_eq!(*hid, ids[i]);
        }
    }

    // ── RanjId → HeerId tests ──

    #[test]
    fn batch_to_heerids_no_squashing() {
        let rids = vec![
            RanjId::new(1_000_000, RanjPrecision::Microseconds, 1, 0).unwrap(), // 1000 ms
            RanjId::new(2_000_000, RanjPrecision::Microseconds, 2, 0).unwrap(), // 2000 ms
            RanjId::new(3_000_000, RanjPrecision::Microseconds, 3, 0).unwrap(), // 3000 ms
        ];

        let results = RanjId::batch_to_heerids(&rids).unwrap();
        assert_eq!(results.len(), 3);

        for (rid, hid) in &results {
            let rparts = rid.into_parts();
            let hparts = hid.into_parts();
            let expected_ms = (rparts.timestamp / rparts.precision.from_millis_multiplier()) as u64;
            assert_eq!(hparts.timestamp_ms, expected_ms);
        }
    }

    #[test]
    fn batch_to_heerids_squashes_sub_millisecond_timestamps() {
        // With Microseconds precision, from_millis_multiplier() = 1000
        // So timestamp 1000..1999 all map to timestamp_ms = 1
        let rids = vec![
            RanjId::new(1000, RanjPrecision::Microseconds, 5, 0).unwrap(),
            RanjId::new(1500, RanjPrecision::Microseconds, 5, 1).unwrap(),
            RanjId::new(1999, RanjPrecision::Microseconds, 5, 2).unwrap(),
        ];

        let results = RanjId::batch_to_heerids(&rids).unwrap();
        assert_eq!(results.len(), 3);

        // All should have timestamp_ms = 1 and node_id = 5
        for (_rid, hid) in &results {
            let hparts = hid.into_parts();
            assert_eq!(hparts.timestamp_ms, 1);
            assert_eq!(hparts.node_id, 5);
        }

        // Sequences should be 0, 1, 2
        let mut seqs: Vec<u16> = results.iter().map(|(_, hid)| hid.sequence()).collect();
        seqs.sort();
        assert_eq!(seqs, vec![0, 1, 2]);
    }

    #[test]
    fn batch_to_heerids_preserves_ordering_within_squashed_groups() {
        // Create RanjIds with ascending timestamps that all squash to same ms
        let rids = vec![
            RanjId::new(5_000_100, RanjPrecision::Microseconds, 1, 0).unwrap(),
            RanjId::new(5_000_200, RanjPrecision::Microseconds, 1, 0).unwrap(),
            RanjId::new(5_000_300, RanjPrecision::Microseconds, 1, 0).unwrap(),
        ];

        let results = RanjId::batch_to_heerids(&rids).unwrap();

        // Within the squashed group, sorted by RanjId (which is temporal),
        // sequences should match the original order
        let pairs: Vec<(RanjId, u16)> = results
            .iter()
            .map(|(rid, hid)| (*rid, hid.sequence()))
            .collect();

        // The first RanjId (smallest timestamp) should get seq 0, etc.
        for (i, (rid, seq)) in pairs.iter().enumerate() {
            assert_eq!(*rid, rids[i]);
            assert_eq!(*seq, i as u16);
        }
    }

    #[test]
    fn batch_to_heerids_fails_on_node_id_overflow() {
        let rid = RanjId::new(1_000_000, RanjPrecision::Microseconds, 1000, 0).unwrap();
        let result = RanjId::batch_to_heerids(&[rid]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConversionError::NodeIdOverflow {
                value: 1000,
                max: 511,
            }
        ));
    }

    #[test]
    fn batch_to_heerids_fails_on_timestamp_overflow() {
        // HeerId::MAX_TIMESTAMP_MS is (1 << 41) - 1 = 2199023255551
        // Create a RanjId with timestamp that exceeds this in ms
        let huge_ts = (HeerId::MAX_TIMESTAMP_MS as u128 + 1) * 1_000; // in microseconds
        let rid = RanjId::new(huge_ts, RanjPrecision::Microseconds, 0, 0).unwrap();
        let result = RanjId::batch_to_heerids(&[rid]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConversionError::TimestampOverflow { .. }
        ));
    }

    #[test]
    fn batch_to_heerids_fails_on_sequence_overflow() {
        // Create 8193 RanjIds all mapping to same (timestamp_ms=1, node_id=1)
        // Use nanosecond precision: from_millis_multiplier() = 1_000_000
        // Timestamps [1_000_000, 1_999_999] all map to ms=1
        let count = HeerId::MAX_SEQUENCE as usize + 2; // 8193
        let rids: Vec<RanjId> = (0..count)
            .map(|i| {
                RanjId::new(1_000_000 + (i as u128), RanjPrecision::Nanoseconds, 1, 0).unwrap()
            })
            .collect();

        let result = RanjId::batch_to_heerids(&rids);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConversionError::SequenceOverflow { count: 8193, .. }
        ));
    }

    // ── check_heerid_convertibility tests ──

    #[test]
    fn check_heerid_convertibility_returns_empty_for_valid_batch() {
        let rids = vec![
            RanjId::new(1_000_000, RanjPrecision::Microseconds, 1, 0).unwrap(),
            RanjId::new(2_000_000, RanjPrecision::Microseconds, 2, 0).unwrap(),
            RanjId::new(3_000_000, RanjPrecision::Microseconds, 3, 0).unwrap(),
        ];
        let conflicts = RanjId::check_heerid_convertibility(&rids);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn check_heerid_convertibility_detects_node_id_overflow() {
        let rids = vec![RanjId::new(1_000_000, RanjPrecision::Microseconds, 1000, 0).unwrap()];
        let conflicts = RanjId::check_heerid_convertibility(&rids);
        assert_eq!(conflicts.len(), 1);
        assert!(matches!(
            conflicts[0].kind,
            ConflictKind::NodeIdOverflow { node_id: 1000 }
        ));
    }

    #[test]
    fn check_heerid_convertibility_detects_sequence_overflow() {
        // Use nanosecond precision so all timestamps [1_000_000, 1_008_192] map to ms=1
        let count = HeerId::MAX_SEQUENCE as usize + 2; // 8193
        let rids: Vec<RanjId> = (0..count)
            .map(|i| {
                RanjId::new(1_000_000 + (i as u128), RanjPrecision::Nanoseconds, 1, 0).unwrap()
            })
            .collect();

        let conflicts = RanjId::check_heerid_convertibility(&rids);
        assert_eq!(conflicts.len(), 1);
        assert!(matches!(
            conflicts[0].kind,
            ConflictKind::SequenceOverflow { count: 8193, .. }
        ));
    }

    // ── Roundtrip test ──

    #[test]
    fn roundtrip_batch_to_ranjids_then_back_preserves_ordering() {
        let heer_ids = vec![
            HeerId::new(100, 1, 0).unwrap(),
            HeerId::new(200, 2, 1).unwrap(),
            HeerId::new(300, 3, 2).unwrap(),
        ];

        let ranj_pairs = HeerId::batch_to_ranjids(&heer_ids);
        let ranj_ids: Vec<RanjId> = ranj_pairs.iter().map(|(_, rid)| *rid).collect();

        let heer_pairs = RanjId::batch_to_heerids(&ranj_ids).unwrap();

        // The resulting HeerIds should match the originals
        // (sequence may differ since batch_to_heerids reassigns sequences within groups,
        // but with distinct timestamps they should each get seq 0)
        assert_eq!(heer_pairs.len(), heer_ids.len());

        // Collect the HeerIds sorted by their timestamp for comparison
        let mut original_sorted = heer_ids.clone();
        original_sorted.sort();

        let mut result_hids: Vec<HeerId> = heer_pairs.iter().map(|(_, hid)| *hid).collect();
        result_hids.sort();

        // With distinct timestamps, each gets seq=0, so the roundtripped HeerIds
        // will have the same timestamp and node but seq=0
        for (orig, result) in original_sorted.iter().zip(result_hids.iter()) {
            assert_eq!(orig.timestamp_ms(), result.timestamp_ms());
            assert_eq!(orig.node_id(), result.node_id());
        }
    }
}

#[cfg(test)]
mod desc_tests {
    use super::*;
    use crate::precision::RanjPrecision;
    use crate::{HeerIdDesc, RanjIdDesc};

    #[test]
    fn heer_desc_into_ranj_desc_preserves_logical_fields() {
        let hd = HeerIdDesc::new(1_234_567, 42, 777).unwrap();
        let rd: RanjIdDesc = hd.into();
        assert_eq!(rd.node_id(), 42);
        assert_eq!(rd.sequence(), 777);
        // Timestamp in ms, scaled via generation precision.
        let factor = crate::precision::generation_precision().from_millis_multiplier();
        assert_eq!(rd.timestamp(), 1_234_567u128 * factor);
    }

    #[test]
    fn ranj_desc_try_into_heer_desc_reports_node_overflow() {
        let rd = RanjIdDesc::new(1000, RanjPrecision::Microseconds, 9999, 0).unwrap();
        let err = HeerIdDesc::try_from(rd).unwrap_err();
        assert!(matches!(err, ConversionError::NodeIdOverflow { .. }));
    }

    #[test]
    fn ranj_desc_try_into_heer_desc_preserves_timestamp_and_node() {
        // A RanjIdDesc with fields that fit in HeerId should convert cleanly.
        // Note: per `batch_to_heerids` semantics, sequence is reassigned
        // within each (ts_ms, node_id) group — for a single-element batch it
        // is always 0. Timestamp and node_id are preserved.
        let hd_orig = HeerIdDesc::new(1_234_567, 42, 777).unwrap();
        let rd: RanjIdDesc = hd_orig.into();
        let hd_back = HeerIdDesc::try_from(rd).unwrap();
        assert_eq!(hd_back.timestamp_ms(), 1_234_567);
        assert_eq!(hd_back.node_id(), 42);
        assert_eq!(hd_back.sequence(), 0);
    }

    #[test]
    fn ranj_desc_try_into_heer_desc_reports_timestamp_overflow() {
        // RanjId's 89-bit timestamp field holds values far beyond HeerId's
        // 41-bit millisecond range. Build a RanjIdDesc whose logical
        // timestamp, scaled down to ms, exceeds HeerId::MAX_TIMESTAMP_MS,
        // and confirm the conversion surfaces the expected error variant.
        let factor = crate::precision::generation_precision().from_millis_multiplier();
        let ts_units = u128::from(HeerId::MAX_TIMESTAMP_MS + 1) * factor;
        let rd = RanjIdDesc::new(ts_units, RanjPrecision::Microseconds, 1, 0).unwrap();
        let err = HeerIdDesc::try_from(rd).unwrap_err();
        assert!(
            matches!(err, ConversionError::TimestampOverflow { .. }),
            "expected TimestampOverflow, got {err:?}"
        );
    }

    // Spec §4.5 lists three failure modes for `TryFrom<RanjIdDesc> for
    // HeerIdDesc`: NodeIdOverflow, TimestampOverflow, SequenceOverflow.
    // The first two are reachable with a single input (see the two tests
    // above). SequenceOverflow is only reachable when a *batch* of inputs
    // shares (timestamp_ms, node_id) and the group size exceeds HeerId's
    // 13-bit sequence field — a single-element batch is always ≤ 1 and
    // therefore cannot trigger it. Callers that need the batch path and
    // its sequence-squashing semantics use `RanjId::batch_to_heerids`
    // directly (covered by `batch_to_heerids_fails_on_sequence_overflow`
    // in the asc test module).
}
