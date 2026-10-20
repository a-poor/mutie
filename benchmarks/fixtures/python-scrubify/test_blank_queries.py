"""Stronger public-API checks discovered through the keep_blank_values survivor."""
from scrubify import redact_url


def test_redaction_preserves_blank_non_sensitive_parameters():
    assert redact_url("https://example.test/callback?next=&mode=login#finish") == (
        "https://example.test/callback?next=&mode=login#finish"
    )


def test_redaction_masks_blank_sensitive_parameters_without_dropping_them():
    assert redact_url("https://example.test/callback?token=&next=") == (
        "https://example.test/callback?token=%2A%2A%2A&next="
    )


def test_redaction_preserves_repeated_values_and_bare_flags():
    assert redact_url("https://example.test/?tag=one&tag=&flag") == (
        "https://example.test/?tag=one&tag=&flag="
    )
