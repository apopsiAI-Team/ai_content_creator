"""
Health check router
"""
from fastapi import APIRouter

from ..config import settings

router = APIRouter(prefix="/api", tags=["health"])


@router.get("/health")
async def health_check():
    """Check backend health status"""
    return {
        "status": "ok",
        "has_api_key": bool(settings.anthropic_api_key),
        "model": settings.model_id,
        "research_hub_available": settings.research_hub_path.exists(),
        "esco_data_available": settings.esco_data_path.exists(),
    }
