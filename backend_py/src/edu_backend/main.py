"""
Educational Material Creator - Python Backend
FastAPI application with Research Hub integration
"""
from fastapi import Depends, FastAPI
from fastapi.middleware.cors import CORSMiddleware
from dotenv import load_dotenv
from pathlib import Path

# Load .env from project root
env_path = Path(__file__).parent.parent.parent.parent / ".env"
load_dotenv(env_path)

from .routers import health, generate, claude, docx, auth, export
from .config import settings
from .auth import require_jwt

app = FastAPI(
    title="Educational Material Creator API",
    description="Backend for generating high-quality educational content with Research Hub integration",
    version="2.0.0"
)


# CORS allowlist — add production origins as needed
ALLOWED_ORIGINS = [
    "http://localhost:5173",
    "http://localhost:4173",
    "http://127.0.0.1:5173",
    "http://127.0.0.1:4173",
]

app.add_middleware(
    CORSMiddleware,
    allow_origins=ALLOWED_ORIGINS,
    allow_credentials=True,
    allow_methods=["GET", "POST", "PUT", "DELETE", "OPTIONS"],
    allow_headers=["Content-Type", "Authorization", "X-Requested-With", "X-Session-ID", "X-Platform-Secret"],
    max_age=600,
)

# Include routers
app.include_router(health.router)
app.include_router(auth.router)
app.include_router(generate.router)
app.include_router(claude.router)
app.include_router(docx.router)
app.include_router(export.router)


@app.get("/api/rate-limit/status", dependencies=[Depends(require_jwt)])
async def rate_limit_status():
    """Return current rate limiter state for monitoring."""
    from .rate_limiter import get_rate_limiter
    rl = get_rate_limiter()
    return await rl.get_status()


@app.on_event("startup")
async def startup():
    from .services.research_service import check_rust_service_health
    from .rate_limiter import init_rate_limiter

    # Initialize rate limiter
    rl = init_rate_limiter()

    print("=" * 50)
    print("Educational Material Creator - Python Backend")
    print("=" * 50)
    print(f"API Key: {'✓' if settings.anthropic_api_key else '✗'}")
    print(f"Model: {settings.model_id}")
    print(f"Max Tokens: {settings.max_tokens}")
    print(f"Rate Limit Tier: {rl.tier} (heavy={rl.config.max_concurrent_heavy}, light={rl.config.max_concurrent_light})")
    print(f"Research Hub Path: {settings.research_hub_path}")
    print(f"ESCO Data: {settings.esco_data_path}")
    print("-" * 50)

    # Check Rust Research Hub service
    rust_url = getattr(settings, 'rust_research_hub_url', 'http://localhost:8091')
    rust_available = await check_rust_service_health(rust_url)

    if not rust_available:
        print("")
        print("💡 To enable multi-source paper search, start the Rust Research Hub:")
        print(f"   cd {settings.research_hub_path}")
        print("   cargo run --release -- http --port 8091")
        print("")
        print("   Fallback: Using CrossRef API directly")

    print("=" * 50)


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        "edu_backend.main:app",
        host=settings.host,
        port=settings.port,
        reload=True
    )
