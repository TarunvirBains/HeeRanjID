import uuid
import pytest
from heeranjid import RanjId

def test_ranjid_from_str():
    rid = RanjId.from_str("00000000-0000-7000-800f-4240006400c8")
    assert isinstance(rid, RanjId)

def test_ranjid_rejects_non_v7():
    with pytest.raises(ValueError, match="version"):
        RanjId.from_str("550e8400-e29b-41d4-a716-446655440000")

def test_ranjid_decodes_parts():
    rid = RanjId.from_str("00000000-0000-7000-800f-4240006400c8")
    assert rid.timestamp_micros == 1_000_000
    assert rid.node_id == 100
    assert rid.sequence == 200

def test_ranjid_to_uuid():
    rid = RanjId.from_str("00000000-0000-7000-800f-4240006400c8")
    u = rid.to_uuid()
    assert isinstance(u, uuid.UUID)
    assert u.version == 7

def test_ranjid_str():
    rid = RanjId.from_str("00000000-0000-7000-800f-4240006400c8")
    s = str(rid)
    assert s == "00000000-0000-7000-800f-4240006400c8"

def test_ranjid_repr():
    rid = RanjId.from_str("00000000-0000-7000-800f-4240006400c8")
    assert repr(rid).startswith("RanjId(")

def test_ranjid_equality():
    a = RanjId.from_str("00000000-0000-7000-800f-4240006400c8")
    b = RanjId.from_str("00000000-0000-7000-800f-4240006400c8")
    assert a == b

def test_ranjid_hash():
    a = RanjId.from_str("00000000-0000-7000-800f-4240006400c8")
    b = RanjId.from_str("00000000-0000-7000-800f-4240006400c8")
    assert hash(a) == hash(b)

def test_ranjid_from_str_rejects_garbage():
    with pytest.raises(ValueError):
        RanjId.from_str("not-a-uuid")
