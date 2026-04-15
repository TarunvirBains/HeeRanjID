# Limits Reference

This page summarizes the practical limits of the built-in identifier layouts.

## HeerId

- Timestamp bits: `41`
- Node bits: `9`
- Sequence bits: `13`
- Max timestamp: `2,199,023,255,551`
- Max node id: `511`
- Max sequence: `8,191`

## RanjId

- Timestamp bits: `89`
- Precision bits: `2`
- Node bits: `15`
- Sequence bits: `16`
- Max node id: `32,767`
- Max sequence: `65,535`

## Notes

- `HeerId` is the compact default format — 64-bit integer, `bigint` storage.
- `RanjId` is the upgrade format — 128-bit UUIDv8, higher node and sequence capacity, sub-ms precision.
- Converting `RanjId` back to `HeerId` is only possible when the encoded values
  fit within the `HeerId` field limits.
