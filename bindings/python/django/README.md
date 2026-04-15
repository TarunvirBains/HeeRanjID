# heeranjid-django

> **HeerRanjId** ([ɦiːɾ.ɾaːnd͡ʒ.ɪd])

Django integration for [HeerRanjId](https://github.com/TarunvirBains/HeeRanjID) — a Snowflake-inspired distributed ID system that gives you compact, time-ordered primary keys without the drawbacks of auto-increment or plain UUIDs.

Snowflake IDs (originally from Twitter) are a well-established approach to distributed ID generation. HeeRanjID is a plug-and-play implementation of that concept for Django, with first-class PostgreSQL and SQL Server support and a UUID-compatible external representation built in.

---

## Why not just use UUIDs?

UUIDs are portable and globally unique, but they are a poor choice for primary keys:

- **Random UUIDs fragment your indexes.** Each insert lands in a random position in the B-tree, causing page splits and cache thrashing at scale.
- **They are large.** 128 bits vs 64 bits — doubles the size of every foreign key and join column.
- **They leak nothing useful.** You cannot tell when a record was created from its UUID.

Auto-increment integers avoid the indexing problem but are not safe for distributed systems — two nodes will collide.

HeeRanjID gives you a third option.

---

## The ID model

HeeRanjID defines two related identifier types:

### HeerId — the default format

HeerId is a **64-bit, time-ordered integer** — the same size as a `bigint`. It is composed from a millisecond timestamp, a node identifier, and a sequence counter (Snowflake-style). This means:

- Inserts are **sequential** — new rows land at the end of the index, not scattered throughout it.
- IDs are **globally unique** across nodes without coordination.
- You can extract **when** a record was created directly from its ID.
- It fits in a `bigint` column — no schema changes, no UUID columns, no type casting.

Limits: up to **511 nodes**, **8,191 IDs per node per millisecond**.

### RanjId — the upgrade format

RanjId is a **128-bit, UUIDv8-compatible identifier**. It stores in a standard `uuid` column and is accepted anywhere a UUID is expected.

RanjId is not random. It encodes the same timestamp, node, and sequence components as a HeerId, with more headroom:

- Up to **32,767 nodes** and **65,535 IDs per node per timestamp unit**
- **Sub-millisecond precision** — nanoseconds by default, configurable down to femtoseconds
- **UUID-compatible** — works in `uuid` columns, UUID form fields, admin, and DRF without extra configuration

### Migrating from HeerId to RanjId

When a system outgrows HeerId's limits, it can migrate to RanjId. The conversion is lossless — every HeerId maps to exactly one RanjId — and the `HeeRanjIdConversion` migration operation handles the column type change and all foreign key updates without disrupting a running system.

---

## Installation

```bash
pip install heeranjid-django
```

Requires `heeranjid` (installed automatically) and Django 4.2 or later.

### Database setup

Run the initial migration to install the ID generation functions in your database:

```bash
python manage.py migrate heeranjid_django
```

This installs the `generate_id()`, `generate_ranjid()`, and related SQL functions into your PostgreSQL or SQL Server database. IDs can then be generated at the application level or directly in the database.

### Settings

Add your node ID to Django settings:

```python
HEERANJID_NODE_ID = int(os.environ["NODE_ID"])
```

Each application node (process, container, worker) should have a unique node ID. HeerId supports up to 512 nodes; RanjId supports up to 32,768.

---

## Usage

### The quickest path — use the mixin

```python
from django.db import models
from heeranjid_django import HeeRanjIdPKMixin, HeeRanjIdFieldType, HeeRanjIdPrefetch


class Article(HeeRanjIdPKMixin, models.Model):
    class HeeRanjId:
        field_type = HeeRanjIdFieldType.HEERID   # 64-bit integer primary key
        prefetch = HeeRanjIdPrefetch.SAVE         # generate ID on save

    title = models.CharField(max_length=255)
    body = models.TextField()
```

`HeeRanjIdPKMixin` injects the correct primary key field and manager before Django finalises the model. No manual field declaration needed.

To use RanjId instead:

```python
class Article(HeeRanjIdPKMixin, models.Model):
    class HeeRanjId:
        field_type = HeeRanjIdFieldType.RANJID
        prefetch = HeeRanjIdPrefetch.SAVE
```

### Prefetch modes

| Mode | Behaviour |
|---|---|
| `SAVE` | ID is generated in `pre_save()` — the default |
| `INIT` | ID is generated when the model is instantiated |
| `MANUAL` | You assign the ID yourself |

### Declaring fields explicitly

If you prefer explicit field declarations:

```python
from heeranjid_django.fields import HeerIdField, RanjIdField

class Order(models.Model):
    id = HeerIdField(primary_key=True)
    objects = HeeRanjIdManager()
```

---

## Bulk operations

Standard Django `bulk_create` does not call `pre_save()`, so IDs would not be assigned automatically. The `HeeRanjIdManager` overrides `bulk_create` to handle this transparently:

```python
articles = [Article(title=f"Post {i}") for i in range(1000)]
Article.objects.bulk_create(articles)
```

This pre-fetches a batch of IDs from the database in a single call, assigns them to any objects missing an ID, then delegates to Django's `bulk_create` normally. It is efficient and safe for large inserts.

### Pre-fetching IDs manually

If you need IDs before saving — for example, to build related objects in memory before committing — you can prefetch them directly:

```python
from heeranjid_django import prefetch_ids

ids = prefetch_ids(Article, 500)
# ids is a list of 500 HeerId (or RanjId) values
```

---

## Database-backed generation

ID generation happens in the database, not in application code. This means:

- **Multiple services can share the same ID space** without coordination — they all call the same SQL functions.
- **Batch allocation is efficient** — fetching 1,000 IDs is a single round-trip.
- **Consistency is guaranteed** — the same generation algorithm runs regardless of which language or service is generating the ID.

This is the recommended approach for distributed systems. If you are running a single-process Django application, application-level generation works just as well.

---

## Migrations

### Initial setup

The `heeranjid_django` migration installs the SQL schema into your database. It is reversible — rolling back removes all installed functions and tables cleanly.

### Replacing an existing UUIDField with RanjIdField

If you have a model that currently uses Django's built-in `UUIDField` and want to switch to `RanjIdField`, you can do so with a standard `AlterField` migration — **as long as the column has no existing rows** (or the table has not yet been created).

`RanjIdField` stores `uuid` on PostgreSQL (the same column type as `UUIDField`), so the migration is a zero-cost metadata change on that backend. On SQL Server, `UUIDField` uses `uniqueidentifier` while `RanjIdField` uses `BINARY(16)` (to preserve correct byte order); the column type change requires an empty column.

**Step 1 — update your model**

```python
# Before
from django.db import models
import uuid

class Widget(models.Model):
    id = models.UUIDField(primary_key=True, default=uuid.uuid4)

# After
from heeranjid_django.fields import RanjIdField
from heeranjid_django.managers import HeeRanjIdManager

class Widget(models.Model):
    id = RanjIdField(primary_key=True)
    objects = HeeRanjIdManager()
```

**Step 2 — generate the migration**

```bash
python manage.py makemigrations
```

Django detects that the field class changed and generates an `AlterField` operation.

**Step 3 — apply the migration**

```bash
python manage.py migrate
```

On **PostgreSQL** the migration is a no-op at the schema level — the underlying column type (`uuid`) is identical to `UUIDField`, so no `ALTER COLUMN` is executed. However, this migration is only safe when the column is **empty**. `RanjId` validates UUIDv8 version bits on every read; any existing rows containing UUIDv4 (or other non-UUIDv8) values will raise `InvalidRanjIdVersion` when fetched.

On **SQL Server** the migration changes the column from `uniqueidentifier` to `BINARY(16)`, which also requires an **empty** column.

> **Why BINARY(16) on SQL Server?**
> SQL Server's `uniqueidentifier` stores GUIDs in mixed-endian format (first three UUID components are byte-swapped from RFC 4122). RanjId encodes timestamp, node, and sequence into specific bit positions using big-endian layout; storing via `uniqueidentifier` would silently corrupt those bits. `BINARY(16)` preserves the raw big-endian bytes faithfully.

### Converting between HeeRanjID formats

If you need to migrate an existing model from HeerId to RanjId primary keys (or vice versa) — where both columns already contain HeeRanjID-generated values — use the `HeeRanjIdConversion` migration operation:

```python
from heeranjid_django.operations import HeeRanjIdConversion

class Migration(migrations.Migration):
    operations = [
        HeeRanjIdConversion(
            model="myapp.Article",
            direction="heerid_to_ranjid",
            foreign_keys=[
                ("myapp_comment", "article_id"),
            ],
            chunk_size=10000,
        ),
    ]
```

This converts the primary key column, updates all specified foreign key columns, and handles constraints. It is chunked to avoid locking large tables. The operation is reversible.

---

## Supported databases

| Database | HeerId | RanjId |
|---|---|---|
| PostgreSQL | `bigint` | `uuid` |
| SQL Server | `bigint` | `BINARY(16)` |

---

## Relationship to the core package

`heeranjid-django` depends on `heeranjid`, the Python extension module (built in Rust). The `HeerId` and `RanjId` types you get back from fields and managers are instances from that package — you can use them in arithmetic, comparisons, and conversions directly.

```python
article = Article.objects.get(pk=some_id)
print(article.pk)               # HeerId(...)
print(article.pk.to_ranjid())   # RanjId(...) — UUID-compatible
print(int(article.pk))          # raw integer
```

---

## License

MIT
