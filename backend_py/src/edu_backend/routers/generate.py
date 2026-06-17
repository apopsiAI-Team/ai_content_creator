"""
Content generation router with Research Hub integration
"""
import hashlib
from typing import Literal, Optional
from fastapi import APIRouter, Depends, HTTPException, Query, Request
from fastapi.responses import StreamingResponse
from pydantic import BaseModel, Field
import json
import traceback

from ..config import settings
from ..prompts.structure import StructureConfig
from ..services.llm_service import get_llm_service
from ..services.research_service import get_research_service
from ..services.esco_service import get_esco_service
from ..rate_limiter import get_rate_limiter, Priority
from ..auth import require_jwt


router = APIRouter(prefix="/api", tags=["generate"], dependencies=[Depends(require_jwt)])


def _extract_user_id(request: Request) -> str:
    """Extract user ID from X-Session-ID header, fallback to IP hash."""
    session_id = request.headers.get("X-Session-ID")
    if session_id:
        return session_id
    client_ip = request.client.host if request.client else "unknown"
    return hashlib.sha256(client_ip.encode()).hexdigest()[:16]


def _log_request(endpoint: str, user_id: str, document_id: str, **extras: object) -> None:
    """Emit a single-line log for an incoming request. Used to correlate calls per user/doc."""
    parts = [
        f"endpoint={endpoint}",
        f"user={user_id[:8]}",
        f"doc={(document_id or '-')[:24]}",
    ]
    for key, value in extras.items():
        parts.append(f"{key}={value}")
    print("[api] " + " ".join(parts), flush=True)


class Skill(BaseModel):
    code: str
    name: str
    type: str = "essential"


class Occupation(BaseModel):
    """ESCO occupation that the training program targets (program-level metadata)."""
    code: str = ""  # ESCO occupation code (e.g. "7223.4")
    name: str  # Human-readable occupation name (e.g. "χειριστής μηχανών ψυχρής ολκής")
    description: str = ""  # Optional ESCO description


class Module(BaseModel):
    number: int
    title: str
    hours: int = 0
    content: str = ""
    activities: str = ""
    skills: list[Skill] = []


class StructureConfigModel(BaseModel):
    """Optional structural elements to include (all default ON). The quality
    core and the Βιβλιογραφία are always present and not configurable."""
    activities: bool = True
    self_assessment: bool = True
    glossary: bool = True
    subsection_keywords: bool = True
    in_text_citations: bool = True


class GenerateRequest(BaseModel):
    module: Module
    use_research_hub: bool = True
    multipass: bool = True
    include_greek_sources: bool = True
    experimental_mode: bool = False  # Strict anti-hallucination mode
    user_instructions: str = ""  # Optional user guidance for content generation
    target_pages: Optional[int] = None  # Target page count (default ~20)
    learning_outcomes: str = ""  # Optional learning outcomes
    keywords: str = ""  # Optional comma-separated keywords
    previous_content: str = ""  # Approved content from previous batches (anti-overlap)
    batch_number: int = 1  # Current batch number
    total_batches: int = 1  # Estimated total batches for the module
    model_provider: Literal["claude", "openai"] = "claude"
    # Revision mode — apply targeted edits to current_draft instead of generating from scratch.
    mode: Literal["generate", "revision"] = "generate"
    current_draft: str = ""  # Only used when mode == "revision"
    document_id: str = ""  # Optional correlation id — stable per uploaded doc/draft session, surfaces in backend logs
    occupation: Optional[Occupation] = None  # Optional ESCO occupation context (program-level)
    structure_config: StructureConfigModel = Field(default_factory=StructureConfigModel)  # Which optional structural elements to include


class ReviewRequest(BaseModel):
    module: Module
    content: str
    model_provider: Literal["claude", "openai"] = "claude"
    document_id: str = ""  # Optional correlation id — stable per uploaded doc/draft session
    occupation: Optional[Occupation] = None  # Optional ESCO occupation context (program-level)


# ============== Research Endpoints ==============

@router.get("/research/search")
async def search_papers(
    query: str = Query(..., description="Search query"),
    limit: int = Query(15, description="Max results"),
    include_greek: bool = Query(True, description="Include Greek sources")
):
    """Search for academic papers"""
    research = get_research_service(settings.research_hub_path, settings.rust_research_hub_url)
    results = await research.search_papers(query, limit, include_greek)
    return {"papers": results, "count": len(results)}


# ============== Generation Endpoints ==============

