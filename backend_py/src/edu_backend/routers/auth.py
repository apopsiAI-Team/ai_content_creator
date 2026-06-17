"""Authentication endpoints for e-mentoring platform handoff."""
import hmac
from urllib.parse import urlencode

from fastapi import APIRouter, Header, HTTPException, status
from pydantic import BaseModel

from ..auth import ALLOWED_ROLES, create_access_token, isoformat_utc
from ..config import settings


router = APIRouter(prefix="/api/auth", tags=["auth"])


class MintTokenRequest(BaseModel):
    sub: str
    email: str
    role: str


@router.post("/mint")
async def mint_token(
    request: MintTokenRequest,
    x_platform_secret: str = Header(default="", alias="X-Platform-Secret"),
):
    """Mint a short-lived app link for a user authenticated by the platform."""
    if not settings.jwt_platform_secret:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Platform secret is not configured",
        )
    if not hmac.compare_digest(x_platform_secret, settings.jwt_platform_secret):
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Invalid platform secret")
    if request.role not in ALLOWED_ROLES:
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail="Invalid role")
    if "@" not in request.email:
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail="Invalid email")

    token, expires_at = create_access_token(sub=request.sub, email=request.email, role=request.role)
    base_url = settings.app_public_url.rstrip("/") + "/"
    return {
        "url": f"{base_url}?{urlencode({'token': token})}",
        "expires_at": isoformat_utc(expires_at),
    }
