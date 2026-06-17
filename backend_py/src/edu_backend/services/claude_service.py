"""
Claude Service - Anthropic implementation of LLMService.

All educational-content business logic lives in the LLMService base class
(`llm_service.py`). This file only implements the raw streaming and
completion calls plus Anthropic-specific prompt-cache wrapping.
"""
from typing import AsyncIterator, Optional
import anthropic

from ..config import settings
from .llm_service import LLMService, get_llm_service


def _cacheable_system(text: str) -> list[dict]:
    """Wrap a system prompt string for Anthropic prompt caching.

    Returns the list[dict] format required by the ``system`` parameter
    with ``cache_control`` set so the prompt is cached for ~5 minutes.
    Cached tokens are 90% cheaper and reduce time-to-first-token.
    """
    return [{"type": "text", "text": text, "cache_control": {"type": "ephemeral"}}]


class ClaudeService(LLMService):
    provider_name = "anthropic"

    def __init__(self) -> None:
        super().__init__()
        self.client = anthropic.AsyncAnthropic(api_key=settings.anthropic_api_key)
        self.model = settings.model_id

    async def stream_text(
        self,
        system: str,
        messages: list[dict],
        max_tokens: int,
        usage_out: dict,
    ) -> AsyncIterator[str]:
        async with self.client.messages.stream(
            model=self.model,
            max_tokens=max_tokens,
            system=_cacheable_system(system),
            messages=messages,
        ) as stream:
            async for text in stream.text_stream:
                yield text
            final_message = await stream.get_final_message()
            usage_out["input_tokens"] = final_message.usage.input_tokens
            usage_out["output_tokens"] = final_message.usage.output_tokens
            usage_out["truncated"] = final_message.stop_reason == "max_tokens"

    async def complete_text(
        self,
        system: Optional[str],
        messages: list[dict],
        max_tokens: int,
    ) -> tuple[str, dict]:
        kwargs: dict = {
            "model": self.model,
            "max_tokens": max_tokens,
            "messages": messages,
        }
        if system:
            kwargs["system"] = _cacheable_system(system)
        response = await self.client.messages.create(**kwargs)
        text = response.content[0].text if response.content else ""
        return text, {
            "input_tokens": response.usage.input_tokens,
            "output_tokens": response.usage.output_tokens,
        }


# Backwards-compatible singleton accessor (existing code calls
# ``get_claude_service()``; new code uses ``get_llm_service(provider)``).
def get_claude_service() -> ClaudeService:
    service = get_llm_service("claude")
    assert isinstance(service, ClaudeService)
    return service
