"""Postgres SQL constants — loaded from bundled .sql files at import time."""

from importlib import resources

_pkg = resources.files(__package__)

try:
    SCHEMA = _pkg.joinpath("schema.sql").read_text(encoding="utf-8")
    SEED = _pkg.joinpath("seed.sql").read_text(encoding="utf-8")
    INSTALL = _pkg.joinpath("install.sql").read_text(encoding="utf-8")
    SESSION = _pkg.joinpath("session.sql").read_text(encoding="utf-8")
    GENERATE_HEERID = _pkg.joinpath("generate_heerid.sql").read_text(encoding="utf-8")
    GENERATE_RANJID = _pkg.joinpath("generate_ranjid.sql").read_text(encoding="utf-8")
    CONFIGURE = _pkg.joinpath("configure.sql").read_text(encoding="utf-8")
except FileNotFoundError:
    raise FileNotFoundError(
        "SQL files not found in heeranjid.sql.postgres. "
        "Build with 'make dev' or 'make build' to copy SQL files from the sql/ submodule."
    )
