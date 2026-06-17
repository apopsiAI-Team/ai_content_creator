"""
Claude API proxy router - matches frontend expectations

Despite the historical /api/claude prefix, this router now dispatches
to either provider via ``get_llm_service``. Renaming the prefix is
deferred so the existing frontend keeps working.
"""
import hashlib
from typing import Literal
from fastapi import APIRouter, Depends, HTTPException, Request
from fastapi.responses import StreamingResponse
from pydantic import BaseModel
import json
import traceback

from ..services.llm_service import get_llm_service
from ..rate_limiter import get_rate_limiter, Priority
from ..auth import require_jwt


router = APIRouter(prefix="/api/claude", tags=["claude"], dependencies=[Depends(require_jwt)])


def _extract_user_id(request: Request) -> str:
    """Extract user ID from X-Session-ID header, fallback to IP hash."""
    session_id = request.headers.get("X-Session-ID")
    if session_id:
        return session_id
    client_ip = request.client.host if request.client else "unknown"
    return hashlib.sha256(client_ip.encode()).hexdigest()[:16]


class Message(BaseModel):
    role: str  # 'user' or 'assistant'
    content: str


class GenerateRequest(BaseModel):
    system: str
    messages: list[Message]
    maxTokens: int = 16000
    model_provider: Literal["claude", "openai"] = "claude"


@router.post("/generate")
async def generate_content(request: GenerateRequest, raw_request: Request):
    """LLM generic proxy — generates content via the selected provider.
    Returns response in the format the frontend expects."""
    try:
        user_id = _extract_user_id(raw_request)
        llm = get_llm_service(request.model_provider)
        rate_limiter = get_rate_limiter()

        messages = [{"role": m.role, "content": m.content} for m in request.messages]

        async with rate_limiter.throttle(user_id, Priority.LIGHT, estimated_output=request.maxTokens):
            text, usage = await llm.complete_text(
                system=request.system,
                messages=messages,
                max_tokens=request.maxTokens,
            )
        await rate_limiter.tracker.record_usage(
            int(usage.get("input_tokens", 0)), int(usage.get("output_tokens", 0))
        )

        return {
            "content": [{"type": "text", "text": text}],
            "usage": {
                "input_tokens": int(usage.get("input_tokens", 0)),
                "output_tokens": int(usage.get("output_tokens", 0)),
            },
        }
    except HTTPException:
        raise
    except Exception as e:
        traceback.print_exc()
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/generate-stream")
async def generate_content_stream(request: GenerateRequest, raw_request: Request):
    """LLM streaming proxy. Returns SSE events with chunks."""
    try:
        user_id = _extract_user_id(raw_request)
        llm = get_llm_service(request.model_provider)
        rate_limiter = get_rate_limiter()

        messages = [{"role": m.role, "content": m.content} for m in request.messages]

        async def event_stream():
            usage_out: dict = {}
            async with rate_limiter.throttle(user_id, Priority.LIGHT, estimated_output=request.maxTokens):
                async for text in llm.stream_text(
                    system=request.system,
                    messages=messages,
                    max_tokens=request.maxTokens,
                    usage_out=usage_out,
                ):
                    yield f"data: {json.dumps({'text': text})}\n\n"
            await rate_limiter.tracker.record_usage(
                int(usage_out.get("input_tokens", 0)),
                int(usage_out.get("output_tokens", 0)),
            )
            yield f"data: {json.dumps({'done': True})}\n\n"

        return StreamingResponse(
            event_stream(),
            media_type="text/event-stream",
            headers={
                "Cache-Control": "no-cache",
                "Connection": "keep-alive",
                "X-Accel-Buffering": "no",
            },
        )
    except HTTPException:
        raise
    except Exception as e:
        traceback.print_exc()
        raise HTTPException(status_code=500, detail=str(e))
