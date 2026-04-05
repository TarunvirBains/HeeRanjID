import pytest
from heeranjid import HeerId

def test_heerid_from_int():
    hid = HeerId(0)
    assert hid.as_int() == 0

def test_heerid_rejects_negative():
    with pytest.raises(ValueError, match="non-negative"):
        HeerId(-1)

def test_heerid_decodes_parts():
    raw = (1000 << 22) | (5 << 13) | 42
    hid = HeerId(raw)
    assert hid.timestamp_ms == 1000
    assert hid.node_id == 5
    assert hid.sequence == 42

def test_heerid_str():
    hid = HeerId(12345)
    assert str(hid) == "12345"

def test_heerid_repr():
    hid = HeerId(12345)
    assert repr(hid) == "HeerId(12345)"

def test_heerid_equality():
    a = HeerId(100)
    b = HeerId(100)
    c = HeerId(200)
    assert a == b
    assert a != c

def test_heerid_ordering():
    a = HeerId(100)
    b = HeerId(200)
    assert a < b
    assert b > a

def test_heerid_hash():
    a = HeerId(100)
    b = HeerId(100)
    assert hash(a) == hash(b)
    s = {a, b}
    assert len(s) == 1

def test_heerid_from_str():
    hid = HeerId.from_str("12345")
    assert hid.as_int() == 12345

def test_heerid_from_str_rejects_garbage():
    with pytest.raises(ValueError):
        HeerId.from_str("not_a_number")
