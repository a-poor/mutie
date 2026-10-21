"""Exhausted network retry budgets must not make another provider call."""
from route_mini.fallback import ErrorKind, FallbackChain, ProviderError
from route_mini.policy import CheapestPolicy
from route_mini.providers import ScriptedProvider, scripted_success
from route_mini.router import Router


def test_network_stops_retrying_at_exact_budget():
    decision = FallbackChain(max_retries=1).decide(ProviderError(network=True), attempt=1)
    assert decision.kind == ErrorKind.NETWORK
    assert decision.retry_same is False
    assert decision.fall_through is True
    assert decision.backoff_ms == 25


def test_zero_network_retry_budget_calls_primary_once(route_request, cheap, fast):
    first = ScriptedProvider('cheap', (cheap,), [ProviderError(network=True)])
    second = ScriptedProvider('fast', (fast,), [scripted_success('secondary')])
    decision, response = Router(
        (first, second), CheapestPolicy(), fallback=FallbackChain(max_retries=0)
    ).route(route_request)
    assert response.text == 'secondary'
    assert len(first.calls) == 1
    assert len(second.calls) == 1
    assert decision.attempts == 2
