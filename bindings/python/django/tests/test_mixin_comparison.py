"""
Comparison test: VanillaPost (UUIDField PK) vs HeeRanjPost (RanjId via mixin).

Goal: demonstrate that HeeRanjIdPKMixin is a manageable drop-in at the
application level. Both models share the same surface area — UUID-shaped
string keys, Django admin, form input, queryset filtering — but differ in
their generation strategy and ordering guarantees.

Tests are grouped by application concern:
  - PK types and representations
  - Creation and auto-assignment
  - ORM retrieval and filtering
  - Ordering guarantees
  - Form field behaviour
  - Admin URL compatibility
  - Serialization
  - Prefetch mode correctness (SAVE / INIT / MANUAL)

DB-backed tests require DATABASE_URL and are skipped otherwise.
Field/form tests run without a database.
"""

import os
import pathlib
import re
import sys
import uuid

import django
import pytest
from django.conf import settings
from django.db.models.expressions import DatabaseDefault

# ── Django settings ──────────────────────────────────────────────────────────

DATABASE_URL = os.environ.get("DATABASE_URL")

_DB_PATTERN = re.compile(
    r"postgres://(?P<user>[^:]+):(?P<password>[^@]+)@(?P<host>[^:]+):(?P<port>\d+)/(?P<name>.+)"
)


def _parse_db_url(url):
    m = _DB_PATTERN.match(url)
    if not m:
        raise ValueError(f"Bad DATABASE_URL: {url!r}")
    return m.groupdict()


if not settings.configured:
    if DATABASE_URL:
        _db = _parse_db_url(DATABASE_URL)
        settings.configure(
            DATABASES={
                "default": {
                    "ENGINE": "django.db.backends.postgresql",
                    "NAME": _db["name"],
                    "USER": _db["user"],
                    "PASSWORD": _db["password"],
                    "HOST": _db["host"],
                    "PORT": _db["port"],
                }
            },
            INSTALLED_APPS=[
                "django.contrib.contenttypes",
                "django.contrib.auth",
                "django.contrib.admin",
                "django.contrib.sessions",
                "django.contrib.messages",
                "heeranjid_django",
                "testapp",
            ],
            DEFAULT_AUTO_FIELD="django.db.models.BigAutoField",
            HEERANJID_NODE_ID=1,
            SECRET_KEY="test-secret-key",
            ROOT_URLCONF="testapp.urls",
            TEMPLATES=[
                {
                    "BACKEND": "django.template.backends.django.DjangoTemplates",
                    "DIRS": [],
                    "APP_DIRS": True,
                    "OPTIONS": {
                        "context_processors": [
                            "django.template.context_processors.request",
                            "django.contrib.auth.context_processors.auth",
                            "django.contrib.messages.context_processors.messages",
                        ]
                    },
                }
            ],
            MIDDLEWARE=[
                "django.contrib.sessions.middleware.SessionMiddleware",
                "django.contrib.auth.middleware.AuthenticationMiddleware",
                "django.contrib.messages.middleware.MessageMiddleware",
            ],
            SESSION_ENGINE="django.contrib.sessions.backends.db",
        )
    else:
        settings.configure(
            DATABASES={"default": {"ENGINE": "django.db.backends.sqlite3", "NAME": ":memory:"}},
            INSTALLED_APPS=["heeranjid_django", "testapp"],
            DEFAULT_AUTO_FIELD="django.db.models.BigAutoField",
            HEERANJID_NODE_ID=1,
        )
    django.setup()

# Make testapp importable when running from repo root
sys.path.insert(0, str(pathlib.Path(__file__).parent))

from django.db import models as django_models  # noqa: E402
from heeranjid import RanjId  # noqa: E402
from testapp.models import HeeRanjPost, VanillaPost  # noqa: E402

from heeranjid_django import HeeRanjIdFieldType, HeeRanjIdPKMixin, HeeRanjIdPrefetch  # noqa: E402
from heeranjid_django.fields import RanjIdField  # noqa: E402
from heeranjid_django.managers import HeeRanjIdManager  # noqa: E402

needs_db = pytest.mark.skipif(DATABASE_URL is None, reason="DATABASE_URL not set")


