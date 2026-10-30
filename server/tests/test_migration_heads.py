"""The Alembic chain has one head and no revision id used twice.

Two branches each added a revision "074" (vibe events and upstream's guest human
verification, #865). They merged without a text conflict, then
``alembic upgrade head`` failed with "Multiple head revisions". The vibe
migration is now 075. This test fails in the PR suite instead of at deploy.
"""

from collections import Counter
from pathlib import Path

from alembic.config import Config
from alembic.script import ScriptDirectory

_SERVER = Path(__file__).resolve().parent.parent


def _scripts() -> ScriptDirectory:
    config = Config(str(_SERVER / "alembic.ini"))
    config.set_main_option("script_location", str(_SERVER / "alembic"))
    return ScriptDirectory.from_config(config)


def test_single_head():
    heads = _scripts().get_heads()
    assert len(heads) == 1, f"Alembic has {len(heads)} heads: {heads}"


def test_revision_ids_unique():
    ids = []
    for path in (_SERVER / "alembic" / "versions").glob("*.py"):
        for line in path.read_text().splitlines():
            if line.startswith("revision = "):
                ids.append(line.split("=", 1)[1].strip().strip("\"'"))
    duplicates = [rev for rev, n in Counter(ids).items() if n > 1]
    assert not duplicates, f"revision ids used more than once: {duplicates}"
