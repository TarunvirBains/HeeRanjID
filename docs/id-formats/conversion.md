# Conversion Between HeerId and RanjId

HeeRanjID supports conversion between its two identifier formats. This is the mechanism that makes migrating a production system from HeerId to RanjId possible without data loss.

---

## Direction and guarantees

* **HeerId → RanjId**: always succeeds, always lossless
* **RanjId → HeerId**: conditional — succeeds only when the RanjId values fit within HeerId's narrower limits

---

## HeerId → RanjId

Any HeerId can be converted into a RanjId. The timestamp, node ID, and sequence components are preserved exactly. The timestamp is scaled from milliseconds to the current `RANJID_PRECISION` unit (default: nanoseconds).

This conversion is the migration path: a system that starts on HeerId can move to RanjId at any point and all existing IDs remain valid.

---

## RanjId → HeerId

Conversion from RanjId to HeerId fails in three cases:

### 1. Node ID overflow

HeerId's node field is 9 bits (max 511). Any RanjId with `node_id > 511` cannot be converted.

### 2. Timestamp overflow

HeerId's timestamp field is 41 bits of milliseconds (max 2,199,023,255,551 ms). A RanjId whose timestamp, after converting to milliseconds, exceeds this value cannot be converted.

### 3. Sequence squash overflow

When converting a batch, multiple RanjIds that share the same (timestamp_ms, node_id) pair after truncating sub-millisecond precision are reassigned sequential HeerId sequence values. If more than 8,192 such RanjIds map to the same millisecond slot on the same node, the conversion fails — HeerId's 13-bit sequence field cannot hold them all.

---

## Conversion failures are explicit

The library returns an error on failure — there is no silent truncation or data loss. Batch conversion pre-checks for conflicts before modifying anything.

---

## Summary

* HeerId → RanjId is always lossless — this is the upgrade migration path
* RanjId → HeerId is conditional — it requires that the RanjId values were generated within HeerId's capacity constraints
* Failures are explicit and must be handled by the caller
