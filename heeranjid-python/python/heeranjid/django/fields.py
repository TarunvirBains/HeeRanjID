from django.db import models
from django.db.models.expressions import RawSQL
from heeranjid import HeerId, RanjId


class HeerIdField(models.BigIntegerField):
    def __init__(self, *args, **kwargs):
        if kwargs.get("primary_key", False) and "db_default" not in kwargs:
            kwargs["db_default"] = RawSQL("generate_id()", [])
        super().__init__(*args, **kwargs)

    def from_db_value(self, value, expression, connection):
        if value is None:
            return None
        return HeerId(int(value))

    def get_prep_value(self, value):
        if value is None:
            return None
        if isinstance(value, HeerId):
            return value.as_int()
        return int(value)

    def deconstruct(self):
        name, path, args, kwargs = super().deconstruct()
        return name, "heeranjid.django.fields.HeerIdField", args, kwargs


class RanjIdField(models.UUIDField):
    def __init__(self, *args, **kwargs):
        if kwargs.get("primary_key", False) and "db_default" not in kwargs:
            kwargs["db_default"] = RawSQL("generate_ranjid()", [])
        super().__init__(*args, **kwargs)

    def from_db_value(self, value, expression, connection):
        if value is None:
            return None
        if not isinstance(value, str):
            value = str(value)
        return RanjId.from_str(value)

    def get_prep_value(self, value):
        if value is None:
            return None
        if isinstance(value, RanjId):
            return value.to_uuid()
        return value

    def deconstruct(self):
        name, path, args, kwargs = super().deconstruct()
        return name, "heeranjid.django.fields.RanjIdField", args, kwargs
