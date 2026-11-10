import click
import shutil
import subprocess
from types import SimpleNamespace
from click.shell_completion import BashComplete, split_arg_string


def test_source_output_preserves_exact_template_bytes(runner, monkeypatch):
    monkeypatch.setattr(BashComplete, "_check_version", staticmethod(lambda: None))
    cli = click.Command("cli")
    expected = BashComplete(cli, {}, "cli", "_CLI_COMPLETE").source().encode()
    result = runner.invoke(cli, env={"_CLI_COMPLETE": "bash_source"})
    assert result.exit_code == 0
    assert result.stdout_bytes == expected


def test_old_bash_warning_uses_stderr(monkeypatch, capsys):
    monkeypatch.setattr(shutil, "which", lambda name: "bash")
    monkeypatch.setattr(subprocess, "run", lambda *args, **kwargs: SimpleNamespace(stdout=b"3.2.57"))
    assert BashComplete(click.Command("cli"), {}, "cli", "_CLI_COMPLETE").source()
    captured = capsys.readouterr()
    assert captured.out == ""
    assert "older than 4.4" in captured.err


def test_missing_bash_warning_uses_stderr(monkeypatch, capsys):
    monkeypatch.setattr(shutil, "which", lambda name: None)
    assert BashComplete(click.Command("cli"), {}, "cli", "_CLI_COMPLETE").source()
    captured = capsys.readouterr()
    assert captured.out == ""
    assert "Couldn't detect Bash version" in captured.err


def test_completion_words_preserve_option_punctuation():
    assert split_arg_string("tool --value=a:b") == ["tool", "--value=a:b"]
