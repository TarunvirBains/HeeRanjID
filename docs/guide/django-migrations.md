# Django migrations with HeeRanjID

HeeRanjID ships two Django migration operations that encode the
asc↔desc and HeerId↔RanjId flows from the operator playbooks:

- `HeeRanjIdConversion` — HeerId (`BIGINT`) ↔ RanjId (`UUID`/`BINARY(16)`).
  Ships since v0.2.x.
- `HeeRanjIdDirectionFlip` — asc ↔ desc direction flip for either ID
  family (HeerId ↔ HeerIdDesc, RanjId ↔ RanjIdDesc). Ships in v0.3.1.

Both are vendor-aware — the same migration works against Postgres and
MSSQL, with dispatch logic in the operation itself.

---

## 1. When to use `HeeRanjIdConversion`

Use when your application has outgrown 8-byte `HeerId` and needs the
larger `RanjId` (more than 511 nodes, more than 8 191 IDs per
node-millisecond, or sub-millisecond precision).

**Example:** a single-node project stored on `BIGSERIAL` / `HeerId`
now needs to scale to 20 writer nodes with microsecond precision.

```python
# app/migrations/0042_heerid_to_ranjid.py
from django.db import migrations
from heeranjid_django import HeeRanjIdConversion


class Migration(migrations.Migration):
    dependencies = [("app", "0041_...")]

    operations = [
        HeeRanjIdConversion(
            model="app.Event",
            direction=HeeRanjIdConversion.DIRECTION_HEERID_TO_RANJID,
            foreign_keys=[
                ("app_eventattachment", "event_id"),
                ("app_eventtag", "event_id"),
            ],
            chunk_size=10_000,
        ),
        # Accompanying AlterField that changes the model definition.
        migrations.AlterField(
            model_name="event",
            name="id",
            field=heeranjid_django.RanjIdField(primary_key=True),
        ),
    ]
```

---

## 2. When to use `HeeRanjIdDirectionFlip`

Use when chronological queries dominate — most reads are
`ORDER BY id DESC LIMIT N` or reverse keyset pagination — and you want
the PK index to serve those directly without a reverse scan or a
secondary DESC index.

**Example:** an `audit_log` table whose read pattern is "most recent
N entries." Flipping to `HeerIdDescField` means `ORDER BY id` is now
physically reverse-chronological.

```python
# app/migrations/0050_events_id_to_desc.py
from django.db import migrations
import heeranjid_django
from heeranjid_django import HeeRanjIdDirectionFlip


class Migration(migrations.Migration):
    dependencies = [("app", "0049_...")]

    operations = [
        HeeRanjIdDirectionFlip(
            model="app.AuditLog",
            direction=HeeRanjIdDirectionFlip.DIRECTION_HEERID_TO_HEERID_DESC,
        ),
        migrations.AlterField(
            model_name="auditlog",
            name="id",
            field=heeranjid_django.HeerIdDescField(primary_key=True),
        ),
    ]
```

**Tradeoff:** the trigger-driven autofill window (post-migration,
before `DROP COLUMN id`) pays one extra UPDATE per INSERT/UPDATE.
For high-write tables, benchmark before committing.

---

## 3. Worked example — `HeerIdField` → `HeerIdDescField`

### 3.1 Model change

```python
# Before
class Event(models.Model):
    id = HeerIdField(primary_key=True)
    # ...

# After
class Event(models.Model):
    id = HeerIdDescField(primary_key=True)
    # ...
```

### 3.2 Auto-generated migration

Running `python manage.py makemigrations` produces an `AlterField`
operation. **You need to augment it** with the `HeeRanjIdDirectionFlip`
op so the data migration actually happens:

```python
# app/migrations/0050_alter_event_id.py (after manual edit)
from django.db import migrations
import heeranjid_django
from heeranjid_django import HeeRanjIdDirectionFlip


class Migration(migrations.Migration):
    dependencies = [("app", "0049_...")]

    operations = [
        # The direction flip runs first — it installs the trigger,
        # backfills, and swaps the PK.
        HeeRanjIdDirectionFlip(
            model="app.Event",
            direction=HeeRanjIdDirectionFlip.DIRECTION_HEERID_TO_HEERID_DESC,
        ),
        # AlterField just updates Django's model state to match the DB.
        migrations.AlterField(
            model_name="event",
            name="id",
            field=heeranjid_django.HeerIdDescField(primary_key=True),
        ),
    ]
```

### 3.3 Run the migration

```bash
python manage.py migrate
```

Watch the progress via the Postgres playbook's monitoring queries
(`docs/migrations/asc-to-desc.md` §13) or the MSSQL playbook's
equivalent (`docs/migrations/asc-to-desc-mssql.md` §13).

---

## 4. FK cascade handling

The `foreign_keys` argument on `HeeRanjIdDirectionFlip` takes a list
of `(table_name, column_name)` tuples — child tables that need their
FK columns flipped alongside the parent's PK.

```python
HeeRanjIdDirectionFlip(
    model="app.Event",
    direction=HeeRanjIdDirectionFlip.DIRECTION_HEERID_TO_HEERID_DESC,
    foreign_keys=[
        ("app_eventattachment", "event_id"),
        ("app_eventtag", "event_id"),
    ],
)
```

For cycles, multi-level FKs, or self-references, follow the
playbook's hand-written pattern — the Operation handles single-level
cascades only. See `docs/migrations/asc-to-desc.md` §4–§8 for the
patterns that don't fit in a single Operation call.

---

## 5. Chunk size tuning

Default is 10 000 rows per batch — suitable for tables up to ~100 M
rows on modern NVMe. Larger tables may benefit from larger batches
(more throughput at the cost of longer-held locks); smaller
transactions reduce lock contention on highly concurrent writes.

```python
HeeRanjIdDirectionFlip(
    model="app.Event",
    direction=HeeRanjIdDirectionFlip.DIRECTION_HEERID_TO_HEERID_DESC,
    chunk_size=50_000,  # for low-contention high-throughput tables
)
```

---

## 6. Pre-flight checklist

Before running either migration:

- [ ] Node ID bound on the migrating service (check via
      `current_heer_node_id()` / `heer_current_node_id()`).
- [ ] Target table is not under merge replication (MSSQL) or
      logical replication subscriber (Postgres).
- [ ] No competing `zzz_*_autofill_desc` trigger on the table.
- [ ] Long-running transactions audited — any open for > 60 s will
      block or force-abort the cutover.
- [ ] For Standard edition MSSQL: scheduled maintenance window for
      the offline index build.
- [ ] Backup taken or restore-point verified.

---

## 7. Monitoring during migration

Use Django signals to instrument — `pre_migrate` / `post_migrate`
hooks expose the migration phase, and connection-level query logging
via `django.db.connection.queries` lets you observe the emitted SQL
during the actual run:

```python
from django.conf import settings
from django.db import connection

settings.DEBUG = True  # enables connection.queries capture
# run migrate
# inspect connection.queries for the full sequence
```

For production: use the playbook's monitoring queries directly
against Postgres (`pg_stat_activity`, `pg_locks`) or MSSQL
(`sys.dm_tran_locks`, `sys.dm_exec_procedure_stats`) to watch the
backfill progress and catch stuck batches early.

---

## 8. Cross-references

- Postgres playbook: `docs/migrations/asc-to-desc.md`
- MSSQL playbook: `docs/migrations/asc-to-desc-mssql.md`
- v0.3.1 spec: `docs/superpowers/specs/2026-04-23-v0.3.1-mssql-and-django-desc-design.md`
- Runnable MSSQL example: `bindings/python/django/examples/migrate_asc_to_desc_mssql.py`
