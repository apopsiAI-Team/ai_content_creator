"""Server-to-server export endpoints for generated material."""
from typing import Any

import httpx
from fastapi import APIRouter, Depends, HTTPException, status
from pydantic import BaseModel, Field

from ..auth import require_jwt
from ..config import settings


router = APIRouter(prefix="/api/export", tags=["export"], dependencies=[Depends(require_jwt)])


class PlatformExportRequest(BaseModel):
    data: dict[str, Any] = Field(..., description="Generated material JSON to send under the platform data wrapper.")


@router.post("/platform")
async def export_to_platform(request: PlatformExportRequest):
    """Forward generated material to the e-mentoring platform import endpoint."""
    if not settings.platform_export_enabled:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail="Platform export is not enabled",
        )
    if not settings.platform_export_url:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Platform export URL is not configured",
        )

    headers = {"Content-Type": "application/json"}
    auth: httpx.BasicAuth | None = None

    if settings.platform_export_auth_header:
        headers["Authorization"] = settings.platform_export_auth_header
    elif settings.platform_export_username and settings.platform_export_password:
        auth = httpx.BasicAuth(settings.platform_export_username, settings.platform_export_password)
    else:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Platform export credentials are not configured",
        )

    try:
        async with httpx.AsyncClient(timeout=settings.platform_export_timeout_seconds) as client:
            response = await client.post(
                settings.platform_export_url,
                headers=headers,
                auth=auth,
                json={"data": request.data},
            )
    except httpx.RequestError as exc:
        raise HTTPException(
            status_code=status.HTTP_502_BAD_GATEWAY,
            detail=f"Platform export request failed: {exc.__class__.__name__}",
        ) from exc

    if response.status_code < 200 or response.status_code >= 300:
        detail = response.text[:1000] if response.text else f"HTTP {response.status_code}"
        raise HTTPException(
            status_code=status.HTTP_502_BAD_GATEWAY,
            detail=f"Platform export failed with HTTP {response.status_code}: {detail}",
        )

    try:
        platform_response: Any = response.json()
    except ValueError:
        platform_response = response.text

    return {
        "status": "ok",
        "platform_status": response.status_code,
        "platform_response": platform_response,
    }