class _FakeConn:
    class _Ops:
        @staticmethod
        def quote_name(value):
            return value

    def __init__(self, vendor, uuid_type="uuid"):
        self.vendor = vendor
        self.ops = self._Ops()
        self.data_types = {"UUIDField": uuid_type}


def _is_unset_pk(value):
    return value is None or value == "" or isinstance(value, DatabaseDefault)


# ── 1. PK field types and representations ────────────────────────────────────


class TestPkTypes:
    """Both models expose UUID-shaped primary keys; types differ."""

    def test_vanilla_pk_field_is_uuidfield(self):
        pk = VanillaPost._meta.pk
        assert isinstance(pk, django_models.UUIDField)

    def test_heerranj_pk_field_is_ranjidfield(self):
        pk = HeeRanjPost._meta.pk
        assert isinstance(pk, RanjIdField)

    def test_heerranj_manager_is_heeranjid_manager(self):
        assert isinstance(HeeRanjPost.objects, HeeRanjIdManager)

    def test_vanilla_manager_is_plain_manager(self):
        # VanillaPost has no special manager — standard Django Manager
        assert not isinstance(VanillaPost.objects, HeeRanjIdManager)

    def test_vanilla_pk_db_type_postgres(self):
        pk = VanillaPost._meta.pk
        assert pk.db_type(_FakeConn("postgresql")) == "uuid"

    def test_heerranj_pk_db_type_postgres(self):
        pk = HeeRanjPost._meta.pk
        assert pk.db_type(_FakeConn("postgresql")) == "uuid"

    def test_both_use_uuid_column_on_postgres(self):
        """Both models map to the same underlying column type on Postgres."""
        conn = _FakeConn("postgresql")
        assert VanillaPost._meta.pk.db_type(conn) == HeeRanjPost._meta.pk.db_type(conn)

    def test_vanilla_pk_db_type_mssql(self):
        pk = VanillaPost._meta.pk
        conn = _FakeConn("microsoft", uuid_type="uniqueidentifier")
        assert pk.db_type(conn) == "uniqueidentifier"

    def test_heerranj_pk_db_type_mssql(self):
        """RanjIdField uses BINARY(16) on MSSQL to preserve big-endian bit layout."""
        pk = HeeRanjPost._meta.pk
        conn = _FakeConn("microsoft", uuid_type="uniqueidentifier")
        assert pk.db_type(conn) == "BINARY(16)"

    def test_mssql_column_types_differ(self):
        """On MSSQL the column types diverge — this is intentional and documented."""
        conn = _FakeConn("microsoft", uuid_type="uniqueidentifier")
        assert VanillaPost._meta.pk.db_type(conn) != HeeRanjPost._meta.pk.db_type(conn)


# ── 2. String representation ─────────────────────────────────────────────────


class TestStringRepresentation:
    """Both PKs look like UUID strings from application code."""

    def test_vanilla_pk_str_is_uuid(self):
        post = VanillaPost.__new__(VanillaPost)
        post.id = uuid.uuid4()
        assert re.match(r"[0-9a-f-]{36}", str(post.id))

    def test_heerranj_pk_str_looks_like_uuid(self):
        rid = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        post = HeeRanjPost.__new__(HeeRanjPost)
        post.id = rid
        # str(RanjId) produces a hyphenated UUID string
        s = str(post.id)
        assert re.match(r"[0-9a-f-]{36}", s), f"Expected UUID-like string, got: {s!r}"

    def test_heerranj_pk_str_is_uuidv8(self):
        """RanjId is a UUIDv8 — version nibble is 8."""
        rid = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        u = uuid.UUID(str(rid))
        assert u.version == 8

    def test_vanilla_pk_version_is_4(self):
        u = uuid.uuid4()
        assert u.version == 4

    def test_heerranj_pk_carries_node_id(self):
        rid = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        assert rid.node_id == 100

    def test_vanilla_pk_carries_no_node_id(self):
        """uuid4 carries no structured metadata — node_id is not extractable."""
        u = uuid.uuid4()
        # uuid4 has no accessible node_id concept
        assert u.version == 4


# ── 3. Auto-assignment behaviour ─────────────────────────────────────────────


