"""Install HeeRanjID schema and functions/procedures."""

from django.db import migrations


def _get_sql_module(schema_editor):
    """Return the SQL constants module for the current database backend."""
    vendor = schema_editor.connection.vendor
    if vendor == "microsoft":
        from heeranjid.sql import mssql

        return mssql
    from heeranjid.sql import postgres

    return postgres


def forwards(apps, schema_editor):
    sql = _get_sql_module(schema_editor)
    sql_parts = [
        sql.SCHEMA,
        sql.SESSION,
        sql.GENERATE_HEERID,
        sql.GENERATE_RANJID,
        sql.CONFIGURE,
        sql.SEED,
    ]

    backend = "mssql" if schema_editor.connection.vendor == "microsoft" else "postgres"
    for part in sql_parts:
        if backend == "mssql":
            batches = part.split("\nGO\n")
            for batch in batches:
                batch = batch.strip()
                if batch and batch != "GO":
                    schema_editor.execute(batch)
        else:
            schema_editor.execute(part)

    # After all SQL parts are executed, call heer_configure() to bake in epoch/precision
    if backend == "mssql":
        schema_editor.execute("EXEC heer_configure")
    else:
        schema_editor.execute("SELECT heer_configure()")


def backwards(apps, schema_editor):
    vendor = schema_editor.connection.vendor

    if vendor == "microsoft":
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
