"""
OpenAI Service - GPT implementation of LLMService.

Mirrors the Anthropic ClaudeService surface so the LLMService base
class can drive either provider through identical high-level methods.
Differences vs. Anthropic:
  - System prompt is a plain string (OpenAI auto-caches prefixes ≥1024 tokens).
  - Truncation: ``finish_reason == "length"`` (vs Anthropic's stop_reason).
  - Token usage fields: ``prompt_tokens`` / ``completion_tokens``
    (mapped to ``input_tokens`` / ``output_tokens`` for the rate limiter).
"""
from typing import AsyncIterator, Optional
from openai import AsyncOpenAI

from ..config import settings
from .llm_service import LLMService


class OpenAIService(LLMService):
    provider_name = "openai"

    def __init__(self) -> None:
        super().__init__()
        self.client = AsyncOpenAI(api_key=settings.openai_api_key)
        self.model = settings.openai_model_id

    @staticmethod
    def _build_messages(system: Optional[str], messages: list[dict]) -> list[dict]:
        """Prepend a system message if provided. OpenAI keeps everything
        in a single messages array (no separate system parameter)."""
        if system:
            return [{"role": "system", "content": system}, *messages]
        return list(messages)

    async def stream_text(
        self,
        system: str,
        messages: list[dict],
        max_tokens: int,
        usage_out: dict,
    ) -> AsyncIterator[str]:
        full_messages = self._build_messages(system, messages)

        truncated = False
        prompt_tokens = 0
        completion_tokens = 0

        stream = await self.client.chat.completions.create(
            model=self.model,
            messages=full_messages,
            max_completion_tokens=max_tokens,
            stream=True,
            stream_options={"include_usage": True},
        )

        async for chunk in stream:
            choices = getattr(chunk, "choices", None) or []
            if choices:
                choice = choices[0]
                delta = getattr(choice, "delta", None)
                if delta is not None:
                    content = getattr(delta, "content", None)
                    if content:
                        yield content
                finish_reason = getattr(choice, "finish_reason", None)
                if finish_reason == "length":
                    truncated = True
            usage = getattr(chunk, "usage", None)
            if usage is not None:
                prompt_tokens = getattr(usage, "prompt_tokens", prompt_tokens) or prompt_tokens
                completion_tokens = (
                    getattr(usage, "completion_tokens", completion_tokens) or completion_tokens
                )

        usage_out["input_tokens"] = prompt_tokens
        usage_out["output_tokens"] = completion_tokens
        usage_out["truncated"] = truncated

    async def complete_text(
        self,
        system: Optional[str],
        messages: list[dict],
        max_tokens: int,
    ) -> tuple[str, dict]:
        full_messages = self._build_messages(system, messages)
        response = await self.client.chat.completions.create(
            model=self.model,
            messages=full_messages,
            max_completion_tokens=max_tokens,
        )
        text = ""
        if response.choices:
            content = response.choices[0].message.content
            text = content or ""
        usage = response.usage
        input_tokens = getattr(usage, "prompt_tokens", 0) if usage else 0
        output_tokens = getattr(usage, "completion_tokens", 0) if usage else 0
        return text, {"input_tokens": input_tokens, "output_tokens": output_tokens}