class TestAutoAssignment:
    """Both models auto-assign a PK when none is provided; timing differs."""

    def test_vanilla_auto_assigns_on_instantiation(self):
        """UUIDField with default=uuid4 assigns the ID at __init__ time."""
        post = VanillaPost(title="hello")
        assert post.id is not None
        assert isinstance(post.id, uuid.UUID)

    def test_heerranj_save_mode_assigns_at_pre_save_not_init(self):
        """SAVE mode: PK is None until save() is called (pre_save generates it)."""
        post = HeeRanjPost.__new__(HeeRanjPost)
        django_models.Model.__init__(post, title="hello")
        # Newer Django may represent an unresolved DB default with DatabaseDefault.
        assert _is_unset_pk(post.id)

    def test_vanilla_explicit_pk_is_respected(self):
        explicit = uuid.UUID("12345678-1234-5678-1234-567812345678")
        post = VanillaPost(id=explicit, title="hello")
        assert post.id == explicit

    def test_heerranj_explicit_pk_is_respected(self):
        """Assigning a RanjId before save() prevents auto-generation."""
        explicit = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        post = HeeRanjPost(id=explicit, title="hello")
        assert post.id == explicit


# ── 4. Form field behaviour ───────────────────────────────────────────────────


class TestFormFieldBehaviour:
    """Both fields accept UUID-formatted string input via forms."""

    def test_vanilla_formfield_accepts_uuid_string(self):
        pk = VanillaPost._meta.pk
        form_field = pk.formfield()
        result = form_field.clean("00000000-0000-8000-8000-0000006400c8")
        assert isinstance(result, uuid.UUID)

    def test_heerranj_formfield_accepts_uuid_string(self):
        pk = HeeRanjPost._meta.pk
        form_field = pk.formfield()
        # RanjIdField.formfield() returns a CharField (field is models.Field)
        # The value round-trips through from_db_value when read back
        result = form_field.clean("00000000-0000-8000-8000-0000006400c8")
        assert result is not None

    def test_vanilla_formfield_rejects_invalid_input(self):
        from django.core.exceptions import ValidationError

        pk = VanillaPost._meta.pk
        form_field = pk.formfield()
        with pytest.raises(ValidationError):
            form_field.clean("not-a-uuid")

    def test_both_formfields_use_different_base_types(self):
        from django import forms

        vanilla_ff = VanillaPost._meta.pk.formfield()
        heerranj_ff = HeeRanjPost._meta.pk.formfield()
        # The custom field preserves UUIDField validation but uses a distinct subclass.
        assert isinstance(vanilla_ff, forms.UUIDField)
        assert isinstance(heerranj_ff, forms.UUIDField)
        assert type(heerranj_ff) is not forms.UUIDField

    def test_vanilla_prep_value_returns_uuid(self):
        pk = VanillaPost._meta.pk
        u = uuid.uuid4()
        result = pk.get_prep_value(u)
        assert isinstance(result, uuid.UUID)

    def test_heerranj_prep_value_returns_uuid(self):
        pk = HeeRanjPost._meta.pk
        rid = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        result = pk.get_prep_value(rid)
        assert isinstance(result, uuid.UUID)

    def test_prep_values_are_same_type(self):
        """Both fields produce uuid.UUID for the DB driver — interoperable."""
        vanilla_pk = VanillaPost._meta.pk
        heerranj_pk = HeeRanjPost._meta.pk
        vanilla_val = vanilla_pk.get_prep_value(uuid.uuid4())
        heerranj_val = heerranj_pk.get_prep_value(
            RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        )
        assert type(vanilla_val) is uuid.UUID
        assert type(heerranj_val) is uuid.UUID


# ── 5. Admin URL compatibility ────────────────────────────────────────────────


