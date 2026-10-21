"""Failure audit records must never claim success; review-copy tests only."""
import pytest

from route_mini.decision import AuditDecision
from route_mini.fallback import ProviderError
from route_mini.policy import CheapestPolicy
from route_mini.providers import ScriptedProvider
from route_mini.router import FallbackExhaustedError, Router
from route_mini.telemetry import Telemetry
from route_mini.types import Model


def test_unfinished_audit_record_defaults_to_unsuccessful():
    record = AuditDecision('pending', Model('p', 'm'), (), 'pending')
    assert record.success is False
    assert record.to_dict()['success'] is False


@pytest.mark.parametrize('status', [400, 401])
def test_blocked_route_has_unsuccessful_audit_record(status, cheap, route_request):
    telemetry = Telemetry()
    provider = ScriptedProvider('cheap', (cheap,), [ProviderError(status_code=status)])
    router = Router((provider,), CheapestPolicy(), telemetry=telemetry)
    with pytest.raises(FallbackExhaustedError) as failure:
        router.route(route_request)
    assert failure.value.decision.reason == 'blocked-error'
    assert failure.value.decision.success is False
    assert telemetry.decisions()[0].to_dict()['success'] is False


def test_no_route_has_unsuccessful_audit_record(route_request):
    telemetry = Telemetry()
    router = Router((), CheapestPolicy(), telemetry=telemetry)
    with pytest.raises(FallbackExhaustedError) as failure:
        router.route(route_request)
    assert failure.value.decision.reason == 'no-route'
    assert failure.value.decision.success is False
    assert telemetry.decisions()[0].to_dict()['success'] is False
