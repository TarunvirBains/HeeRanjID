from django.core.exceptions import ImproperlyConfigured
from django.db import models
from django.db.models.base import ModelBase

from heeranjid_django.enums import HeeRanjIdFieldType, HeeRanjIdPrefetch
from heeranjid_django.fields import HeerIdField, RanjIdField
from heeranjid_django.managers import HeeRanjIdManager


def _post_init_generate_id(sender, instance, **kwargs):
    """Signal receiver: generate a HeeRanjID at model instantiation time (INIT mode)."""
    pk_field = instance._meta.pk
    if pk_field is None:
        return
    if getattr(instance, pk_field.attname, None) is None:
        # Reuse pre_save logic with add=True to generate and assign the ID
        pk_field.pre_save(instance, add=True)


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

        is_proxy = getattr(namespace.get("Meta"), "proxy", False)

        if config is not None and not is_abstract and not is_proxy:
            field_type = getattr(config, "field_type", HeeRanjIdFieldType.HEERID)
            prefetch = getattr(config, "prefetch", HeeRanjIdPrefetch.SAVE)

            if not isinstance(field_type, HeeRanjIdFieldType):
                raise ImproperlyConfigured(
                    f"HeeRanjId.field_type must be a HeeRanjIdFieldType enum, got {field_type!r}"
                )
            if not isinstance(prefetch, HeeRanjIdPrefetch):
                raise ImproperlyConfigured(
                    f"HeeRanjId.prefetch must be a HeeRanjIdPrefetch enum, got {prefetch!r}"
                )

            if "id" not in namespace:
                if field_type == HeeRanjIdFieldType.RANJID:
                    namespace["id"] = RanjIdField(primary_key=True)
                else:
                    namespace["id"] = HeerIdField(primary_key=True)

            # MANUAL mode: mark the model so pre_save skips auto-generation.
            if prefetch == HeeRanjIdPrefetch.MANUAL:
                namespace["_heeranjid_prefetch_manual"] = True

        cls = super().__new__(mcs, name, bases, namespace, **kwargs)

        # INIT mode: connect post_init signal so IDs are generated at instantiation.
        # This runs after the class exists so we can pass it as sender.
        if (
            config is not None
            and not is_abstract
            and not is_proxy
            and getattr(config, "prefetch", HeeRanjIdPrefetch.SAVE) == HeeRanjIdPrefetch.INIT
        ):
            from django.db.models.signals import post_init

            post_init.connect(_post_init_generate_id, sender=cls, weak=False)

        return cls


class HeeRanjIdPKMixin(models.Model, metaclass=HeeRanjIdMeta):
    """
    Abstract model mixin that provides a HeeRanjID primary key.

    Configure via inner class:

        class MyModel(HeeRanjIdPKMixin, models.Model):
            class HeeRanjId:
                field_type = HeeRanjIdFieldType.HEERID  # or RANJID
                prefetch = HeeRanjIdPrefetch.SAVE  # SAVE, INIT, or MANUAL

    Prefetch modes:
        SAVE   — ID generated in pre_save() just before the INSERT (default).
        INIT   — ID generated when the model instance is created (__init__),
                 so article.pk is available before save(). Requires a live DB
                 connection at instantiation time.
        MANUAL — No automatic generation. You must assign the PK yourself.
                 Saving without an assigned PK will raise a DB NOT NULL error.

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
