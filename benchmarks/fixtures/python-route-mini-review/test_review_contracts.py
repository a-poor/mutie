"""Separate-copy checks for immutable records and unknown-error retry policy."""
import pytest
from dataclasses import FrozenInstanceError
from route_mini.decision import AuditDecision
from route_mini.fallback import ErrorKind, FallbackChain, FallbackDecision, ProviderError
from route_mini.providers import ModelCapabilities, ScriptedSuccess
from route_mini.telemetry import ProviderRollup
from route_mini.types import Cost, Decision, Model, Provider, Request, Response


def test_unknown_error_never_retries():
    for attempt in (0, 1, 5):
        decision = FallbackChain(max_retries=5).decide(ProviderError(), attempt)
        assert decision.kind == ErrorKind.UNKNOWN
        assert decision.retry_same is False
        assert decision.fall_through is True
        assert decision.backoff_ms == 0


def test_audit_record_is_immutable():
    record = AuditDecision('id', Model('p', 'm'), (), 'pending')
    with pytest.raises(FrozenInstanceError):
        record.reason = 'changed'


def test_error_record_is_immutable():
    record = ProviderError()
    with pytest.raises(FrozenInstanceError):
        record.message = 'changed'


def test_fallback_record_is_immutable():
    record = FallbackDecision(ErrorKind.UNKNOWN, False, True, 0)
    with pytest.raises(FrozenInstanceError):
        record.retry_same = True


def test_capabilities_record_is_immutable():
    record = ModelCapabilities(Model('p', 'm'), 10, 0.1, 0.1, 10)
    with pytest.raises(FrozenInstanceError):
        record.max_tokens = 20


def test_script_record_is_immutable():
    record = ScriptedSuccess()
    with pytest.raises(FrozenInstanceError):
        record.text = 'changed'


def test_rollup_record_is_immutable():
    record = ProviderRollup('p', 1, 1, 1.0, 10, 10, 0.1)
    with pytest.raises(FrozenInstanceError):
        record.total = 2


def test_provider_record_is_immutable():
    record = Provider('p')
    with pytest.raises(FrozenInstanceError):
        record.name = 'changed'


def test_cost_record_is_immutable():
    record = Cost()
    with pytest.raises(FrozenInstanceError):
        record.output = 1.0


def test_model_record_is_immutable():
    record = Model('p', 'm')
    with pytest.raises(FrozenInstanceError):
        record.name = 'changed'


def test_request_record_is_immutable():
    record = Request('prompt', 10)
    with pytest.raises(FrozenInstanceError):
        record.prompt = 'changed'


def test_response_record_is_immutable():
    record = Response('ok', Model('p', 'm'), 1, 1, 10, Cost())
    with pytest.raises(FrozenInstanceError):
        record.text = 'changed'


def test_decision_record_is_immutable():
    record = Decision('id', Model('p', 'm'), (), 'pending')
    with pytest.raises(FrozenInstanceError):
        record.reason = 'changed'
