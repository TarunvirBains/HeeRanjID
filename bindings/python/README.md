# heeranjid

> **HeerRanjId** ([ɦiːɾ.ɾaːnd͡ʒ.ɪd])

Python bindings for the core HeerRanjId types.

The package exposes:

- `HeerId` for compact 64-bit identifiers
- `RanjId` for UUIDv8-compatible external identifiers
- bundled SQL assets used by higher-level integrations

```python
from heeranjid import HeerId, RanjId

hid = HeerId(42)
rid = RanjId.from_str("00000000-0000-8000-8007-a120006400c8")
```

When building from a git checkout, initialize submodules first:

```bash
git submodule update --init --recursive
```
