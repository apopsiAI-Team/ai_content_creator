# OpenAlex Provider Setup and Usage Guide

## Overview

**Good news: OpenAlex is already configured and ready to use!** 🎉

OpenAlex is a fully open catalog of scholarly papers that provides access to **240+ million scholarly works** with no authentication requirements. It's one of the most comprehensive academic databases available and is completely free to use.

### Key Features
- **240+ million scholarly works** from across all disciplines
- **No API key required** - completely open access
- **High-quality metadata** including abstracts, authors, citations
- **Open access PDF detection** with direct links
- **Author disambiguation** and institutional affiliations
- **Citation networks** and impact metrics
- **Real-time updates** as new papers are published

## ✅ Zero Setup Required

OpenAlex is already fully integrated into your Rust Research MCP server as a high-priority provider. Here's what's already configured:

### Pre-configured Settings
- **Base URL**: `https://api.openalex.org/works`
- **Rate Limit**: 1.15 requests per second (respects OpenAlex's 100K/day limit)
- **User Agent**: Properly set with contact email for polite API usage
- **Timeout**: 30-second request timeout
- **Priority**: 180 (high priority in meta search results)

### Current Integration
OpenAlex is automatically included when you use the search tools:
- Integrated into `search_papers` MCP tool
- Included in meta-search aggregation
- Automatic deduplication with other providers
- Health check monitoring enabled

## Configuration Options

While OpenAlex works out of the box, you can customize its behavior through environment variables:

### Environment Variables (.env file)

```bash
# OpenAlex API Configuration
OPENALEX_API_URL=https://api.openalex.org/works
OPENALEX_RATE_LIMIT=1.15

# General settings that affect OpenAlex
REQUEST_TIMEOUT=30
RATE_LIMIT=1
```

### Configuration Details

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENALEX_API_URL` | `https://api.openalex.org/works` | Base API endpoint |
| `OPENALEX_RATE_LIMIT` | `1.15` | Requests per second (max: ~1.15) |
| `REQUEST_TIMEOUT` | `30` | Timeout in seconds for API requests |

## Usage Examples

### 1. Search by Keywords

```bash
# Search for papers about machine learning
cargo run -- search "machine learning" --max-results 10
```

**What OpenAlex provides:**
- Relevant papers with abstracts
- Author information with affiliations
- Publication venues and years
- Open access PDF links when available
- Citation counts and impact metrics

### 2. Search by DOI

```bash
# Find specific paper by DOI
cargo run -- search "10.1038/nature12373" --search-type doi
```

**OpenAlex advantages:**
- Fastest DOI resolution
- Most comprehensive metadata
- Alternative format detection (handles various DOI formats)

### 3. Search by Author

```bash
# Find papers by specific author
cargo run -- search "Geoffrey Hinton" --search-type author --max-results 20
```

**Author features:**
- Author disambiguation (distinguishes between authors with same name)
- Institutional affiliations over time
- Collaboration networks
- Career trajectory analysis

### 4. Search by Title

```bash
# Find specific paper by title
cargo run -- search "Attention Is All You Need" --search-type title
```

**Title search benefits:**
- Fuzzy matching for slight variations
- Handles punctuation and formatting differences
- Finds papers even with incomplete titles

### 5. Subject-based Search

```bash
# Search within specific subject areas
cargo run -- search "neural networks" --search-type subject
```

## API Response Details

### Data Mapping

OpenAlex provides rich metadata that gets mapped to your `PaperMetadata` structure:

```rust
// What you get from OpenAlex
PaperMetadata {
    title: "Paper title from OpenAlex",
    authors: vec!["Author 1", "Author 2"], // Disambiguated authors
    abstract_text: Some("Full abstract reconstructed from inverted index"),
    doi: Some("10.1000/example"),
    url: Some("https://openalex.org/W123456"),
    pdf_url: Some("https://arxiv.org/pdf/paper.pdf"), // Best open access location
    publication_year: Some(2023),
    journal: Some("Nature"), // Source venue
    // ... additional metadata
}
```

### Abstract Reconstruction

OpenAlex uses an **inverted index** format for abstracts to save space. Your implementation automatically reconstructs the full abstract text:

```json
// OpenAlex returns this:
"abstract_inverted_index": {
    "This": [0],
    "paper": [1],
    "presents": [2],
    "a": [3],
    "new": [4],
    "method": [5]
}

// Your system reconstructs to:
"This paper presents a new method"
```

### Open Access Detection

OpenAlex prioritizes PDF access in this order:

1. **Best OA Location**: Highest quality open access version
2. **Primary Location**: Publisher's official version
3. **Alternative Sources**: Repository versions, preprints

## Advanced Search Features

### 1. Pagination Support

OpenAlex uses cursor-based pagination for efficient large result sets:

```rust
// Automatically handled - you can request up to 200 results per query
SearchQuery {
    query: "your search term".to_string(),
    max_results: 200, // OpenAlex maximum
    // Cursor pagination handled automatically
}
```

### 2. Result Filtering

OpenAlex automatically provides high-quality results by:
- **Relevance ranking**: Most relevant papers first
- **Quality filtering**: Excludes low-quality or duplicate entries
- **Date sorting**: Most recent papers prioritized
- **Open access priority**: Papers with PDFs ranked higher

### 3. Metadata Enrichment

Beyond basic metadata, OpenAlex provides:
- **Citation counts**: How many times each paper is cited
- **Author H-index**: Author impact metrics
- **Institutional rankings**: University/organization prestige
- **Subject classification**: Automatic topic categorization
- **Related works**: Similar papers and citations

## Testing OpenAlex Integration

### 1. Run Provider Tests

```bash
# Test OpenAlex provider specifically
cargo nextest run -E 'test(openalex)'

# Run all provider tests
cargo nextest run --test providers_e2e_test
```

### 2. Health Check

```bash
# Check if OpenAlex is responding
cargo run -- health-check
```

The health check verifies:
- ✅ API endpoint accessibility
- ✅ Response time under threshold
- ✅ Rate limiting compliance
- ✅ Authentication status (none required)

### 3. Manual Testing

```bash
# Test basic functionality
cargo run -- search "rust programming language" --provider openalex

# Test DOI lookup
cargo run -- search "10.1145/3377811.3380330" --search-type doi --provider openalex

# Test rate limiting (should succeed without throttling)
for i in {1..5}; do
    cargo run -- search "test query $i" --provider openalex --max-results 5
done
```

## Performance Characteristics

### Response Times
- **Average**: 200-500ms per request
- **DOI lookups**: 100-300ms (fastest)
- **Complex searches**: 300-800ms
- **Large result sets**: 500-1000ms

### Rate Limiting
- **Limit**: 100,000 requests per day (~1.15 req/sec)
- **Implementation**: Token bucket with 1.15/sec refill
- **Burst**: Allows short bursts up to 5 requests
- **Backoff**: Automatic exponential backoff on rate limit

### Caching
- **Metadata TTL**: 24 hours (papers don't change often)
- **Search TTL**: 1 hour (search results may change)
- **Error TTL**: 5 minutes (retry failed requests sooner)

## Troubleshooting

### Common Issues

#### 1. Rate Limiting Errors

**Symptom**: `Rate limit exceeded` errors
**Solution**:
```bash
# Check current rate limit setting
grep OPENALEX_RATE_LIMIT .env

# Reduce if needed (but 1.15 should work)
OPENALEX_RATE_LIMIT=1.0
```

#### 2. Network Timeouts

**Symptom**: `Request timeout` errors
**Solution**:
```bash
# Increase timeout in .env
REQUEST_TIMEOUT=60

# Or retry the request (automatic exponential backoff)
```

#### 3. Empty Results

**Symptom**: No papers returned for valid queries
**Causes & Solutions**:

```bash
# Check query syntax
cargo run -- search "\"exact phrase\"" --search-type title

# Try broader search
cargo run -- search "machine learning" --search-type keywords

# Increase result limit
cargo run -- search "narrow topic" --max-results 50
```

#### 4. Invalid DOI Format

**Symptom**: `Invalid DOI format` error
**Solution**:
```bash
# Valid DOI formats
cargo run -- search "10.1038/nature12373" --search-type doi
cargo run -- search "https://doi.org/10.1038/nature12373" --search-type doi
cargo run -- search "doi:10.1038/nature12373" --search-type doi

# Invalid (will be rejected)
cargo run -- search "not-a-doi" --search-type doi
```

### Debug Mode

Enable debug logging to see OpenAlex API interactions:

```bash
# Enable debug logging
RUST_LOG=debug cargo run -- search "your query"

# Or just for OpenAlex
RUST_LOG=rust_research_mcp::client::providers::openalex=debug cargo run -- search "your query"
```

### Health Monitoring

Monitor OpenAlex provider health:

```bash
# Check provider statistics
cargo run -- stats

# View recent errors
tail -f ~/.local/share/rust-research-mcp/logs/provider-errors.log

# Monitor rate limiting
grep "rate limit" ~/.local/share/rust-research-mcp/logs/app.log
```

## Advanced Configuration

### Custom API Endpoint

If you need to use a mirror or proxy:

```bash
# In .env file
OPENALEX_API_URL=https://your-proxy.com/openalex/works
```

### Rate Limit Optimization

For high-volume usage, tune the rate limiter:

```bash
# Conservative (slower but safer)
OPENALEX_RATE_LIMIT=1.0

# Aggressive (faster but might hit limits)
OPENALEX_RATE_LIMIT=1.2  # Don't exceed 1.15 average
```

### User Agent Customization

The user agent is set to comply with OpenAlex's polite usage policy. It includes:
- Application name and version
- Purpose (Academic Research Tool)
- Contact email for issue resolution

## Integration with Other Providers

OpenAlex works seamlessly with other providers in your meta-search:

### Priority Order (Higher = Better)
1. **CrossRef** (190) - Authoritative DOI metadata
2. **Semantic Scholar** (185) - AI-enhanced features
3. **OpenAlex** (180) - Comprehensive coverage
4. **arXiv** (170) - Preprint access
5. **CORE** (160) - Repository aggregation

### Deduplication

Papers from OpenAlex are automatically deduplicated with results from other providers based on:
- **DOI matching** (exact)
- **Title similarity** (fuzzy)
- **Author overlap** (partial matching)
- **Year + venue** (combined matching)

## OpenAlex vs Other Providers

| Feature | OpenAlex | CrossRef | Semantic Scholar | arXiv |
|---------|----------|----------|------------------|-------|
| **Coverage** | 240M+ papers | 140M+ papers | 200M+ papers | 2M+ papers |
| **Auth Required** | ❌ No | ❌ No | ⚠️ Optional | ❌ No |
| **Abstracts** | ✅ Yes | ❌ No | ✅ Yes | ✅ Yes |
| **PDF Links** | ✅ Best OA | ❌ No | ✅ Some | ✅ All |
| **Citations** | ✅ Yes | ⚠️ Basic | ✅ Advanced | ❌ No |
| **Rate Limit** | 100K/day | Polite pool | 100/5min | 1/3sec |
| **Update Speed** | Daily | Real-time | Weekly | Real-time |

## Getting Help

### Documentation
- [OpenAlex Documentation](https://docs.openalex.org/)
- [API Reference](https://docs.openalex.org/api)
- [Data Schema](https://docs.openalex.org/api-entities/works)

### Community Resources
- [OpenAlex GitHub](https://github.com/ourresearch/openalex-guts)
- [OpenAlex Twitter](https://twitter.com/openalex_org)
- [Academic Community Forum](https://github.com/ourresearch/openalex-help)

### Support Channels
- **Technical Issues**: Check the health endpoint and logs
- **API Changes**: Monitor [OpenAlex changelog](https://docs.openalex.org/changelog)
- **Rate Limiting**: Verify your usage patterns with debug logging

---

## Summary

OpenAlex is a powerful, zero-configuration provider that gives you access to one of the world's largest academic databases. With no setup required, comprehensive metadata, and excellent open access detection, it's an essential component of your research toolkit.

**Key Takeaways:**
- ✅ **Ready to use** - No configuration needed
- 🔓 **Open access** - No API keys or registration
- 🎯 **High quality** - Comprehensive, clean metadata
- ⚡ **Fast** - Optimized for performance
- 🔄 **Integrated** - Works seamlessly with other providers

Just start searching - OpenAlex is already working behind the scenes to find the papers you need!