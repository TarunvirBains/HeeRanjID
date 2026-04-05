import uuid as uuid_mod

from django.db import connection, migrations


class HeeRanjIdConversion(migrations.Operation):
    """
    Converts a model's primary key between HeerId (BIGINT) and RanjId (UUID),
    including all foreign key columns that reference it.
    """

    reduces_to_sql = True
    reversible = True

    DIRECTION_HEERID_TO_RANJID = "heerid_to_ranjid"
    DIRECTION_RANJID_TO_HEERID = "ranjid_to_heerid"
    VALID_DIRECTIONS = (DIRECTION_HEERID_TO_RANJID, DIRECTION_RANJID_TO_HEERID)

    def __init__(self, model, direction, foreign_keys=None, chunk_size=10000):
        """
        model: "app_label.ModelName"
        direction: "heerid_to_ranjid" or "ranjid_to_heerid"
        foreign_keys: [("table_name", "column_name"), ...]
        chunk_size: rows per batch for conversion
        """
        if direction not in self.VALID_DIRECTIONS:
            raise ValueError(
                f"Invalid direction {direction!r}. "
                f"Must be one of: {', '.join(self.VALID_DIRECTIONS)}"
            )
        self.model = model
        self.direction = direction
        self.foreign_keys = foreign_keys or []
        self.chunk_size = chunk_size

    def state_forwards(self, app_label, state):
        pass  # State changes handled by AlterField that accompanies this

    def database_forwards(self, app_label, schema_editor, from_state, to_state):
        if self.direction == self.DIRECTION_HEERID_TO_RANJID:
            self._convert_heerid_to_ranjid(schema_editor)
        else:
            self._convert_ranjid_to_heerid(schema_editor)

    def database_backwards(self, app_label, schema_editor, from_state, to_state):
        # Reverse direction
        if self.direction == self.DIRECTION_HEERID_TO_RANJID:
            self._convert_ranjid_to_heerid(schema_editor)
        else:
            self._convert_heerid_to_ranjid(schema_editor)

    def _get_table_name(self):
        """Extract table name from model string."""
        app_label, model_name = self.model.split(".")
        return f"{app_label}_{model_name.lower()}"

    def _is_mssql(self):
        return connection.vendor == "microsoft"

    def _convert_heerid_to_ranjid(self, schema_editor):
        """Convert BIGINT PK to UUID PK with all FK references."""
        from heeranjid import HeerId

        table = self._get_table_name()
        cursor = connection.cursor()
        is_mssql = self._is_mssql()

        # 1. Add new UUID column
        if is_mssql:
            cursor.execute(f"ALTER TABLE {table} ADD id_new BINARY(16)")
        else:
            cursor.execute(f"ALTER TABLE {table} ADD COLUMN id_new UUID")

        # 2. Convert in chunks using batch_to_ranjids
        offset = 0
        while True:
            if is_mssql:
                cursor.execute(
                    f"SELECT TOP {self.chunk_size} id FROM {table} "
                    f"ORDER BY id OFFSET {offset} ROWS"
                )
            else:
                cursor.execute(
                    f"SELECT id FROM {table} ORDER BY id "
                    f"LIMIT {self.chunk_size} OFFSET {offset}"
                )
            rows = cursor.fetchall()
            if not rows:
                break

            heer_ids = [HeerId(int(r[0])) for r in rows]
            pairs = HeerId.batch_to_ranjids(heer_ids)

            for hid, rid in pairs:
                if is_mssql:
                    cursor.execute(
                        f"UPDATE {table} SET id_new = %s WHERE id = %s",
                        [rid.to_uuid().bytes, hid.as_int()],
                    )
                else:
                    cursor.execute(
                        f"UPDATE {table} SET id_new = %s WHERE id = %s",
                        [str(rid), hid.as_int()],
                    )

            offset += self.chunk_size

        # 3. Convert FK columns
        for fk_table, fk_column in self.foreign_keys:
            if is_mssql:
                cursor.execute(
                    f"ALTER TABLE {fk_table} ADD {fk_column}_new BINARY(16)"
                )
            else:
                cursor.execute(
                    f"ALTER TABLE {fk_table} ADD COLUMN {fk_column}_new UUID"
                )

            cursor.execute(
                f"UPDATE {fk_table} SET {fk_column}_new = "
                f"(SELECT id_new FROM {table} "
                f"WHERE {table}.id = {fk_table}.{fk_column})"
            )

        # 4. Drop old FK constraints, swap columns
        for fk_table, fk_column in self.foreign_keys:
            self._drop_fk_constraints(cursor, fk_table, fk_column, is_mssql)
            cursor.execute(f"ALTER TABLE {fk_table} DROP COLUMN {fk_column}")
            if is_mssql:
                cursor.execute(
                    f"EXEC sp_rename '{fk_table}.{fk_column}_new', "
                    f"'{fk_column}', 'COLUMN'"
                )
            else:
                cursor.execute(
                    f"ALTER TABLE {fk_table} RENAME COLUMN "
                    f"{fk_column}_new TO {fk_column}"
                )

        # 5. Drop old PK, rename new column, recreate PK
        self._drop_pk_constraint(cursor, table, is_mssql)
        cursor.execute(f"ALTER TABLE {table} DROP COLUMN id")
        if is_mssql:
            cursor.execute(
                f"EXEC sp_rename '{table}.id_new', 'id', 'COLUMN'"
            )
        else:
            cursor.execute(
                f"ALTER TABLE {table} RENAME COLUMN id_new TO id"
            )
        cursor.execute(f"ALTER TABLE {table} ADD PRIMARY KEY (id)")

        # 6. Recreate FK constraints
        for fk_table, fk_column in self.foreign_keys:
            cursor.execute(
                f"ALTER TABLE {fk_table} ADD CONSTRAINT "
                f"fk_{fk_table}_{fk_column} "
                f"FOREIGN KEY ({fk_column}) REFERENCES {table}(id)"
            )

    def _convert_ranjid_to_heerid(self, schema_editor):
        """Convert UUID PK to BIGINT PK with all FK references."""
        from heeranjid import RanjId

        table = self._get_table_name()
        cursor = connection.cursor()
        is_mssql = self._is_mssql()

        # 1. Pre-flight check: fetch all RanjIds and verify convertibility
        cursor.execute(f"SELECT id FROM {table}")
        all_rows = cursor.fetchall()

        if is_mssql:
            ranj_ids = [
                RanjId.from_str(str(uuid_mod.UUID(bytes=bytes(r[0]))))
                for r in all_rows
            ]
        else:
            ranj_ids = [RanjId.from_str(str(r[0])) for r in all_rows]

        conflicts = RanjId.check_heerid_convertibility(ranj_ids)
        if conflicts:
            raise ValueError(
                f"Cannot convert {table} to HeerId: {len(conflicts)} "
                f"conflicts found. First conflict: {conflicts[0]}"
            )

        # 2. Add new BIGINT column
        cursor.execute(f"ALTER TABLE {table} ADD COLUMN id_new BIGINT")

        # 3. Convert using batch_to_heerids
        pairs = RanjId.batch_to_heerids(ranj_ids)
        for rid, hid in pairs:
            if is_mssql:
                cursor.execute(
                    f"UPDATE {table} SET id_new = %s WHERE id = %s",
                    [hid.as_int(), rid.to_uuid().bytes],
                )
            else:
                cursor.execute(
                    f"UPDATE {table} SET id_new = %s WHERE id = %s",
                    [hid.as_int(), str(rid)],
                )

        # 4. Convert FK columns
        for fk_table, fk_column in self.foreign_keys:
            cursor.execute(
                f"ALTER TABLE {fk_table} ADD COLUMN {fk_column}_new BIGINT"
            )

            cursor.execute(
                f"UPDATE {fk_table} SET {fk_column}_new = "
                f"(SELECT id_new FROM {table} "
                f"WHERE {table}.id = {fk_table}.{fk_column})"
            )

        # 5. Drop old FK constraints, swap columns
        for fk_table, fk_column in self.foreign_keys:
            self._drop_fk_constraints(cursor, fk_table, fk_column, is_mssql)
            cursor.execute(f"ALTER TABLE {fk_table} DROP COLUMN {fk_column}")
            if is_mssql:
                cursor.execute(
                    f"EXEC sp_rename '{fk_table}.{fk_column}_new', "
                    f"'{fk_column}', 'COLUMN'"
                )
            else:
                cursor.execute(
                    f"ALTER TABLE {fk_table} RENAME COLUMN "
                    f"{fk_column}_new TO {fk_column}"
                )

        # 6. Drop old PK, rename new column, recreate PK
        self._drop_pk_constraint(cursor, table, is_mssql)
        cursor.execute(f"ALTER TABLE {table} DROP COLUMN id")
        if is_mssql:
            cursor.execute(
                f"EXEC sp_rename '{table}.id_new', 'id', 'COLUMN'"
            )
        else:
            cursor.execute(
                f"ALTER TABLE {table} RENAME COLUMN id_new TO id"
            )
        cursor.execute(f"ALTER TABLE {table} ADD PRIMARY KEY (id)")

        # 7. Recreate FK constraints
        for fk_table, fk_column in self.foreign_keys:
            cursor.execute(
                f"ALTER TABLE {fk_table} ADD CONSTRAINT "
                f"fk_{fk_table}_{fk_column} "
                f"FOREIGN KEY ({fk_column}) REFERENCES {table}(id)"
            )

    def _drop_fk_constraints(self, cursor, fk_table, fk_column, is_mssql):
        """Find and drop foreign key constraints on a given column."""
        if is_mssql:
            cursor.execute(
                f"""
                SELECT fk.name
                FROM sys.foreign_keys fk
                JOIN sys.foreign_key_columns fkc
                    ON fk.object_id = fkc.constraint_object_id
                WHERE fk.parent_object_id = OBJECT_ID('{fk_table}')
                AND COL_NAME(fkc.parent_object_id, fkc.parent_column_id) = '{fk_column}'
                """
            )
        else:
            cursor.execute(
                f"""
                SELECT tc.constraint_name
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage kcu
                    ON tc.constraint_name = kcu.constraint_name
                WHERE tc.table_name = '{fk_table}'
                AND tc.constraint_type = 'FOREIGN KEY'
                AND kcu.column_name = '{fk_column}'
                """
            )
        for (constraint_name,) in cursor.fetchall():
            cursor.execute(
                f"ALTER TABLE {fk_table} DROP CONSTRAINT {constraint_name}"
            )

    def _drop_pk_constraint(self, cursor, table, is_mssql):
        """Find and drop the primary key constraint on a table."""
        if is_mssql:
            cursor.execute(
                f"""
                SELECT name FROM sys.key_constraints
                WHERE parent_object_id = OBJECT_ID('{table}')
                AND type = 'PK'
                """
            )
            for (constraint_name,) in cursor.fetchall():
                cursor.execute(
                    f"ALTER TABLE {table} DROP CONSTRAINT {constraint_name}"
                )
        else:
            # PostgreSQL uses <table>_pkey by convention, but query to be safe
            cursor.execute(
                f"""
                SELECT tc.constraint_name
                FROM information_schema.table_constraints tc
                WHERE tc.table_name = '{table}'
                AND tc.constraint_type = 'PRIMARY KEY'
                """
            )
            for (constraint_name,) in cursor.fetchall():
                cursor.execute(
                    f"ALTER TABLE {table} DROP CONSTRAINT {constraint_name}"
                )

    def describe(self):
        return f"Convert {self.model} PK: {self.direction}"

    def deconstruct(self):
        kwargs = {
            "model": self.model,
            "direction": self.direction,
        }
        if self.foreign_keys:
            kwargs["foreign_keys"] = self.foreign_keys
        if self.chunk_size != 10000:
            kwargs["chunk_size"] = self.chunk_size
        return (
            self.__class__.__qualname__,
            [],
            kwargs,
        )
