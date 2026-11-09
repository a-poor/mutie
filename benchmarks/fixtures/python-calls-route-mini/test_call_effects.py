import pytest

from route_mini.fallback import ProviderError
from route_mini.policy import BudgetCapPolicy, CheapestPolicy
from route_mini.providers import ScriptedProvider, scripted_success
from route_mini.router import FallbackExhaustedError, Router
from route_mini.telemetry import Telemetry


def test_no_route_preserves_exception_message(route_request, cheap):
    provider = ScriptedProvider("cheap", (cheap,), [scripted_success()])
    router = Router((provider,), BudgetCapPolicy(0))
    with pytest.raises(FallbackExhaustedError) as raised:
        router.route(route_request)
    assert str(raised.value) == "no eligible models"
    assert provider.calls == []


def test_no_route_is_recorded(route_request, cheap):
    telemetry = Telemetry()
    provider = ScriptedProvider("cheap", (cheap,), [scripted_success()])
    router = Router((provider,), BudgetCapPolicy(0), telemetry=telemetry)
    with pytest.raises(FallbackExhaustedError) as raised:
        router.route(route_request)
    assert telemetry.decisions() == (raised.value.decision,)
    assert raised.value.decision.reason == "no-route"
    assert provider.calls == []


def test_blocked_error_is_recorded(route_request, cheap):
    telemetry = Telemetry()
    provider = ScriptedProvider("cheap", (cheap,), [ProviderError(status_code=401)])
    router = Router((provider,), CheapestPolicy(), telemetry=telemetry)
    with pytest.raises(FallbackExhaustedError) as raised:
        router.route(route_request)
    assert telemetry.decisions() == (raised.value.decision,)
    assert raised.value.decision.reason == "blocked-error"
    assert len(provider.calls) == 1
