.PHONY: dev build clean copy-sql

SQL_SRC := ../../sql
SQL_DEST := python/heeranjid/sql

dev: copy-sql
	maturin develop

build: copy-sql
	maturin build --release

copy-sql:
	mkdir -p $(SQL_DEST)/postgres $(SQL_DEST)/mssql
	cp $(SQL_SRC)/postgres/schema.sql $(SQL_SRC)/postgres/seed.sql $(SQL_SRC)/postgres/install.sql $(SQL_DEST)/postgres/
	cp $(SQL_SRC)/postgres/functions/*.sql $(SQL_DEST)/postgres/
	cp $(SQL_SRC)/mssql/schema.sql $(SQL_SRC)/mssql/seed.sql $(SQL_SRC)/mssql/install.sql $(SQL_DEST)/mssql/
	cp $(SQL_SRC)/mssql/procedures/*.sql $(SQL_DEST)/mssql/

clean:
	rm -f $(SQL_DEST)/postgres/*.sql
	rm -f $(SQL_DEST)/mssql/*.sql
