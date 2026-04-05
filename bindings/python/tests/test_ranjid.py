import uuid
import pytest
from heeranjid import RanjId

# UUIDv8 encoding: timestamp=1_000_000 us, node_id=100, sequence=200
VALID_V8 = "00000000-0000-8000-8007-a120006400c8"

def test_ranjid_from_str():
    rid = RanjId.from_str(VALID_V8)
    assert isinstance(rid, RanjId)

def test_ranjid_rejects_non_v8():
    with pytest.raises(ValueError, match="version"):
        RanjId.from_str("550e8400-e29b-41d4-a716-446655440000")

def test_ranjid_decodes_parts():
    rid = RanjId.from_str(VALID_V8)
    assert rid.timestamp_micros == 1_000_000
    assert rid.node_id == 100
    assert rid.sequence == 200

def test_ranjid_to_uuid():
    rid = RanjId.from_str(VALID_V8)
    u = rid.to_uuid()
    assert isinstance(u, uuid.UUID)
    assert u.version == 8

def test_ranjid_str():
    rid = RanjId.from_str(VALID_V8)
    s = str(rid)
    assert s == VALID_V8

def test_ranjid_repr():
    rid = RanjId.from_str(VALID_V8)
    assert repr(rid).startswith("RanjId(")

def test_ranjid_equality():
    a = RanjId.from_str(VALID_V8)
    b = RanjId.from_str(VALID_V8)
    assert a == b

def test_ranjid_hash():
    a = RanjId.from_str(VALID_V8)
    b = RanjId.from_str(VALID_V8)
    assert hash(a) == hash(b)

def test_ranjid_from_str_rejects_garbage():
    with pytest.raises(ValueError):
        RanjId.from_str("not-a-uuid")

def test_ranjid_timestamp_getter():
    rid = RanjId.from_str(VALID_V8)
    assert rid.timestamp == 1_000_000

def test_ranjid_precision_getter():
    rid = RanjId.from_str(VALID_V8)
    assert rid.precision == "us"
