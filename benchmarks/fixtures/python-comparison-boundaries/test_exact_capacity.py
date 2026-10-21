"""A model that exactly meets the requested capacity remains eligible."""
import pytest
from route_mini.policy import CheapestPolicy
from route_mini.providers import ScriptedProvider, scripted_success
from route_mini.router import FallbackExhaustedError, Router
from route_mini.types import Request


def test_model_exactly_meets_requested_capacity(cheap):
    request_value = Request('exact capacity', max_tokens=cheap.max_tokens, input_tokens=2)
    provider = ScriptedProvider('cheap', (cheap,), [scripted_success('accepted')])
    router = Router((provider,), CheapestPolicy())
    try:
        decision, response = router.route(request_value)
    except FallbackExhaustedError as error:
        pytest.fail(f'exact-capacity model must be eligible: {error}')
    assert decision.success is True
    assert decision.selected == cheap.model
    assert response.text == 'accepted'
    assert len(provider.calls) == 1
