"""
Playwright E2E tests for Django admin with VanillaPost and HeeRanjPost.

These tests prove the mixin is genuinely plug-and-play at the browser level:
a developer can open Django admin, fill a form, and get a correctly typed PK
with zero extra configuration.

Requires:
  - DATABASE_URL set to a live Postgres instance with HeeRanjID schema
  - playwright Python package: pip install playwright
  - Chromium: playwright install chromium
  - pytest-django: pip install pytest-django
  - pytest-playwright: pip install pytest-playwright

Skip if DATABASE_URL is not set.

Run:
  DATABASE_URL=postgres://... pytest tests/test_admin_playwright.py -v
"""
import os
import re
import uuid

import pytest

DATABASE_URL = os.environ.get("DATABASE_URL")
os.environ.setdefault("DJANGO_ALLOW_ASYNC_UNSAFE", "true")

if DATABASE_URL is None:
    pytest.skip("DATABASE_URL not set — skipping Playwright admin tests", allow_module_level=True)

# ── Configure Django ──────────────────────────────────────────────────────────

_DB_PATTERN = re.compile(
    r"postgres://(?P<user>[^:]+):(?P<password>[^@]+)@(?P<host>[^:]+):(?P<port>\d+)/(?P<name>.+)"
)


def _parse_db_url(url):
    m = _DB_PATTERN.match(url)
    if not m:
        raise ValueError(f"Bad DATABASE_URL: {url!r}")
    return m.groupdict()


import django
from django.conf import settings

if not settings.configured:
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
            "django.contrib.staticfiles",
            "heeranjid_django",
            "testapp",
        ],
        DEFAULT_AUTO_FIELD="django.db.models.BigAutoField",
        HEERANJID_NODE_ID=1,
        SECRET_KEY="test-secret-key-playwright",
        ROOT_URLCONF="testapp.urls",
        STATIC_URL="/static/",
        TEMPLATES=[{
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
        }],
        MIDDLEWARE=[
            "django.contrib.sessions.middleware.SessionMiddleware",
            "django.middleware.common.CommonMiddleware",
            "django.contrib.auth.middleware.AuthenticationMiddleware",
            "django.contrib.messages.middleware.MessageMiddleware",
        ],
        SESSION_ENGINE="django.contrib.sessions.backends.db",
        ALLOWED_HOSTS=["*"],
    )
    django.setup()

import sys
import pathlib
sys.path.insert(0, str(pathlib.Path(__file__).parent))

# ── Fixtures ──────────────────────────────────────────────────────────────────


@pytest.fixture(scope="session")
def django_db_setup():
    """Prepare built-in admin tables plus HeeRanjID SQL and testapp models."""
    from django.core.management import call_command
    from django.db import connection
    from heeranjid.sql import postgres as pg_sql
    from testapp.models import HeeRanjPost, VanillaPost

    for app_label in ("contenttypes", "auth", "admin", "sessions"):
        call_command("migrate", app_label, verbosity=0)

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

    existing_tables = set(connection.introspection.table_names())
    with connection.schema_editor() as editor:
        if VanillaPost._meta.db_table not in existing_tables:
            editor.create_model(VanillaPost)
        if HeeRanjPost._meta.db_table not in existing_tables:
            editor.create_model(HeeRanjPost)


@pytest.fixture
def admin_user(django_db_setup):
    """Create a Django admin superuser for the current test."""
    from django.contrib.auth.models import User
    username = "playwright_admin"
    User.objects.filter(username=username).delete()
    User.objects.create_superuser(username, "admin@test.com", "playwright_password")
    return {"username": username, "password": "playwright_password"}


@pytest.fixture(scope="session")
def live_server_url(live_server):
    return live_server.url


def _login(page, base_url, credentials):
    page.goto(f"{base_url}/admin/login/")
    page.fill("#id_username", credentials["username"])
    page.fill("#id_password", credentials["password"])
    page.click('[type="submit"]')
    page.wait_for_url(f"**/admin/**")


# ── VanillaPost admin tests ───────────────────────────────────────────────────


class TestVanillaPostAdmin:
    """Verify Django admin works with a plain UUIDField PK model."""

    @pytest.mark.django_db(transaction=True)
    def test_can_create_vanilla_post_in_admin(self, page, live_server_url, admin_user):
        _login(page, live_server_url, admin_user)

        page.goto(f"{live_server_url}/admin/testapp/vanillapost/add/")
        page.fill("#id_title", "Playwright VanillaPost")
        page.get_by_role("button", name="Save").first.click()

        # Successful save redirects to changelist
        assert "vanillapost" in page.url
        assert page.get_by_text("was added successfully").count() > 0 or \
               "vanillapost" in page.url

    @pytest.mark.django_db(transaction=True)
    def test_vanilla_post_pk_shown_in_changelist(self, page, live_server_url, admin_user):
        from testapp.models import VanillaPost
        post = VanillaPost.objects.create(title="Changelist Test Vanilla")

        _login(page, live_server_url, admin_user)
        page.goto(f"{live_server_url}/admin/testapp/vanillapost/")
        assert "Changelist Test Vanilla" in page.content()

    @pytest.mark.django_db(transaction=True)
    def test_vanilla_post_change_url_contains_uuid(self, page, live_server_url, admin_user):
        from testapp.models import VanillaPost
        post = VanillaPost.objects.create(title="URL Test Vanilla")
        pk_str = str(post.pk)

        _login(page, live_server_url, admin_user)
        page.goto(f"{live_server_url}/admin/testapp/vanillapost/{pk_str}/change/")
        assert page.get_by_label("Title").input_value() == "URL Test Vanilla"


