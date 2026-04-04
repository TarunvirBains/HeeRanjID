.PHONY: dev build clean

SQL_SRC := ../sql
SQL_DEST := python/heeranjid/sql

dev: copy-sql
	maturin develop

build: copy-sql
	maturin build --release

copy-sql:
	# Postgres: top-level files
	cp $(SQL_SRC)/postgres/schema.sql $(SQL_DEST)/postgres/
	cp $(SQL_SRC)/postgres/seed.sql $(SQL_DEST)/postgres/
	cp $(SQL_SRC)/postgres/install.sql $(SQL_DEST)/postgres/
	# Postgres: functions
	cp $(SQL_SRC)/postgres/functions/session.sql $(SQL_DEST)/postgres/
	cp $(SQL_SRC)/postgres/functions/generate_heerid.sql $(SQL_DEST)/postgres/
	cp $(SQL_SRC)/postgres/functions/generate_ranjid.sql $(SQL_DEST)/postgres/
	# MSSQL: top-level files
	cp $(SQL_SRC)/mssql/schema.sql $(SQL_DEST)/mssql/
	cp $(SQL_SRC)/mssql/seed.sql $(SQL_DEST)/mssql/
	cp $(SQL_SRC)/mssql/install.sql $(SQL_DEST)/mssql/
	# MSSQL: procedures
	cp $(SQL_SRC)/mssql/procedures/session.sql $(SQL_DEST)/mssql/
	cp $(SQL_SRC)/mssql/procedures/generate_heerid.sql $(SQL_DEST)/mssql/
	cp $(SQL_SRC)/mssql/procedures/generate_ranjid.sql $(SQL_DEST)/mssql/

clean:
	rm -f $(SQL_DEST)/postgres/*.sql
	rm -f $(SQL_DEST)/mssql/*.sql