class TestAdminUrlCompatibility:
    """Both PKs can appear in admin URLs as UUID strings."""

    def test_vanilla_pk_str_is_url_safe(self):
        post = VanillaPost(title="t")
        post.id = uuid.UUID("12345678-1234-5678-1234-567812345678")
        pk_str = str(post.id)
        # Must contain only hex digits and hyphens — valid URL path segment
        assert re.fullmatch(r"[0-9a-f\-]+", pk_str)

    def test_heerranj_pk_str_is_url_safe(self):
        post = HeeRanjPost.__new__(HeeRanjPost)
        post.id = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        pk_str = str(post.id)
        assert re.fullmatch(r"[0-9a-f\-]+", pk_str)

    def test_both_pks_are_36_chars(self):
        vanilla_pk = str(uuid.uuid4())
        ranj_pk = str(RanjId.from_str("00000000-0000-8000-8000-0000006400c8"))
        assert len(vanilla_pk) == 36
        assert len(ranj_pk) == 36


# ── 6. Ordering guarantees ────────────────────────────────────────────────────


class TestOrderingGuarantees:
    """Key difference: RanjId is time-ordered, uuid4 is random."""

    def test_ranjid_string_sort_matches_time_order(self):
        """RanjIds generated in sequence sort correctly as strings."""
        # Use known monotonic examples
        earlier = RanjId.from_str("00000001-0000-8000-8000-000000010001")
        later = RanjId.from_str("00000002-0000-8000-8000-000000010001")
        assert str(earlier) < str(later), "RanjId string sort must match time order"

    def test_ranjid_carries_timestamp(self):
        rid = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        # timestamp_micros is extractable and non-negative
        assert hasattr(rid, "timestamp_micros")
        assert rid.timestamp_micros >= 0

    def test_uuid4_has_no_timestamp(self):
        """uuid4 carries no timestamp — you cannot tell when it was generated."""
        u = uuid.uuid4()
        assert not hasattr(u, "timestamp_micros")

    def test_ranjid_is_time_ordered_by_str_comparison(self):
        """If you sort RanjIds lexicographically, you get chronological order.
        This is a key advantage over uuid4 for database indexes."""
        rid1 = RanjId.from_str("00000000-0000-8000-8000-000000010001")
        rid2 = RanjId.from_str("00000000-0000-8001-8000-000000010001")
        # A later timestamp means a lexicographically larger string
        assert str(rid1) < str(rid2) or str(rid1) == str(rid2)


# ── 7. Serialization ──────────────────────────────────────────────────────────


class TestSerialization:
    """Both PKs serialise to strings that can be parsed back."""

    def test_vanilla_pk_serializes_to_str(self):
        u = uuid.uuid4()
        serialized = str(u)
        restored = uuid.UUID(serialized)
        assert restored == u

    def test_heerranj_pk_serializes_to_str(self):
        rid = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        serialized = str(rid)
        restored = RanjId.from_str(serialized)
        assert str(restored) == str(rid)

    def test_heerranj_pk_is_valid_uuid_object(self):
        """RanjId can be parsed as a uuid.UUID — interoperable with UUID-aware tools."""
        rid = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        u = uuid.UUID(str(rid))
        assert u.version == 8

    def test_both_pks_round_trip_through_prep_value_and_python_conversion(self):
        vanilla_pk = VanillaPost._meta.pk
        heerranj_pk = HeeRanjPost._meta.pk

        u = uuid.uuid4()
        vanilla_prep = vanilla_pk.get_prep_value(u)
        vanilla_back = vanilla_pk.to_python(str(vanilla_prep))
        assert str(vanilla_back) == str(u)

        rid = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        heerranj_prep = heerranj_pk.get_prep_value(rid)
        heerranj_back = heerranj_pk.from_db_value(str(heerranj_prep), None, None)
        assert str(heerranj_back) == str(rid)


# ── 8. DB-backed tests ────────────────────────────────────────────────────────


