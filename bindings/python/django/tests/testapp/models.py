"""
Two parallel Django models for comparison testing:

  VanillaPost   — standard UUIDField(primary_key=True, default=uuid.uuid4)
  HeeRanjPost   — HeeRanjIdPKMixin with field_type=RANJID, prefetch=SAVE

Both store a uuid-shaped primary key and a text title field.
The comparison tests in test_mixin_comparison.py exercise all meaningful
application-level behaviours for both models side by side.
"""
import uuid

from django.db import models

from heeranjid_django import HeeRanjIdFieldType, HeeRanjIdPKMixin, HeeRanjIdPrefetch


class VanillaPost(models.Model):
    """Baseline: Django UUIDField PK with uuid4 default."""

    id = models.UUIDField(primary_key=True, default=uuid.uuid4, editable=False)
    title = models.CharField(max_length=255)
    created_at = models.DateTimeField(auto_now_add=True)

    class Meta:
        app_label = "testapp"
        ordering = ["created_at"]


class HeeRanjPost(HeeRanjIdPKMixin, models.Model):
    """Comparison: RanjId PK via HeeRanjIdPKMixin."""

    class HeeRanjId:
        field_type = HeeRanjIdFieldType.RANJID
        prefetch = HeeRanjIdPrefetch.SAVE

    title = models.CharField(max_length=255)
    created_at = models.DateTimeField(auto_now_add=True)

    class Meta:
        app_label = "testapp"
        ordering = ["id"]  # RanjId is time-ordered so id ordering = created_at ordering
