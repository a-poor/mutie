import pytest

from route_mini.decision import AuditDecision
from route_mini.fallback import ProviderError
from route_mini.policy import CheapestPolicy
from route_mini.providers import ScriptedProvider
from route_mini.router import FallbackExhaustedError, Router
from route_mini.types import Model


def test_exception_keeps_specific_diagnostic():
    model = Model("provider", "model")
    decision = AuditDecision(
        "request", model, (model,), "fallback-exhausted",
        error="503: upstream maintenance",
    )
    error = FallbackExhaustedError(decision)
    assert str(error) == "503: upstream maintenance"
    assert error.decision is decision


@pytest.mark.parametrize("message", [None, ""])
def test_exception_uses_default_only_without_diagnostic(message):
    model = Model("provider", "model")
    decision = AuditDecision("request", model, (model,), "fallback-exhausted", error=message)
    assert str(FallbackExhaustedError(decision)) == "fallback exhausted"


def test_exhausted_route_preserves_last_provider_diagnostic(route_request, cheap):
    provider = ScriptedProvider(
        "cheap", (cheap,), [ProviderError(status_code=503, message="upstream maintenance")]
    )
    router = Router((provider,), CheapestPolicy())
    with pytest.raises(FallbackExhaustedError) as raised:
        router.route(route_request)
    assert len(provider.calls) == 1
    assert raised.value.decision.reason == "fallback-exhausted"
    assert raised.value.decision.attempts == 1
    assert raised.value.decision.error == "503: upstream maintenance"
    assert str(raised.value) == "503: upstream maintenance"