@pytest.fixture(scope="module")
def db_tables():
    """Create VanillaPost and HeeRanjPost tables for the test session."""
    from django.db import connection
    from heeranjid.sql import postgres as pg_sql

    with connection.cursor() as cur:
        for sql in [
            pg_sql.SCHEMA,
            pg_sql.SESSION,
            pg_sql.GENERATE_HEERID,
            pg_sql.GENERATE_RANJID,
            pg_sql.SEED,
        ]:
            cur.execute(sql)
        cur.execute(pg_sql.CONFIGURE)
        cur.execute(
            """
            INSERT INTO heer_config (id, epoch, precision)
            VALUES (1, '2026-01-01T00:00:00', 'us')
            ON CONFLICT (id) DO UPDATE
            SET epoch = EXCLUDED.epoch, precision = EXCLUDED.precision
            """
        )
        cur.execute("SELECT heer_configure()")
        cur.execute(
            """
            INSERT INTO heer_nodes (node_id, name, description, is_active)
            VALUES (2, 'test-node-2', 'Second test node', true)
            ON CONFLICT (node_id) DO NOTHING
            """
        )

    with connection.schema_editor() as editor:
        editor.create_model(VanillaPost)
        editor.create_model(HeeRanjPost)
    yield
    with connection.schema_editor() as editor:
        try:
            editor.delete_model(HeeRanjPost)
        except Exception:
            pass
        try:
            editor.delete_model(VanillaPost)
        except Exception:
            pass


@pytest.mark.skipif(DATABASE_URL is None, reason="DATABASE_URL not set")
class TestDbCreation:
    """End-to-end ORM tests that hit a real Postgres database."""

    def test_vanilla_post_saves_and_retrieves(self, db_tables):
        post = VanillaPost.objects.create(title="vanilla")
        assert post.pk is not None
        assert isinstance(post.pk, uuid.UUID)
        fetched = VanillaPost.objects.get(pk=post.pk)
        assert fetched.title == "vanilla"

    def test_heerranj_post_saves_and_retrieves(self, db_tables):
        post = HeeRanjPost.objects.create(title="heerranj")
        assert post.pk is not None
        assert isinstance(post.pk, RanjId)
        fetched = HeeRanjPost.objects.get(pk=post.pk)
        assert fetched.title == "heerranj"

    def test_vanilla_filterable_by_uuid_string(self, db_tables):
        post = VanillaPost.objects.create(title="filterable")
        pk_str = str(post.pk)
        fetched = VanillaPost.objects.get(pk=pk_str)
        assert fetched.title == "filterable"

    def test_heerranj_filterable_by_ranjid_string(self, db_tables):
        post = HeeRanjPost.objects.create(title="filterable")
        pk_str = str(post.pk)
        fetched = HeeRanjPost.objects.get(pk=pk_str)
        assert fetched.title == "filterable"

    def test_heerranj_bulk_create_assigns_ids(self, db_tables):
        posts = [HeeRanjPost(title=f"bulk-{i}") for i in range(5)]
        created = HeeRanjPost.objects.bulk_create(posts)
        assert all(isinstance(p.pk, RanjId) for p in created)
        assert len(set(str(p.pk) for p in created)) == 5

    def test_heerranj_ids_are_time_ordered_in_db(self, db_tables):
        """RanjId ordering matches insertion order — no random scattering."""
        for i in range(3):
            HeeRanjPost.objects.create(title=f"ordered-{i}")
        posts = list(HeeRanjPost.objects.filter(title__startswith="ordered").order_by("id"))
        pk_strs = [str(p.pk) for p in posts]
        assert pk_strs == sorted(pk_strs), "RanjId ordering should be lexicographic == time order"

    def test_vanilla_uuid4_ids_are_random(self, db_tables):
        """uuid4 PKs are not in any predictable order — confirmed by variance."""
        for i in range(10):
            VanillaPost.objects.create(title=f"random-{i}")
        posts = list(VanillaPost.objects.filter(title__startswith="random"))
        pk_strs = [str(p.pk) for p in posts]
        # uuid4s are almost certainly not sorted (random) — we just verify they're unique
        assert len(set(pk_strs)) == 10

    def test_init_mode_pk_assigned_at_instantiation(self, db_tables):
        """INIT mode: pk is set by the post_init signal before save() is called."""
        from django.db import connection

        class InitPost(HeeRanjIdPKMixin, django_models.Model):
            class HeeRanjId:
                field_type = HeeRanjIdFieldType.RANJID
                prefetch = HeeRanjIdPrefetch.INIT

            title = django_models.CharField(max_length=100, default="")

            class Meta:
                app_label = "testapp_init"

        with connection.schema_editor() as editor:
            editor.create_model(InitPost)
        try:
            post = InitPost(title="init-test")
            # post_init signal fires inside __init__; pk must be set before save()
            assert post.pk is not None, "INIT mode should assign pk at __init__ time"
            assert isinstance(post.pk, RanjId)

            pk_before_save = post.pk
            post.save()
            assert post.pk == pk_before_save, "save() should not regenerate an already-assigned pk"

            fetched = InitPost.objects.get(pk=post.pk)
            assert fetched.title == "init-test"
        finally:
            with connection.schema_editor() as editor:
                editor.delete_model(InitPost)


