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

- `HeerId` is optimized for compact internal storage.
- `RanjId` is optimized for interoperability and UUID tooling.
- Converting `RanjId` back to `HeerId` is only possible when the encoded values
  fit within the `HeerId` field limits.