@router.post("/generate")
async def generate_content(request: GenerateRequest):
    """
    Generate educational content with optional Research Hub integration
    and multi-pass generation
    """
    try:
        module_dict = request.module.model_dump()

        # Step 1: Get references from Research Hub
        references = []
        if request.use_research_hub:
            research = get_research_service(settings.research_hub_path, settings.rust_research_hub_url)

            # Extract keywords from module (Greek + English)
            keywords, english_query = _extract_keywords(module_dict)
            query = " ".join(keywords)

            # Search for papers (both Greek and English)
            references = await research.search_papers(
                query,
                limit=15,
                include_greek=request.include_greek_sources,
                english_keywords=english_query
            )

        # Step 2: Generate content
        llm = get_llm_service(request.model_provider)

        if request.multipass:
            # Multi-pass generation (higher quality)
            result = await llm.generate_content_multipass(
                module=module_dict,
                references=references
            )
        else:
            # Single pass (faster)
            content_parts = []
            async for chunk in llm.generate_content_stream(module_dict, references):
                content_parts.append(chunk)

            result = {
                "content": "".join(content_parts),
                "references": references,
                "quality_score": None
            }

        return {
            "content": result.get("content", ""),
            "references": references,
            "quality_score": result.get("quality_score"),
            "outline": result.get("outline"),
            "page_count": len(result.get("content", "")) // 3000
        }
    except HTTPException:
        raise
    except Exception as e:
        traceback.print_exc()
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/generate-stream")
async def generate_content_stream(request: GenerateRequest, raw_request: Request):
    """Stream content generation (single pass for real-time feedback)"""
    try:
        user_id = _extract_user_id(raw_request)
        occupation_dict = request.occupation.model_dump() if request.occupation else None
        _log_request(
            "/api/generate-stream",
            user_id,
            request.document_id,
            mode=request.mode,
            batch=f"{request.batch_number}/{request.total_batches}",
            module=request.module.number,
            provider=request.model_provider,
            occupation=(occupation_dict["code"] or occupation_dict["name"]) if occupation_dict else "-",
        )
        module_dict = request.module.model_dump()
        is_revision = request.mode == "revision"

        # Get references first (skipped in revision mode — draft already has them)
        references = []
        if request.use_research_hub and not is_revision:
            research = get_research_service(settings.research_hub_path, settings.rust_research_hub_url)
            # Use user-provided keywords if available (standard mode), else extract from module
            if request.keywords.strip():
                user_kws = [k.strip() for k in request.keywords.split(",") if k.strip()]
                keywords = user_kws[:5]
                english_query = " ".join(user_kws[:5])
            else:
                keywords, english_query = _extract_keywords(module_dict)
            query = " ".join(keywords)
            references = await research.search_papers(
                query,
                limit=15,
                english_keywords=english_query
            )

        llm = get_llm_service(request.model_provider)
        rate_limiter = get_rate_limiter()

        async def event_stream():
            # First, send references
            yield f"data: {json.dumps({'type': 'references', 'data': references})}\n\n"

            # Check queue position before streaming starts
            can_proceed = await rate_limiter.tracker.can_proceed(30000)
            sem_status = await rate_limiter.semaphore.get_status()
            heavy_full = sem_status["active_heavy"] >= sem_status["max_heavy"]

            if heavy_full or not can_proceed:
                est_wait = await rate_limiter.tracker.estimated_wait_seconds(30000)
                queue_len = sem_status["queue_heavy"] + 1
                yield f"data: {json.dumps({'type': 'queue', 'position': queue_len, 'estimated_wait': int(est_wait)})}\n\n"

            # Stream content (rate limiting happens inside generate_content_stream)
            async for chunk in llm.generate_content_stream(
                module_dict,
                references,
                experimental_mode=request.experimental_mode,
                user_instructions=request.user_instructions,
                target_pages=request.target_pages,
                learning_outcomes=request.learning_outcomes,
                keywords=request.keywords,
                previous_content=request.previous_content,
                batch_number=request.batch_number,
                total_batches=request.total_batches,
                user_id=user_id,
                mode=request.mode,
                current_draft=request.current_draft,
                occupation=occupation_dict,
                structure_config=StructureConfig(**request.structure_config.model_dump()),
            ):
                yield f"data: {json.dumps({'type': 'content', 'text': chunk})}\n\n"

            # Done
            yield f"data: {json.dumps({'type': 'done'})}\n\n"

        return StreamingResponse(
            event_stream(),
            media_type="text/event-stream"
        )
    except HTTPException:
        raise
    except Exception as e:
        traceback.print_exc()
        raise HTTPException(status_code=500, detail=str(e))


class SummaryRequest(BaseModel):
    module_title: str
    full_content: str  # All approved batches concatenated
    model_provider: Literal["claude", "openai"] = "claude"
    document_id: str = ""  # Optional correlation id — stable per uploaded doc/draft session


@router.post("/generate-summary")
async def generate_summary(request: SummaryRequest, raw_request: Request):
    """Generate a summary (Περίληψη) for a completed module."""
    try:
        if not request.full_content.strip():
            return {"summary": ""}

        user_id = _extract_user_id(raw_request)
        _log_request(
            "/api/generate-summary",
            user_id,
            request.document_id,
            provider=request.model_provider,
        )
        llm = get_llm_service(request.model_provider)
        summary_text = await llm.generate_summary(
            module_title=request.module_title,
            full_content=request.full_content,
            user_id=user_id,
        )

        return {"summary": summary_text}
    except HTTPException:
        raise
    except Exception as e:
        traceback.print_exc()
        raise HTTPException(status_code=500, detail=str(e))


class BibliographyRequest(BaseModel):
    citations: list[str]  # e.g., ["Deming, 1986", "Porter & Kramer, 2011"]
    topic: str = ""  # Module topic for context
    model_provider: Literal["claude", "openai"] = "claude"
    document_id: str = ""  # Optional correlation id — stable per uploaded doc/draft session