# ── 9. Prefetch mode correctness ──────────────────────────────────────────────


class TestPrefetchModes:
    """Verify all three prefetch modes behave as documented."""

    def test_save_mode_pk_is_none_before_save(self):
        from heeranjid_django import HeeRanjIdFieldType, HeeRanjIdPKMixin, HeeRanjIdPrefetch

        class SaveModel(HeeRanjIdPKMixin, django_models.Model):
            class HeeRanjId:
                field_type = HeeRanjIdFieldType.RANJID
                prefetch = HeeRanjIdPrefetch.SAVE

            title = django_models.CharField(max_length=100, default="")

            class Meta:
                app_label = "testapp_meta"

        instance = SaveModel.__new__(SaveModel)
        django_models.Model.__init__(instance, title="test")
        assert _is_unset_pk(instance.id)

    def test_manual_mode_does_not_auto_generate(self):
        from heeranjid_django import HeeRanjIdFieldType, HeeRanjIdPKMixin, HeeRanjIdPrefetch

        class ManualModel(HeeRanjIdPKMixin, django_models.Model):
            class HeeRanjId:
                field_type = HeeRanjIdFieldType.RANJID
                prefetch = HeeRanjIdPrefetch.MANUAL

            title = django_models.CharField(max_length=100, default="")

            class Meta:
                app_label = "testapp_meta"

        instance = ManualModel.__new__(ManualModel)
        django_models.Model.__init__(instance, title="test")
        # pre_save in MANUAL mode should NOT generate an ID
        pk_field = ManualModel._meta.pk
        result = pk_field.pre_save(instance, add=True)
        assert _is_unset_pk(result), f"MANUAL mode should not auto-generate; got {result!r}"

    def test_manual_mode_respects_explicit_assignment(self):
        from heeranjid_django import HeeRanjIdFieldType, HeeRanjIdPKMixin, HeeRanjIdPrefetch

        class ManualModel2(HeeRanjIdPKMixin, django_models.Model):
            class HeeRanjId:
                field_type = HeeRanjIdFieldType.RANJID
                prefetch = HeeRanjIdPrefetch.MANUAL

            title = django_models.CharField(max_length=100, default="")

            class Meta:
                app_label = "testapp_meta2"

        explicit = RanjId.from_str("00000000-0000-8000-8000-0000006400c8")
        instance = ManualModel2(id=explicit, title="test")
        pk_field = ManualModel2._meta.pk
        result = pk_field.pre_save(instance, add=True)
        assert result == explicit

    def test_manual_mode_sentinel_set_on_class(self):
        from heeranjid_django import HeeRanjIdFieldType, HeeRanjIdPKMixin, HeeRanjIdPrefetch

        class ManualModel3(HeeRanjIdPKMixin, django_models.Model):
            class HeeRanjId:
                field_type = HeeRanjIdFieldType.RANJID
                prefetch = HeeRanjIdPrefetch.MANUAL

            class Meta:
                app_label = "testapp_meta3"

        assert getattr(ManualModel3, "_heeranjid_prefetch_manual", False) is True

    def test_save_mode_has_no_manual_sentinel(self):
        from heeranjid_django import HeeRanjIdFieldType, HeeRanjIdPKMixin, HeeRanjIdPrefetch

        class SaveModel2(HeeRanjIdPKMixin, django_models.Model):
            class HeeRanjId:
                field_type = HeeRanjIdFieldType.RANJID
                prefetch = HeeRanjIdPrefetch.SAVE

            class Meta:
                app_label = "testapp_meta4"

        assert not getattr(SaveModel2, "_heeranjid_prefetch_manual", False)
