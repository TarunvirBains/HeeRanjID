import uuid as uuid_mod

from django.db import models
from django.db.models.expressions import RawSQL
from heeranjid import HeerId, RanjId


class HeerIdField(models.BigIntegerField):
    def __init__(self, *args, **kwargs):
        if kwargs.get("primary_key", False) and "db_default" not in kwargs:
            kwargs["db_default"] = RawSQL("generate_id()", [])
        super().__init__(*args, **kwargs)

    def pre_save(self, model_instance, add):
        value = getattr(model_instance, self.attname, None)
        if value is not None:
            return value
        if not add:
            return value

        from django.db import connection

        cursor = connection.cursor()
        if connection.vendor == "microsoft":
            cursor.execute("EXEC generate_id @in_node_id = 1")
        else:
            cursor.execute("SELECT generate_id()")
        row = cursor.fetchone()
        new_id = HeerId(int(row[0]))
        setattr(model_instance, self.attname, new_id)
        return new_id

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


class RanjIdField(models.Field):
    def __init__(self, *args, **kwargs):
        if kwargs.get("primary_key", False) and "db_default" not in kwargs:
            kwargs["db_default"] = RawSQL("generate_ranjid()", [])
        super().__init__(*args, **kwargs)

    def db_type(self, connection):
        if connection.vendor == "microsoft":
            return "BINARY(16)"
        return "uuid"

    def rel_db_type(self, connection):
        return self.db_type(connection)

    def get_internal_type(self):
        return "RanjIdField"

    def pre_save(self, model_instance, add):
        value = getattr(model_instance, self.attname, None)
        if value is not None:
            return value
        if not add:
            return value

        from django.db import connection

        cursor = connection.cursor()
        if connection.vendor == "microsoft":
            cursor.execute("EXEC generate_ranjid @in_node_id = 1")
            row = cursor.fetchone()
            raw = row[0]
            new_id = RanjId.from_str(str(uuid_mod.UUID(bytes=bytes(raw))))
        else:
            cursor.execute("SELECT generate_ranjid()")
            row = cursor.fetchone()
            new_id = RanjId.from_str(str(row[0]))
        setattr(model_instance, self.attname, new_id)
        return new_id

    def from_db_value(self, value, expression, connection):
        if value is None:
            return None
        if isinstance(value, (bytes, memoryview)):
            value = uuid_mod.UUID(bytes=bytes(value))
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
