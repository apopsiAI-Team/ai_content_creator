"""Bridge endpoint for starting the UI from the e-mentoring platform."""
import json
from typing import Any

from fastapi import APIRouter, Form, HTTPException, Query, status
from fastapi.responses import HTMLResponse

from ..auth import decode_access_token
from ..config import settings


router = APIRouter(prefix="/api/platform", tags=["platform-start"])

MAX_FORM_FIELD_CHARS = 2_000_000
BOOTSTRAP_STORAGE_KEY = "edu-material-platform-start"
AUTH_STORAGE_KEY = "edu-material-auth-token"


def _loads_json_field(name: str, value: str) -> Any:
    if len(value) > MAX_FORM_FIELD_CHARS:
        raise HTTPException(
            status_code=status.HTTP_413_REQUEST_ENTITY_TOO_LARGE,
            detail=f"{name} is too large",
        )
    try:
        return json.loads(value)
    except json.JSONDecodeError as exc:
        raise HTTPException(
            status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
            detail=f"{name} must be valid JSON",
        ) from exc


def _parse_module_index(value: str | None) -> int | None:
    if value is None or value.strip() == "":
        return None
    try:
        parsed = int(value)
    except ValueError as exc:
        raise HTTPException(
            status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
            detail="moduleIndex must be an integer",
        ) from exc
    if parsed < 0:
        raise HTTPException(
            status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
            detail="moduleIndex must be zero-based and non-negative",
        )
    return parsed


def _safe_script_json(data: Any) -> str:
    return json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003c")


@router.post("/start", response_class=HTMLResponse)
async def start_from_platform(
    token: str = Query(..., description="Platform-issued JWT access token"),
    proposal: str = Form(..., description="Educational proposal JSON"),
    moduleIndex: str | None = Form(None, description="Optional zero-based proposal.modules index"),
    clientContext: str = Form("{}", description="Opaque JSON returned with platform export"),
):
    """Accept platform form POST data and open the SPA with state pre-loaded."""
    decode_access_token(token)

    proposal_data = _loads_json_field("proposal", proposal)
    if not isinstance(proposal_data, dict):
        raise HTTPException(
            status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
            detail="proposal must be a JSON object",
        )

    client_context_data = _loads_json_field("clientContext", clientContext or "{}")
    selected_index = _parse_module_index(moduleIndex)
    modules = proposal_data.get("modules")
    if not isinstance(modules, list):
        modules = proposal_data.get("chapters")
    if selected_index is not None:
        if not isinstance(modules, list):
            raise HTTPException(
                status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
                detail="moduleIndex requires proposal.modules or proposal.chapters to be an array",
            )
        if selected_index >= len(modules):
            raise HTTPException(
                status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
                detail="moduleIndex is out of range",
            )

    bootstrap = {
        "proposal": proposal_data,
        "moduleIndex": selected_index,
        "clientContext": client_context_data,
    }

    public_url = str(settings.app_public_url or "/").rstrip("/") + "/"
    return HTMLResponse(
        content=f"""<!doctype html>
<html lang="el">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Open AI Content</title>
</head>
<body>
  <p>Opening application...</p>
  <script>
    sessionStorage.setItem({json.dumps(BOOTSTRAP_STORAGE_KEY)}, JSON.stringify({_safe_script_json(bootstrap)}));
    sessionStorage.setItem({json.dumps(AUTH_STORAGE_KEY)}, {_safe_script_json(token)});
    window.location.replace({_safe_script_json(public_url)});
  </script>
</body>
</html>""",
        status_code=200,
    )
