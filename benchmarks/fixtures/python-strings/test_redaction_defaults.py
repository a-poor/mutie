"""Public redaction contracts missing from Scrubify's original six tests."""
import pytest

from scrubify import redact, redact_headers


@pytest.mark.parametrize('key', [
    'db_password', 'db_passwd', 'client_secret', 'token', 'token_primary',
    'access_token', 'client_api_key', 'client_apikey', 'auth', 'auth_primary',
    'client_auth', 'session', 'session_primary', 'client_session',
    'browser_cookie', 'db_credential', 'ssh_private_key', 'cloud_access_key',
    'oauth_refresh_key',
])
def test_default_sensitive_keys_are_redacted(key):
    payload = {key: 'private-value', 'label': 'public-value'}
    assert redact(payload) == {key: '***', 'label': 'public-value'}
    assert payload[key] == 'private-value'


@pytest.mark.parametrize('header', [
    'Authorization', 'Proxy-Authorization', 'X-Api-Key', 'X-Auth-Token',
    'Cookie', 'Set-Cookie',
])
def test_default_sensitive_headers_are_redacted(header):
    headers = {header: 'private-value', 'X-Request-Id': 'public-value'}
    assert redact_headers(headers) == {header: '***', 'X-Request-Id': 'public-value'}
    assert headers[header] == 'private-value'


def test_hyphenated_key_uses_the_same_redaction_rule():
    assert redact({'SSH-PRIVATE-KEY': 'private-value'}) == {'SSH-PRIVATE-KEY': '***'}
