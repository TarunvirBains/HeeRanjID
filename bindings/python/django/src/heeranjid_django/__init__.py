from heeranjid_django.fields import HeerIdField, RanjIdField
from heeranjid_django.managers import HeeRanjIdManager, HeeRanjIdManagerMixin, prefetch_ids

default_app_config = "heeranjid_django.apps.HeeranjidConfig"
__all__ = [
    "HeerIdField",
    "RanjIdField",
    "HeeRanjIdManager",
    "HeeRanjIdManagerMixin",
    "prefetch_ids",
    "HeeRanjIdFieldType",
    "HeeRanjIdPrefetch",
    "HeeRanjIdPKMixin",
    "HeeRanjIdConversion",
]

# Lazily import Django-model-dependent symbols so this module can be imported
# before django.setup() is called (e.g. during app registry population).
_lazy = {
    "HeeRanjIdFieldType": "heeranjid_django.enums",
    "HeeRanjIdPrefetch": "heeranjid_django.enums",
    "HeeRanjIdPKMixin": "heeranjid_django.mixins",
    "HeeRanjIdConversion": "heeranjid_django.operations",
}


def __getattr__(name):
    if name in _lazy:
        import importlib

        module = importlib.import_module(_lazy[name])
        value = getattr(module, name)
        globals()[name] = value  # cache for subsequent access
        return value
    raise AttributeError(f"module 'heeranjid_django' has no attribute {name!r}")