@router.post("/generate-bibliography")
async def generate_bibliography(request: BibliographyRequest, raw_request: Request):
    """Generate full APA bibliography entries from in-text citations.

    Used as fallback when main generation was truncated and bibliography is missing.
    """
    try:
        if not request.citations:
            return {"bibliography": ""}

        user_id = _extract_user_id(raw_request)
        _log_request(
            "/api/generate-bibliography",
            user_id,
            request.document_id,
            citations=len(request.citations),
            provider=request.model_provider,
        )
        llm = get_llm_service(request.model_provider)
        bibliography_text = await llm.generate_bibliography(
            citations=request.citations,
            topic=request.topic,
            user_id=user_id,
        )

        return {"bibliography": bibliography_text}
    except HTTPException:
        raise
    except Exception as e:
        traceback.print_exc()
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/review")
async def review_skill_coverage(request: ReviewRequest, raw_request: Request):
    """Generate ESCO skill coverage review"""
    try:
        user_id = _extract_user_id(raw_request)
        occupation_dict = request.occupation.model_dump() if request.occupation else None
        _log_request(
            "/api/review",
            user_id,
            request.document_id,
            module=request.module.number,
            skills=len(request.module.skills),
            provider=request.model_provider,
            occupation=(occupation_dict["code"] or occupation_dict["name"]) if occupation_dict else "-",
        )
        module_dict = request.module.model_dump()

        # Get skill descriptions from ESCO
        esco = get_esco_service(settings.esco_data_path)
        skill_names = [s.get("name", "") for s in module_dict.get("skills", [])]
        skill_descriptions = esco.lookup(skill_names)

        # Generate review
        llm = get_llm_service(request.model_provider)
        review = await llm.generate_skill_review(
            module=module_dict,
            content=request.content,
            skill_descriptions=skill_descriptions,
            user_id=user_id,
            occupation=occupation_dict,
        )

        # Calculate stats
        skill_analysis = review.get("skillAnalysis", [])
        total = len(skill_analysis)
        full = sum(1 for s in skill_analysis if s.get("coverageLevel") == "full")
        partial = sum(1 for s in skill_analysis if s.get("coverageLevel") == "partial")
        missing = sum(1 for s in skill_analysis if s.get("coverageLevel") == "missing")

        coverage_pct = ((full * 100) + (partial * 50)) // total if total > 0 else 0

        return {
            "moduleNumber": module_dict.get("number"),
            "totalSkills": total,
            "coveredFully": full,
            "coveredPartially": partial,
            "missing": missing,
            "coveragePercentage": coverage_pct,
            "skillAnalysis": skill_analysis,
            "overallAssessment": review.get("overallAssessment", ""),
            "recommendations": review.get("recommendations", [])
        }
    except HTTPException:
        raise
    except Exception as e:
        traceback.print_exc()
        raise HTTPException(status_code=500, detail=str(e))


def _extract_keywords(module: dict) -> tuple[list[str], str]:
    """Extract search keywords from module.

    Returns:
        tuple: (greek_keywords, english_query_string)
    """
    keywords = []

    # From title
    title = module.get("title", "")
    if title:
        keywords.extend(title.split()[:5])

    # From skills
    for skill in module.get("skills", []):
        name = skill.get("name", "")
        if name:
            keywords.append(name)

    # Comprehensive Greek to English translation for educational terms
    translations = {
        # Management & Business
        "διοίκηση": "management",
        "οργάνωση": "organization",
        "επιχείρηση": "business",
        "επιχειρήσεις": "business",
        "στρατηγική": "strategy",
        "ηγεσία": "leadership",
        "διαχείριση": "management",
        "πωλήσεις": "sales",
        "μάρκετινγκ": "marketing",
        "πελάτες": "customers",
        "προϊόντα": "products",
        "υπηρεσίες": "services",
        "γραφείο": "office",
        "επικοινωνία": "communication",
        # Education & Training
        "εκπαίδευση": "education",
        "κατάρτιση": "training",
        "μάθηση": "learning",
        "διδασκαλία": "teaching",
        "αξιολόγηση": "evaluation",
        "δεξιότητες": "skills",
        "ικανότητες": "competencies",
        # Technology
        "τεχνολογία": "technology",
        "ψηφιακός": "digital",
        "πληροφορική": "informatics",
        "υπολογιστές": "computers",
        "λογισμικό": "software",
        "δεδομένα": "data",
        # General
        "ανάπτυξη": "development",
        "έρευνα": "research",
        "ανάλυση": "analysis",
        "σχεδιασμός": "design planning",
        "υλοποίηση": "implementation",
        "ποιότητα": "quality",
        "καινοτομία": "innovation",
        "βελτίωση": "improvement",
    }

    english_keywords = []
    for kw in keywords:
        kw_lower = kw.lower()
        if kw_lower in translations:
            english_keywords.append(translations[kw_lower])

    # Build English query string
    english_query = " ".join(english_keywords[:5]) if english_keywords else ""

    return keywords[:5], english_query
