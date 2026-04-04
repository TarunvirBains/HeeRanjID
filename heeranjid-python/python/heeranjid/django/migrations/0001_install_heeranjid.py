"""Install HeeRanjID schema and functions in Postgres."""
from importlib import resources

from django.db import migrations


def _read_sql(filename: str) -> str:
    """Read a bundled SQL file from the heeranjid.sql package."""
    return resources.files("heeranjid.sql").joinpath(filename).read_text(encoding="utf-8")


def forwards(apps, schema_editor):
    sql_files = [
        "schema.sql",
        "session.sql",
        "generate_heerid.sql",
        "generate_ranjid.sql",
        "seed.sql",
    ]
    for filename in sql_files:
        schema_editor.execute(_read_sql(filename))


def backwards(apps, schema_editor):
    schema_editor.execute("DROP FUNCTION IF EXISTS generate_id(INTEGER) CASCADE;")
    schema_editor.execute("DROP FUNCTION IF EXISTS generate_ids(INTEGER) CASCADE;")
    schema_editor.execute("DROP FUNCTION IF EXISTS generate_ids(INTEGER, INTEGER) CASCADE;")
    schema_editor.execute("DROP FUNCTION IF EXISTS generate_ids(INTEGER, INTEGER, BOOLEAN) CASCADE;")
    schema_editor.execute("DROP FUNCTION IF EXISTS generate_ids(INTEGER, BOOLEAN) CASCADE;")
    schema_editor.execute("DROP FUNCTION IF EXISTS generate_ranjid() CASCADE;")
    schema_editor.execute("DROP FUNCTION IF EXISTS generate_ranjid(INTEGER) CASCADE;")
    schema_editor.execute("DROP FUNCTION IF EXISTS generate_ranjids(INTEGER) CASCADE;")
    schema_editor.execute("DROP FUNCTION IF EXISTS generate_ranjids(INTEGER, BOOLEAN) CASCADE;")
    schema_editor.execute("DROP FUNCTION IF EXISTS generate_ranjids(INTEGER, INTEGER, BOOLEAN) CASCADE;")
    schema_editor.execute("DROP FUNCTION IF EXISTS set_heer_node_id(INTEGER) CASCADE;")
    schema_editor.execute("DROP FUNCTION IF EXISTS current_heer_node_id() CASCADE;")
    schema_editor.execute("DROP FUNCTION IF EXISTS set_heer_ranj_node_id(INTEGER) CASCADE;")
    schema_editor.execute("DROP FUNCTION IF EXISTS current_heer_ranj_node_id() CASCADE;")
    schema_editor.execute("DROP TABLE IF EXISTS heer_ranj_node_state CASCADE;")
    schema_editor.execute("DROP TABLE IF EXISTS heer_node_state CASCADE;")
    schema_editor.execute("DROP TABLE IF EXISTS heer_config CASCADE;")
    schema_editor.execute("DROP TABLE IF EXISTS heer_nodes CASCADE;")


class Migration(migrations.Migration):
    initial = True
    dependencies = []
    operations = [
        migrations.RunPython(forwards, backwards),
    ]
