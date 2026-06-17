"""
Research Service - Integrates with Rust Research Hub HTTP API and Greek academic sources

The Rust Research Hub provides multi-source academic paper search across:
- CrossRef (DOI metadata)
- Semantic Scholar (AI papers)
- Unpaywall (Open Access)
- arXiv, PubMed, bioRxiv, OpenAlex, CORE, OpenReview
"""
import asyncio
from pathlib import Path
from typing import Optional
import httpx


class ResearchService:
    def __init__(
        self,
        research_hub_path: Path,
        rust_http_url: str = "http://localhost:8091"
    ):
        self.research_hub_path = research_hub_path
        self.mcp_binary = research_hub_path / "target" / "release" / "rust-research-mcp"
        self.rust_http_url = rust_http_url
        self._rust_available: Optional[bool] = None

    async def search_papers(
        self,
        query: str,
        limit: int = 15,
        include_greek: bool = True,
        english_keywords: str = ""
    ) -> list[dict]:
        """Search for academic papers using Rust Research Hub (multi-source)
        with fallback to CrossRef direct.

        Args:
            query: Search query (can be Greek or English)
            limit: Maximum number of results
            include_greek: Whether to include Greek sources
            english_keywords: Additional English keywords for international papers
        """
        results = []

        # 1. ALWAYS search CrossRef (primary — reliable journal articles with DOIs)
        crossref_results = await self._search_crossref_direct(query, limit)
        results.extend(crossref_results)
        print(f"CrossRef returned {len(crossref_results)} results for query: {query}")

        # 2. Also search Rust Research Hub (supplementary — multi-source)
        rust_results = await self._search_rust_http(query, limit)
        if rust_results:
            results.extend(rust_results)
            print(f"Rust Research Hub returned {len(rust_results)} additional results for query: {query}")

        # 3. Also search with English keywords for international papers
        if english_keywords and english_keywords != query:
            english_crossref = await self._search_crossref_direct(english_keywords, limit // 2)
            results.extend(english_crossref)
            print(f"English CrossRef returned {len(english_crossref)} results for: {english_keywords}")

            english_rust = await self._search_rust_http(english_keywords, limit // 2)
            if english_rust:
                results.extend(english_rust)
                print(f"English Research Hub returned {len(english_rust)} results for: {english_keywords}")

        # 4. Search Greek sources (optional, fewer results - may not have year)
        if include_greek:
            greek_results = await self._search_greek_sources(query, max(3, limit // 5))
            results.extend(greek_results)
            print(f"Greek sources returned {len(greek_results)} results")

        # 5. Filter out undergraduate theses/dissertations
        filtered = self._filter_theses(results)

        # 6. Deduplicate by DOI or title
        unique_results = self._deduplicate(filtered)

        # 7. Rank and limit - prioritize papers with DOI and year
        ranked = sorted(unique_results, key=lambda x: (
            x.get("doi") is not None,  # Has DOI
            x.get("year") is not None,  # Has year
            x.get("year") or 0  # Recent first
        ), reverse=True)

        return ranked[:limit]

    async def _search_rust_http(self, query: str, limit: int) -> list[dict]:
        """Call Rust Research Hub HTTP API for multi-source paper search"""
        # Check if we already know Rust service is unavailable
        if self._rust_available is False:
            return []

        async with httpx.AsyncClient(timeout=60) as client:
            try:
                response = await client.post(
                    f"{self.rust_http_url}/api/search",
                    json={
                        "query": query,
                        "limit": limit,
                        "search_type": "auto"
                    }
                )

                if response.status_code == 200:
                    data = response.json()
                    if data.get("success") and data.get("data"):
                        self._rust_available = True
                        papers = data["data"].get("papers", [])
                        providers = data["data"].get("providers_used", [])
                        print(f"  Sources queried: {', '.join(providers)}")
                        return [self._format_rust_paper(p) for p in papers]

            except httpx.ConnectError:
                print(f"Rust Research Hub not available at {self.rust_http_url}")
                self._rust_available = False
            except httpx.TimeoutException:
                print("Rust Research Hub request timed out")
            except Exception as e:
                print(f"Rust Research Hub error: {e}")

        return []

    def _format_rust_paper(self, paper: dict) -> dict:
        """Format Rust Research Hub paper to standard format"""
        authors = paper.get("authors", [])
        # Format authors: "Family, G." style
        formatted_authors = []
        for author in authors[:5]:  # Limit to 5 authors
            if isinstance(author, str):
                # Already formatted or single name
                formatted_authors.append(author)
            elif isinstance(author, dict):
                family = author.get("family", "")
                given = author.get("given", "")
                if family:
                    name = f"{family}, {given[:1]}." if given else family
                    formatted_authors.append(name)

        return {
            "doi": paper.get("doi"),
            "title": paper.get("title"),
            "authors": formatted_authors if formatted_authors else ["Unknown"],
            "year": paper.get("year"),
            "journal": paper.get("journal"),
            "abstract": paper.get("abstract"),
            "pdf_url": paper.get("pdf_url"),
            "source": paper.get("source", "rust_research_hub")
        }

    async def _search_crossref_direct(self, query: str, limit: int) -> list[dict]:
        """Direct CrossRef API search - primary source for international papers"""
        results = []
        async with httpx.AsyncClient(timeout=30) as client:
            try:
                # Add filters for better quality results
                response = await client.get(
                    "https://api.crossref.org/works",
                    params={
                        "query": query,
                        "rows": limit,
                        "filter": "type:journal-article,has-abstract:true",
                        "sort": "relevance",
                        "select": "DOI,title,author,published-print,published-online,container-title,abstract"
                    },
                    headers={
                        "User-Agent": "EducationalMaterialCreator/1.0 (mailto:contact@example.com)"
                    }
                )
                print(f"CrossRef API status: {response.status_code}")
                if response.status_code == 200:
                    data = response.json()
                    items = data.get("message", {}).get("items", [])
                    print(f"CrossRef returned {len(items)} items")
                    for item in items:
                        formatted = self._format_crossref_item(item)
                        if formatted.get("title"):  # Only add if has title
                            results.append(formatted)
                else:
                    print(f"CrossRef error response: {response.text[:200]}")
            except Exception as e:
                print(f"CrossRef API error: {e}")
        return results

    def _format_crossref_item(self, item: dict) -> dict:
        """Format CrossRef item to standard format"""
        authors = []
        for author in item.get("author", [])[:5]:  # Limit to first 5 authors
            family = author.get("family", "")
            given = author.get("given", "")
            if family:
                name = f"{family}, {given[:1]}." if given else family
                authors.append(name.strip(", ."))

        title = item.get("title", [""])[0] if item.get("title") else ""

        # Try both published-print and published-online for year
        year = None
        for date_field in ["published-print", "published-online", "created"]:
            if item.get(date_field):
                date_parts = item[date_field].get("date-parts", [[None]])
                if date_parts and date_parts[0] and date_parts[0][0]:
                    year = date_parts[0][0]
                    break

        journal = ""
        if item.get("container-title"):
            journal = item["container-title"][0] if isinstance(item["container-title"], list) else item["container-title"]

        return {
            "doi": item.get("DOI"),
            "title": title,
            "authors": authors if authors else ["Unknown"],
            "year": year,
            "journal": journal,
            "source": "crossref"
        }

    async def _search_greek_sources(self, query: str, limit: int) -> list[dict]:
        """Search Greek academic repositories"""
        results = []

        # Kallipos repository
        kallipos_results = await self._search_kallipos(query, limit)
        results.extend(kallipos_results)

        return results

    async def _search_kallipos(self, query: str, limit: int) -> list[dict]:
        """Search Kallipos repository for Greek academic books"""
        results = []
        async with httpx.AsyncClient(timeout=30) as client:
            try:
                # Kallipos OAI-PMH or REST API
                # Note: This is a simplified implementation
                response = await client.get(
                    "https://repository.kallipos.gr/rest/items/find-by-metadata-field",
                    params={
                        "key": "dc.title",
                        "value": query,
                        "limit": limit
                    }
                )
                if response.status_code == 200:
                    data = response.json()
                    for item in data:
                        results.append({
                            "doi": item.get("handle"),
                            "title": item.get("name", ""),
                            "authors": [],
                            "year": None,
                            "journal": "Kallipos",
                            "url": f"https://repository.kallipos.gr/handle/{item.get('handle', '')}",
                            "source": "kallipos"
                        })
            except Exception as e:
                print(f"Kallipos search error: {e}")
        return results

    def _filter_theses(self, results: list[dict]) -> list[dict]:
        """Filter out undergraduate/master theses (doctoral dissertations allowed)"""
        THESIS_KEYWORDS = [
            # Greek — undergraduate/master only
            "πτυχιακή εργασία", "πτυχιακή", "διπλωματική εργασία", "διπλωματική",
            "μεταπτυχιακή εργασία", "μεταπτυχιακή διατριβή",
            # English — undergraduate/master only
            "undergraduate thesis", "bachelor thesis", "bachelor's thesis",
            "master thesis", "master's thesis", "msc thesis",
            "diploma thesis", "diploma work",
        ]
        filtered = []
        removed = 0
        for item in results:
            title = (item.get("title") or "").lower()
            journal = (item.get("journal") or "").lower()
            text = f"{title} {journal}"
            if any(kw in text for kw in THESIS_KEYWORDS):
                removed += 1
                continue
            filtered.append(item)
        if removed:
            print(f"Filtered out {removed} theses/dissertations")
        return filtered

    def _deduplicate(self, results: list[dict]) -> list[dict]:
        """Remove duplicate papers by DOI or title, prioritize papers with year"""
        seen_dois = set()
        seen_titles = set()
        unique = []

        # First pass: papers WITH year (prioritized)
        for item in results:
            if not item.get("year"):
                continue

            doi = item.get("doi")
            title_raw = item.get("title")
            title = title_raw.strip().lower() if isinstance(title_raw, str) else ""

            if doi and doi in seen_dois:
                continue
            if title and title in seen_titles:
                continue

            if doi:
                seen_dois.add(doi)
            if title:
                seen_titles.add(title)
            unique.append(item)

        # Second pass: papers without year (only if we need more)
        if len(unique) < 5:
            for item in results:
                if item.get("year"):
                    continue  # Already processed

                doi = item.get("doi")
                title_raw = item.get("title")
                title = title_raw.strip().lower() if isinstance(title_raw, str) else ""

                if doi and doi in seen_dois:
                    continue
                if title and title in seen_titles:
                    continue

                if doi:
                    seen_dois.add(doi)
                if title:
                    seen_titles.add(title)
                # Mark as "n.d." instead of leaving empty
                item["year_display"] = "n.d."
                unique.append(item)

        return unique

    def format_references_for_prompt(self, references: list[dict]) -> str:
        """Format references for Claude prompt"""
        lines = []
        for i, ref in enumerate(references, 1):
            authors = ", ".join(ref.get("authors", [])) or "Unknown"
            year = ref.get("year", "n.d.")
            title = ref.get("title", "")
            journal = ref.get("journal", "")
            doi = ref.get("doi", "")

            citation = f"{i}. {authors} ({year}). {title}."
            if journal:
                citation += f" {journal}."
            if doi:
                citation += f" https://doi.org/{doi}"

            lines.append(citation)

        return "\n".join(lines)


# Singleton instance
_research_service: Optional[ResearchService] = None


def get_research_service(
    research_hub_path: Path,
    rust_http_url: str = "http://localhost:8091"
) -> ResearchService:
    global _research_service
    if _research_service is None:
        _research_service = ResearchService(research_hub_path, rust_http_url)
    return _research_service


async def check_rust_service_health(url: str = "http://localhost:8091") -> bool:
    """Check if Rust Research Hub HTTP service is healthy"""
    async with httpx.AsyncClient(timeout=5) as client:
        try:
            response = await client.get(f"{url}/health")
            if response.status_code == 200:
                data = response.json()
                print(f"✓ Rust Research Hub: {data.get('status')} (v{data.get('version', '?')})")
                return True
        except Exception as e:
            print(f"✗ Rust Research Hub not available: {e}")
    return False
