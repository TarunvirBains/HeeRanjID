.PHONY: dev build clean copy-sql clean-wheels

SQL_SRC := ../../sql
SQL_DEST := python/heeranjid/sql
WHEEL_DIR := ../../target/wheels

dev: copy-sql
	maturin develop

build: clean-wheels copy-sql
	maturin build --release

clean-wheels:
	rm -rf $(WHEEL_DIR)
	mkdir -p $(WHEEL_DIR)

copy-sql:
	mkdir -p $(SQL_DEST)/postgres $(SQL_DEST)/mssql
	cp $(SQL_SRC)/postgres/schema.sql $(SQL_SRC)/postgres/seed.sql $(SQL_SRC)/postgres/install.sql $(SQL_DEST)/postgres/
	cp $(SQL_SRC)/postgres/functions/*.sql $(SQL_DEST)/postgres/
	cp $(SQL_SRC)/mssql/schema.sql $(SQL_SRC)/mssql/seed.sql $(SQL_SRC)/mssql/install.sql $(SQL_DEST)/mssql/
	cp $(SQL_SRC)/mssql/procedures/*.sql $(SQL_DEST)/mssql/

clean:
	rm -f $(SQL_DEST)/postgres/*.sql
	rm -f $(SQL_DEST)/mssql/*.sql
