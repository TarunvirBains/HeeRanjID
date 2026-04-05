import uuid as uuid_mod

from django.core.exceptions import ImproperlyConfigured
from django.db import models
from django.db.models.expressions import RawSQL
from heeranjid import HeerId, RanjId


class HeerIdField(models.BigIntegerField):
    def __init__(self, *args, **kwargs):
        if kwargs.get("primary_key", False) and "db_default" not in kwargs:
            kwargs["db_default"] = RawSQL("generate_id()", [])
        super().__init__(*args, **kwargs)

    def contribute_to_class(self, cls, name, **kwargs):
        super().contribute_to_class(cls, name, **kwargs)

        def check_manager(sender, **signal_kwargs):
            manager = cls._default_manager
            if manager is None or not getattr(manager, "_heeranjid_enabled", False):
                raise ImproperlyConfigured(
                    f"Model '{cls.__name__}' has a {self.__class__.__name__} but its "
                    f"default manager does not support HeeRanjID bulk operations. "
                    f"Use HeeRanjIdManager or add HeeRanjIdManagerMixin to your custom manager."
                )

        from django.db.models.signals import class_prepared

        class_prepared.connect(check_manager, sender=cls, weak=False)

    def pre_save(self, model_instance, add):
        value = getattr(model_instance, self.attname, None)
        if value is not None:
            return value
        if not add:
            return value

        from django.db import connection

        from heeranjid_django.managers import _get_node_id

        node_id = _get_node_id()

        cursor = connection.cursor()
        if connection.vendor == "microsoft":
            cursor.execute("EXEC generate_id @in_node_id = %s", [node_id])
        else:
            cursor.execute("SELECT generate_id(%s)", [node_id])
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
        return name, "heeranjid_django.fields.HeerIdField", args, kwargs


class RanjIdField(models.Field):
    def __init__(self, *args, **kwargs):
        if kwargs.get("primary_key", False) and "db_default" not in kwargs:
            kwargs["db_default"] = RawSQL("generate_ranjid()", [])
        super().__init__(*args, **kwargs)

    def contribute_to_class(self, cls, name, **kwargs):
        super().contribute_to_class(cls, name, **kwargs)

        def check_manager(sender, **signal_kwargs):
            manager = cls._default_manager
            if manager is None or not getattr(manager, "_heeranjid_enabled", False):
                raise ImproperlyConfigured(
                    f"Model '{cls.__name__}' has a {self.__class__.__name__} but its "
                    f"default manager does not support HeeRanjID bulk operations. "
                    f"Use HeeRanjIdManager or add HeeRanjIdManagerMixin to your custom manager."
                )

        from django.db.models.signals import class_prepared

        class_prepared.connect(check_manager, sender=cls, weak=False)

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

        from heeranjid_django.managers import _get_node_id

        node_id = _get_node_id()

        cursor = connection.cursor()
        if connection.vendor == "microsoft":
            cursor.execute("EXEC generate_ranjid @in_node_id = %s", [node_id])
            row = cursor.fetchone()
            raw = row[0]
            new_id = RanjId.from_str(str(uuid_mod.UUID(bytes=bytes(raw))))
        else:
            cursor.execute("SELECT generate_ranjid(%s)", [node_id])
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
        return name, "heeranjid_django.fields.RanjIdField", args, kwargs
