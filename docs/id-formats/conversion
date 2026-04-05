# Conversion Between HeerId and RanjId

HeeRanjID provides conversion between its two identifier formats:

* **HeerId (64-bit)**
* **RanjId (128-bit, UUID-compatible)**

These conversions allow systems to move between compact internal representations and portable external identifiers.

---

## Overview

* HeerId → RanjId conversion is always supported
* RanjId → HeerId conversion is conditional

This asymmetry exists because HeerId is a compact representation with stricter limits.

---

## HeerId → RanjId

A HeerId can always be converted into a RanjId.

During conversion:

* The timestamp, node, and sequence components are preserved
* Additional space in the 128-bit format may be used for structure or future extension

This conversion is **lossless**.

---

## RanjId → HeerId

Conversion from RanjId to HeerId is only possible when the RanjId encodes values that fit within HeerId constraints.

This conversion may fail in the following cases:

### 1. Timestamp overflow

If the timestamp encoded in the RanjId exceeds the range supported by HeerId, conversion is not possible.

---

### 2. Node identifier overflow

If the node or worker identifier exceeds the allowed range for HeerId, conversion fails.

---

### 3. Sequence overflow

If the sequence component exceeds HeerId limits, conversion fails.

---

### 4. Missing or incompatible structure

If a RanjId was not generated from a compatible format, or does not encode the required components in a recoverable way, conversion may not be possible.

---

## Handling Conversion Failures

The library provides explicit handling for conversion failures.

Depending on the language or API:

* Conversion may return an error or result type
* Failure cases are not silently ignored

This ensures that invalid or lossy conversions are handled intentionally.

---

## Design Considerations

### Compact vs. portable representations

HeerId is designed to be compact and efficient, while RanjId prioritizes portability and compatibility.

The conversion model reflects this:

* Expanding from HeerId → RanjId is always safe
* Compressing from RanjId → HeerId requires validation

---

### Interoperability

RanjId can be used as a stable external identifier, even in systems that do not understand HeerId.

However, only RanjId values that follow the HeeRanjID encoding can be converted back into HeerId.

---

## Practical Usage

Typical usage patterns include:

* **Internal storage** → use HeerId
* **External APIs / integration** → use RanjId
* **Conversion at boundaries** → convert as needed

---

## Summary

* HeerId → RanjId is always possible and lossless
* RanjId → HeerId is conditional and may fail
* Conversion failures are explicit and must be handled by the caller

This model allows HeeRanjID to support both efficient internal storage and interoperable external identifiers without forcing a single format for all use cases.
