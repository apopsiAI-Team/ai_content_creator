from pydantic_settings import BaseSettings, SettingsConfigDict
from pathlib import Path


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_file=".env", env_file_encoding="utf-8")

    # API Keys
    anthropic_api_key: str = ""
    openai_api_key: str = ""

    # Paths
    project_root: Path = Path(__file__).parent.parent.parent.parent
    research_hub_path: Path = project_root / "research_hub_mcp"
    esco_data_path: Path = Path(__file__).parent / "data" / "skills_compact.json"

    # Model settings
    # `model_id` is the Anthropic (Claude) model — kept as the original setting name
    # for backwards compatibility with existing code paths.
    model_id: str = "claude-opus-4-6"
    openai_model_id: str = "gpt-5.4"
    max_tokens: int = 64000  # Max output tokens per API call (~30K typical, 64K cap)

    # Rate limiting — Anthropic API tier (2, 3, or 4)
    anthropic_tier: int = 2

    # Server settings
    host: str = "0.0.0.0"
    port: int = 8000

    # Rust Research Hub settings
    rust_research_hub_url: str = "http://localhost:8091"

    # JWT access control. Keep disabled until the e-mentoring integration is ready.
    jwt_auth_enabled: bool = False
    jwt_signing_secret: str = ""
    jwt_platform_secret: str = ""
    jwt_issuer: str = "apopsi-ai"
    jwt_expire_hours: int = 8
    app_public_url: str = "https://apopsi-ai.apopsi.gr/ai-content/"


settings = Settings()
