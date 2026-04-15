import uuid as uuid_mod

from django import forms
from django.core.exceptions import ImproperlyConfigured
from django.db import models
from django.db.models.expressions import RawSQL
from heeranjid import HeerId, RanjId


class RanjIdFormField(forms.UUIDField):
    """
    Form field for RanjId values.

    Accepts the same input as Django's UUIDField (hyphenated or bare hex strings)
    but returns a RanjId instance instead of uuid.UUID, keeping the type
    consistent whether the value came from a form or the database.
    """

    def to_python(self, value):
        uuid_val = super().to_python(value)
        if uuid_val is None:
            return None
        return RanjId.from_str(str(uuid_val))


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
        # MANUAL mode: do not auto-generate; let the caller assign the PK.
        if getattr(model_instance.__class__, "_heeranjid_prefetch_manual", False):
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


class RanjIdField(models.UUIDField):
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
        # Preserve raw big-endian bytes on MSSQL to avoid uniqueidentifier's
        # mixed-endian byte-swap, which would corrupt RanjId's timestamp bits.
        if connection.vendor == "microsoft":
            return "BINARY(16)"
        return super().db_type(connection)  # native uuid on Postgres

    def pre_save(self, model_instance, add):
        value = getattr(model_instance, self.attname, None)
        if value is not None:
            return value
        if not add:
            return value
        # MANUAL mode: do not auto-generate; let the caller assign the PK.
        if getattr(model_instance.__class__, "_heeranjid_prefetch_manual", False):
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
            # MSSQL BINARY(16): raw big-endian bytes
            value = uuid_mod.UUID(bytes=bytes(value))
        if isinstance(value, uuid_mod.UUID):
            return RanjId.from_str(str(value))
        return RanjId.from_str(str(value))

    def to_python(self, value):
        if isinstance(value, RanjId):
            return value
        if value is None:
            return None
        # Delegate UUID string/UUID normalization to UUIDField, then wrap
        uuid_val = super().to_python(value)
        if uuid_val is None:
            return None
        return RanjId.from_str(str(uuid_val))

    def get_prep_value(self, value):
        if value is None:
            return None
        if isinstance(value, RanjId):
            # Return uuid.UUID so DB drivers (psycopg2, pyodbc) handle it correctly
            return value.to_uuid()
        if isinstance(value, uuid_mod.UUID):
            return value
        return uuid_mod.UUID(str(value))

    def formfield(self, **kwargs):
        return super().formfield(**{"form_class": RanjIdFormField, **kwargs})

    def deconstruct(self):
        name, path, args, kwargs = super().deconstruct()
        return name, "heeranjid_django.fields.RanjIdField", args, kwargs
