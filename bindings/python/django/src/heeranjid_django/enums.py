from enum import Enum


class HeeRanjIdFieldType(Enum):
    HEERID = "heerid"
    RANJID = "ranjid"


class HeeRanjIdPrefetch(Enum):
    SAVE = "save"
    INIT = "init"
    MANUAL = None
