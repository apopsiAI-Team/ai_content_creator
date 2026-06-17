"""
DOCX endpoints — accept a .docx upload and produce either:
- structured educational-design JSON (`/api/parse-docx`), or
- the full document content as markdown (`/api/docx-to-markdown`).

Both make the docx handling the frontend does (mammoth + …) available to
API consumers that don't run the React UI.
"""
import io
import traceback
import uuid

import mammoth
from fastapi import APIRouter, Depends, File, HTTPException, UploadFile
from markdownify import markdownify

from ..config import settings
from ..services.docx_parser import parse_docx_bytes, to_dict
from ..services.esco_service import get_esco_service
from ..auth import require_jwt


router = APIRouter(prefix="/api", tags=["docx"], dependencies=[Depends(require_jwt)])


MAX_FILE_BYTES = 10 * 1024 * 1024  # 10 MB cap
ALLOWED_EXTENSIONS = (".docx",)


def _validate_docx_upload(file: UploadFile, data: bytes) -> None:
    """Common guards for .docx uploads. Raises HTTPException on failure."""
    filename = file.filename or ""
    if not filename.lower().endswith(ALLOWED_EXTENSIONS):
        raise HTTPException(status_code=400, detail="Only .docx files are supported")
    if len(data) > MAX_FILE_BYTES:
        raise HTTPException(status_code=413, detail="File too large (max 10 MB)")
    if not data:
        raise HTTPException(status_code=400, detail="Empty file")


@router.post("/parse-docx")
async def parse_docx(file: UploadFile = File(...)):
    """Parse an uploaded educational-design .docx into structured JSON.

    Returns the parsed shape plus a fresh ``document_id`` (UUID with ``doc-``
    prefix). The consumer is expected to reuse this id in all subsequent
    /api/generate-stream, /api/review, /api/generate-summary and
    /api/generate-bibliography calls that concern this document — it surfaces
    in backend logs for traceability.

    Response shape:
        {
          "documentTitle": str,
          "totalHours": int,
          "modules": [{ number, title, hours, content, activities, skills }],
          "document_id": "doc-<uuid4>"
        }
    """
    data = await file.read()
    _validate_docx_upload(file, data)

    try:
        parsed = parse_docx_bytes(data)
    except Exception as e:
        traceback.print_exc()
        raise HTTPException(status_code=422, detail=f"Failed to parse .docx: {e}") from e

    result = to_dict(parsed)
    document_id = f"doc-{uuid.uuid4()}"
    result["document_id"] = document_id

    # Enrich every skill.code with its real ESCO URI when the name matches the
    # Greek ESCO dataset. Names not found keep the synthetic code produced by
    # the parser — so consumers can still treat .code as a stable identifier.
    esco = get_esco_service(settings.esco_data_path)
    enriched = 0
    for module in result.get("modules", []):
        for skill in module.get("skills", []):
            looked_up = esco.lookup_one(skill.get("name", ""))
            uri = looked_up.get("uri")
            if uri:
                skill["code"] = uri
                enriched += 1

    print(
        f"[api] endpoint=/api/parse-docx doc={document_id[:24]} "
        f"modules={len(result.get('modules', []))} "
        f"esco_uris_resolved={enriched} "
        f"title={(result.get('documentTitle') or '')[:40]}",
        flush=True,
    )

    return result


@router.post("/docx-to-markdown")
async def docx_to_markdown(file: UploadFile = File(...)):
    """Convert an uploaded .docx into a markdown string.

    Returns the document content in markdown format, ready to be passed as
    ``current_draft`` to ``POST /api/generate-stream`` with ``mode=revision``.

    This is **distinct** from ``/api/parse-docx``: this endpoint returns the
    full document content as a single markdown string (no structure
    extraction), whereas ``/api/parse-docx`` returns the educational-design
    schema (modules + ESCO skills).

    Pipeline: mammoth (.docx → HTML preserving headings/lists/bold/italic)
    → markdownify (HTML → markdown).

    Response shape:
        {
          "markdown": "# Heading\\n\\nParagraph...\\n",
          "document_id": "doc-<uuid4>"
        }
    """
    data = await file.read()
    _validate_docx_upload(file, data)

    try:
        html_result = mammoth.convert_to_html(io.BytesIO(data))
        html = html_result.value or ""
        # markdownify config matches what the frontend's HTML walker produces:
        # ATX headings (#), bullet style for lists, no wrapping (keep as-is).
        markdown = markdownify(
            html,
            heading_style="ATX",
            bullets="-",
            strip=["script", "style"],
        ).strip()
    except Exception as e:
        traceback.print_exc()
        raise HTTPException(status_code=422, detail=f"Failed to convert .docx to markdown: {e}") from e

    document_id = f"doc-{uuid.uuid4()}"
    warnings = [msg.message for msg in (html_result.messages or [])]

    print(
        f"[api] endpoint=/api/docx-to-markdown doc={document_id[:24]} "
        f"chars={len(markdown)} warnings={len(warnings)}",
        flush=True,
    )

    return {
        "markdown": markdown,
        "document_id": document_id,
    }
