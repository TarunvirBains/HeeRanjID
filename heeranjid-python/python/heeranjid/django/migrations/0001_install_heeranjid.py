"""Install HeeRanjID schema and functions/procedures."""
from importlib import resources

from django.db import migrations


def _get_backend(schema_editor):
    """Return 'postgres' or 'mssql' based on the database vendor."""
    vendor = schema_editor.connection.vendor
    if vendor == "microsoft":
        return "mssql"
    return "postgres"


def _read_sql(backend, filename):
    """Read a bundled SQL file from the heeranjid.sql.<backend> package."""
    package = f"heeranjid.sql.{backend}"
    return resources.files(package).joinpath(filename).read_text(encoding="utf-8")


def forwards(apps, schema_editor):
    backend = _get_backend(schema_editor)
    sql_files = [
        "schema.sql",
        "session.sql",
        "generate_heerid.sql",
        "generate_ranjid.sql",
        "seed.sql",
    ]
    for filename in sql_files:
        sql = _read_sql(backend, filename)
        if backend == "mssql":
            # MSSQL requires splitting on GO batch separators
            batches = sql.split("\nGO\n")
            for batch in batches:
                batch = batch.strip()
                if batch and batch != "GO":
                    schema_editor.execute(batch)
        else:
            schema_editor.execute(sql)


def backwards(apps, schema_editor):
    backend = _get_backend(schema_editor)

    if backend == "mssql":
        drops = [
            "DROP PROCEDURE IF EXISTS generate_id;",
            "DROP PROCEDURE IF EXISTS generate_ids;",
            "DROP PROCEDURE IF EXISTS generate_ranjid;",
            "DROP PROCEDURE IF EXISTS generate_ranjids;",
            "DROP PROCEDURE IF EXISTS heer_set_node_id;",
            "DROP PROCEDURE IF EXISTS heer_set_ranj_node_id;",
            "DROP FUNCTION IF EXISTS dbo.heer_current_node_id;",
            "DROP FUNCTION IF EXISTS dbo.heer_current_ranj_node_id;",
            "DROP TABLE IF EXISTS heer_ranj_node_state;",
            "DROP TABLE IF EXISTS heer_node_state;",
            "DROP TABLE IF EXISTS heer_config;",
            "DROP TABLE IF EXISTS heer_nodes;",
        ]
    else:
        drops = [
            "DROP FUNCTION IF EXISTS generate_id(INTEGER) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ids(INTEGER) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ids(INTEGER, INTEGER) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ids(INTEGER, INTEGER, BOOLEAN) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ids(INTEGER, BOOLEAN) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ranjid() CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ranjid(INTEGER) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ranjids(INTEGER) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ranjids(INTEGER, BOOLEAN) CASCADE;",
            "DROP FUNCTION IF EXISTS generate_ranjids(INTEGER, INTEGER, BOOLEAN) CASCADE;",
            "DROP FUNCTION IF EXISTS set_heer_node_id(INTEGER) CASCADE;",
            "DROP FUNCTION IF EXISTS current_heer_node_id() CASCADE;",
            "DROP FUNCTION IF EXISTS set_heer_ranj_node_id(INTEGER) CASCADE;",
            "DROP FUNCTION IF EXISTS current_heer_ranj_node_id() CASCADE;",
            "DROP TABLE IF EXISTS heer_ranj_node_state CASCADE;",
            "DROP TABLE IF EXISTS heer_node_state CASCADE;",
            "DROP TABLE IF EXISTS heer_config CASCADE;",
            "DROP TABLE IF EXISTS heer_nodes CASCADE;",
        ]

    for stmt in drops:
        schema_editor.execute(stmt)


class Migration(migrations.Migration):
    initial = True
    dependencies = []
    operations = [
        migrations.RunPython(forwards, backwards),
    ]