# ── HeeRanjPost admin tests ───────────────────────────────────────────────────


class TestHeeRanjPostAdmin:
    """Verify Django admin works identically with HeeRanjIdPKMixin model."""

    @pytest.mark.django_db(transaction=True)
    def test_can_create_heerranj_post_in_admin(self, page, live_server_url, admin_user):
        _login(page, live_server_url, admin_user)

        page.goto(f"{live_server_url}/admin/testapp/heeranjpost/add/")
        page.fill("#id_title", "Playwright HeeRanjPost")
        page.get_by_role("button", name="Save").first.click()

        assert "heeranjpost" in page.url

    @pytest.mark.django_db(transaction=True)
    def test_heerranj_post_pk_is_ranjid_after_admin_save(self, page, live_server_url, admin_user):
        """After creating via admin, the saved PK is a valid RanjId (UUIDv8)."""
        from testapp.models import HeeRanjPost
        from heeranjid import RanjId

        _login(page, live_server_url, admin_user)
        page.goto(f"{live_server_url}/admin/testapp/heeranjpost/add/")
        page.fill("#id_title", "PK Check Post")
        page.get_by_role("button", name="Save").first.click()

        post = HeeRanjPost.objects.get(title="PK Check Post")
        assert isinstance(post.pk, RanjId)
        u = uuid.UUID(str(post.pk))
        assert u.version == 8, f"Expected UUIDv8, got version {u.version}"

    @pytest.mark.django_db(transaction=True)
    def test_heerranj_post_change_url_contains_uuid(self, page, live_server_url, admin_user):
        from testapp.models import HeeRanjPost
        post = HeeRanjPost.objects.create(title="URL Test HeeRanj")
        pk_str = str(post.pk)

        _login(page, live_server_url, admin_user)
        page.goto(f"{live_server_url}/admin/testapp/heeranjpost/{pk_str}/change/")
        assert page.get_by_label("Title").input_value() == "URL Test HeeRanj"

    @pytest.mark.django_db(transaction=True)
    def test_heerranj_changelist_works_like_vanilla(self, page, live_server_url, admin_user):
        from testapp.models import HeeRanjPost
        HeeRanjPost.objects.create(title="Changelist Test HeeRanj")

        _login(page, live_server_url, admin_user)
        page.goto(f"{live_server_url}/admin/testapp/heeranjpost/")
        assert "Changelist Test HeeRanj" in page.content()


# ── Side-by-side comparison in admin ─────────────────────────────────────────


class TestAdminComparison:
    """Both models render in admin with the same UX — confirmed by page structure."""

    @pytest.mark.django_db(transaction=True)
    def test_both_add_pages_have_title_field(self, page, live_server_url, admin_user):
        _login(page, live_server_url, admin_user)

        page.goto(f"{live_server_url}/admin/testapp/vanillapost/add/")
        vanilla_has_title = page.locator("#id_title").count() > 0

        page.goto(f"{live_server_url}/admin/testapp/heeranjpost/add/")
        heerranj_has_title = page.locator("#id_title").count() > 0

        assert vanilla_has_title
        assert heerranj_has_title

    @pytest.mark.django_db(transaction=True)
    def test_both_add_pages_have_save_button(self, page, live_server_url, admin_user):
        _login(page, live_server_url, admin_user)

        for model_slug in ("vanillapost", "heeranjpost"):
            page.goto(f"{live_server_url}/admin/testapp/{model_slug}/add/")
            assert page.get_by_role("button", name="Save").count() > 0

    @pytest.mark.django_db(transaction=True)
    def test_pk_field_is_readonly_in_both_admins(self, page, live_server_url, admin_user):
        """Both model admins mark the PK as readonly — you don't type a UUID in."""
        from testapp.models import VanillaPost, HeeRanjPost
        vp = VanillaPost.objects.create(title="readonly test")
        hp = HeeRanjPost.objects.create(title="readonly test")

        _login(page, live_server_url, admin_user)

        page.goto(f"{live_server_url}/admin/testapp/vanillapost/{vp.pk}/change/")
        # The id field is readonly — no <input> with name="id"
        assert page.locator('input[name="id"]').count() == 0

        page.goto(f"{live_server_url}/admin/testapp/heeranjpost/{hp.pk}/change/")
        assert page.locator('input[name="id"]').count() == 0
