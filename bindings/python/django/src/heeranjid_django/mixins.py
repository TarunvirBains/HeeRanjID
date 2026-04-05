from django.core.exceptions import ImproperlyConfigured
from django.db import models
from django.db.models.base import ModelBase

from heeranjid_django.enums import HeeRanjIdFieldType, HeeRanjIdPrefetch
from heeranjid_django.fields import HeerIdField, RanjIdField
from heeranjid_django.managers import HeeRanjIdManager


class HeeRanjIdMeta(ModelBase):
    """
    Custom metaclass for HeeRanjIdPKMixin that injects the correct primary key
    field (HeerIdField or RanjIdField) before Django's ModelBase finalises the
    model, ensuring the auto-pk logic is bypassed cleanly.
    """

    def __new__(mcs, name, bases, namespace, **kwargs):
        is_abstract = getattr(namespace.get("Meta"), "abstract", False)

        # Resolve HeeRanjId config from the namespace or nearest base
        config = namespace.get("HeeRanjId")
        if config is None:
            for base in bases:
                c = getattr(base, "HeeRanjId", None)
                if c is not None:
                    config = c
                    break

        if config is not None and not is_abstract:
            field_type = getattr(config, "field_type", HeeRanjIdFieldType.HEERID)
            prefetch = getattr(config, "prefetch", HeeRanjIdPrefetch.SAVE)

            if not isinstance(field_type, HeeRanjIdFieldType):
                raise ImproperlyConfigured(
                    f"HeeRanjId.field_type must be a HeeRanjIdFieldType enum, "
                    f"got {field_type!r}"
                )
            if not isinstance(prefetch, HeeRanjIdPrefetch):
                raise ImproperlyConfigured(
                    f"HeeRanjId.prefetch must be a HeeRanjIdPrefetch enum, "
                    f"got {prefetch!r}"
                )

            if "id" not in namespace:
                if field_type == HeeRanjIdFieldType.RANJID:
                    namespace["id"] = RanjIdField(primary_key=True)
                else:
                    namespace["id"] = HeerIdField(primary_key=True)

        return super().__new__(mcs, name, bases, namespace, **kwargs)


class HeeRanjIdPKMixin(models.Model, metaclass=HeeRanjIdMeta):
    """
    Abstract model mixin that provides a HeeRanjID primary key.

    Configure via inner class:

        class MyModel(HeeRanjIdPKMixin, models.Model):
            class HeeRanjId:
                field_type = HeeRanjIdFieldType.HEERID  # or RANJID
                prefetch = HeeRanjIdPrefetch.SAVE  # or INIT, MANUAL

    Automatically sets the primary key field and HeeRanjIdManager.
    The HeeRanjIdMeta metaclass injects the correct field before Django's
    ModelBase finalises the model, bypassing the default auto-pk logic.
    """

    class Meta:
        abstract = True

    class HeeRanjId:
        field_type = HeeRanjIdFieldType.HEERID
        prefetch = HeeRanjIdPrefetch.SAVE

    objects = HeeRanjIdManager()
