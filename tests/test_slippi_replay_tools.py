import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TOOL = ROOT / "tools" / "slippi_replay_to_inputs.cjs"


def run_node_tool(*args):
    return subprocess.run(
        ["node", str(TOOL), *args],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


def test_slippi_replay_tool_self_test_covers_native_stick_conversion():
    result = run_node_tool("--self-test")

    assert result.returncode == 0, result.stderr
    assert "slippi_replay_to_inputs self-test passed" in result.stdout


def test_slippi_replay_tool_documents_local_dependency_install():
    text = TOOL.read_text(encoding="utf-8")

    assert "npm install --prefix tools/slippi" in text
    assert "@slippi/slippi-js/node" in text
    assert "--include-negative-frames" in text
    assert "--stdout" in text
    assert "EntryStart" in text


def test_slippi_replays_and_generated_diagnostics_stay_local():
    gitignore = (ROOT / ".gitignore").read_text(encoding="utf-8")

    assert "replays/*.slp" in gitignore
    assert "debug/slippi/" in gitignore
