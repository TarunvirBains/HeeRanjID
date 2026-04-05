import uuid as uuid_mod

from django.core.exceptions import ImproperlyConfigured
from django.db import connection, models
from heeranjid import HeerId, RanjId


def _get_node_id():
    """Read HEERANJID_NODE_ID from Django settings."""
    from django.conf import settings

    node_id = getattr(settings, "HEERANJID_NODE_ID", None)
    if node_id is None:
        raise ImproperlyConfigured(
            "HEERANJID_NODE_ID must be set in Django settings. "
            "Example: HEERANJID_NODE_ID = int(os.environ['NODE_ID'])"
        )
    return int(node_id)


def _generate_heer_ids(count):
    """Generate a batch of HeerId values via SQL."""
    node_id = _get_node_id()
    cursor = connection.cursor()
    if connection.vendor == "microsoft":
        cursor.execute(f"EXEC generate_ids @in_node_id = {node_id}, @requested_count = {count}")
    else:
        cursor.execute(f"SELECT id FROM generate_ids({node_id}, {count})")
    rows = cursor.fetchall()
    return [HeerId(int(r[0])) for r in rows]


def _generate_ranj_ids(count):
    """Generate a batch of RanjId values via SQL."""
    node_id = _get_node_id()
    cursor = connection.cursor()
    if connection.vendor == "microsoft":
        cursor.execute(f"EXEC generate_ranjids @in_node_id = {node_id}, @requested_count = {count}")
        rows = cursor.fetchall()
        return [RanjId.from_str(str(uuid_mod.UUID(bytes=bytes(r[0])))) for r in rows]
    else:
        cursor.execute(f"SELECT id FROM generate_ranjids({node_id}, {count})")
        rows = cursor.fetchall()
        return [RanjId.from_str(str(r[0])) for r in rows]


class HeeRanjIdManagerMixin:
    """Mixin for Django managers that support HeeRanjID bulk operations."""

    _heeranjid_enabled = True

    def heeranjid_bulk_create(self, objs, **kwargs):
        """Generate HeeRanjID values for objects missing them, then bulk_create."""
        from heeranjid_django.fields import HeerIdField, RanjIdField

        if not objs:
            return self.bulk_create(objs, **kwargs)

        model = self.model

        heer_fields = [f for f in model._meta.get_fields() if isinstance(f, HeerIdField)]
        ranj_fields = [f for f in model._meta.get_fields() if isinstance(f, RanjIdField)]

        for field in heer_fields:
            needs_id = [obj for obj in objs if getattr(obj, field.attname, None) is None]
            if needs_id:
                ids = _generate_heer_ids(len(needs_id))
                for obj, new_id in zip(needs_id, ids):
                    setattr(obj, field.attname, new_id)

        for field in ranj_fields:
            needs_id = [obj for obj in objs if getattr(obj, field.attname, None) is None]
            if needs_id:
                ids = _generate_ranj_ids(len(needs_id))
                for obj, new_id in zip(needs_id, ids):
                    setattr(obj, field.attname, new_id)

        return self.bulk_create(objs, **kwargs)


class HeeRanjIdManager(HeeRanjIdManagerMixin, models.Manager):
    """Django manager with HeeRanjID bulk create support."""

    pass
