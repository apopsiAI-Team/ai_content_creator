import type { StructureConfig } from '../store/useStore';

// Backend API service - Python FastAPI backend.
// Default to same-origin (/api) so Cloudflare tunnels work without CORS issues.
const API_BASE = (import.meta.env.VITE_API_URL || '').replace(/\/$/, '');
const apiUrl = (path: string) => `${API_BASE}${path}`;
const AUTH_TOKEN_STORAGE_KEY = 'edu-material-auth-token';

export function initializeAuthFromUrl(): boolean {
  const params = new URLSearchParams(window.location.search);
  const token = params.get('token');
  if (!token) return false;

  sessionStorage.setItem(AUTH_TOKEN_STORAGE_KEY, token);
  params.delete('token');
  const nextSearch = params.toString();
  const nextUrl = `${window.location.pathname}${nextSearch ? `?${nextSearch}` : ''}${window.location.hash}`;
  window.history.replaceState({}, document.title, nextUrl);
  return true;
}

function getAuthToken(): string {
  return sessionStorage.getItem(AUTH_TOKEN_STORAGE_KEY) || '';
}

async function apiError(response: Response): Promise<Error> {
  if (response.status === 401) {
    return new Error('Δεν υπάρχει ενεργή πρόσβαση. Παρακαλώ ανοίξτε το εργαλείο μέσα από την πλατφόρμα e-mentoring.');
  }
  if (response.status === 403) {
    return new Error('Ο λογαριασμός σας δεν έχει δικαίωμα πρόσβασης σε αυτό το εργαλείο.');
  }
  const error = await response.json().catch(() => null);
  return new Error(error?.detail || error?.error || `HTTP ${response.status}`);
}

// Stable session ID for rate limiting (persists for this tab's lifetime).
// crypto.randomUUID is not available in every non-secure LAN origin (http://192.168.x.x).
function makeSessionId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `session-${Date.now()}-${Math.random().toString(36).slice(2, 12)}`;
}

const SESSION_USER_ID = makeSessionId();

/** Common headers for all API calls. */
function apiHeaders(extra?: Record<string, string>): Record<string, string> {
  const token = getAuthToken();
  return {
    'Content-Type': 'application/json',
    'X-Session-ID': SESSION_USER_ID,
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
    ...extra,
  };
}

type SseEvent = Record<string, unknown>;

function processSseBuffer(
  buffer: string,
  onEvent: (event: SseEvent) => void
): string {
  const events = buffer.split('\n\n');
  const completeEvents = events.slice(0, -1);
  const remainder = events[events.length - 1] || '';

  for (const rawEvent of completeEvents) {
    const dataLines = rawEvent
      .split('\n')
      .filter((line) => line.startsWith('data:'))
      .map((line) => line.slice(5).trimStart());

    if (dataLines.length === 0) continue;

    const payload = dataLines.join('\n');
    if (!payload) continue;

    try {
      const parsed = JSON.parse(payload) as SseEvent;
      onEvent(parsed);
    } catch {
      // Ignore malformed event payloads and continue streaming.
    }
  }

  return remainder;
}

interface HealthStatus {
  status: string;
  hasApiKey: boolean;
  escoSkillsLoaded: boolean;
}

interface ClaudeMessage {
  role: 'user' | 'assistant';
  content: string;
}

interface ClaudeResponse {
  content: Array<{ type: string; text?: string }>;
  usage: { input_tokens: number; output_tokens: number };
}

// Health check
export async function checkHealth(): Promise<HealthStatus> {
  const response = await fetch(apiUrl('/api/health'));
  if (!response.ok) throw new Error('Backend not available');
  return response.json();
}

// Claude content generation
export async function generateContent(
  system: string,
  messages: ClaudeMessage[],
  maxTokens = 16000,
  modelProvider: 'claude' | 'openai' = 'claude'
): Promise<ClaudeResponse> {
  const response = await fetch(apiUrl('/api/claude/generate'), {
    method: 'POST',
    headers: apiHeaders(),
    body: JSON.stringify({ system, messages, maxTokens, model_provider: modelProvider }),
  });

  if (!response.ok) {
    throw await apiError(response);
  }

  return response.json();
}

// Claude streaming generation (legacy - for simple prompts)
export async function generateContentStream(
  system: string,
  messages: ClaudeMessage[],
  onChunk: (text: string) => void,
  maxTokens = 16000,
  modelProvider: 'claude' | 'openai' = 'claude'
): Promise<void> {
  const response = await fetch(apiUrl('/api/claude/generate-stream'), {
    method: 'POST',
    headers: apiHeaders(),
    body: JSON.stringify({ system, messages, maxTokens, model_provider: modelProvider }),
  });

  if (!response.ok) {
    throw await apiError(response);
  }

  const reader = response.body?.getReader();
  if (!reader) throw new Error('No response body');

  const decoder = new TextDecoder();
  let buffer = '';

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    buffer += decoder.decode(value, { stream: true });
    buffer = processSseBuffer(buffer, (data) => {
      if (typeof data.text === 'string') onChunk(data.text);
    });
  }

  buffer += decoder.decode();
  processSseBuffer(buffer, (data) => {
    if (typeof data.text === 'string') onChunk(data.text);
  });
}

// Module interface for generation
interface ModuleData {
  number: number;
  title: string;
  hours?: number;
  content?: string;
  activities?: string;
  skills?: Array<{ code: string; name: string; type: string }>;
}

// ESCO occupation that the training program targets (program-level metadata).
// Sent at the top level of generate/review requests so the backend can inject it
// into the prompt as context. Optional.
export interface Occupation {
  code?: string;
  name: string;
  description?: string;
}

// Generation result with Research Hub references
interface GenerationResult {
  content: string;
  references: Array<{
    title: string;
    authors: string[];
    year: number;
    journal?: string;
    doi?: string;
  }>;
  quality_score?: {
    academic_style: number;
    paragraph_quality: number;
    citations: number;
    structure: number;
    coverage: number;
    overall: number;
    notes: string;
  };
  outline?: string;
  page_count: number;
}

// Enhanced generation with Research Hub + Multi-pass (HIGH QUALITY)
export async function generateEducationalContent(
  module: ModuleData,
  useResearchHub = true,
  multipass = true,
  includeGreekSources = true,
  experimentalMode = false,
  modelProvider: 'claude' | 'openai' = 'claude'
): Promise<GenerationResult> {
  const response = await fetch(apiUrl('/api/generate'), {
    method: 'POST',
    headers: apiHeaders(),
    body: JSON.stringify({
      module,
      use_research_hub: useResearchHub,
      multipass,
      include_greek_sources: includeGreekSources,
      experimental_mode: experimentalMode,
      model_provider: modelProvider,
    }),
  });

  if (!response.ok) {
    throw await apiError(response);
  }

  return response.json();
}

// Streaming generation with Research Hub (for real-time feedback)
export async function generateEducationalContentStream(
  module: ModuleData,
  onChunk: (text: string) => void,
  onReferences?: (refs: GenerationResult['references']) => void,
  useResearchHub = true,
  experimentalMode = false,
  userInstructions = '',
  targetPages?: number | null,
  learningOutcomes?: string,
  keywords?: string,
  previousContent?: string,
  batchNumber = 1,
  totalBatches = 1,
  signal?: AbortSignal,
  onQueue?: (position: number, estimatedWait: number) => void,
  modelProvider: 'claude' | 'openai' = 'claude',
  mode: 'generate' | 'revision' = 'generate',
  currentDraft = '',
  documentId = '',
  occupation: Occupation | null = null,
  structureConfig?: StructureConfig,
): Promise<void> {
  const response = await fetch(apiUrl('/api/generate-stream'), {
    method: 'POST',
    headers: apiHeaders(),
    signal,
    body: JSON.stringify({
      module,
      use_research_hub: useResearchHub,
      multipass: false, // streaming is single-pass
      include_greek_sources: true,
      experimental_mode: experimentalMode,
      user_instructions: userInstructions,
      target_pages: targetPages || null,
      learning_outcomes: learningOutcomes || '',
      keywords: keywords || '',
      previous_content: previousContent || '',
      batch_number: batchNumber,
      total_batches: totalBatches,
      model_provider: modelProvider,
      mode,
      current_draft: currentDraft,
      document_id: documentId,
      occupation,
      ...(structureConfig ? { structure_config: structureConfig } : {}),
    }),
  });

  if (!response.ok) {
    throw await apiError(response);
  }

  const reader = response.body?.getReader();
  if (!reader) throw new Error('No response body');

  const decoder = new TextDecoder();
  let buffer = '';

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    buffer += decoder.decode(value, { stream: true });
    buffer = processSseBuffer(buffer, (data) => {
      if (data.type === 'content' && typeof data.text === 'string') {
        onChunk(data.text);
      } else if (data.type === 'references' && onReferences && Array.isArray(data.data)) {
        onReferences(data.data as GenerationResult['references']);
      } else if (data.type === 'queue' && onQueue) {
        onQueue(data.position as number, data.estimated_wait as number);
      }
    });
  }

  buffer += decoder.decode();
  processSseBuffer(buffer, (data) => {
    if (data.type === 'content' && typeof data.text === 'string') {
      onChunk(data.text);
    } else if (data.type === 'references' && onReferences && Array.isArray(data.data)) {
      onReferences(data.data as GenerationResult['references']);
    } else if (data.type === 'queue' && onQueue) {
      onQueue(data.position as number, data.estimated_wait as number);
    }
  });
}

// Generate full APA bibliography from in-text citations (fallback)
export async function generateBibliography(
  citations: string[],
  topic: string = '',
  modelProvider: 'claude' | 'openai' = 'claude',
  documentId = '',
): Promise<string> {
  const response = await fetch(apiUrl('/api/generate-bibliography'), {
    method: 'POST',
    headers: apiHeaders(),
    body: JSON.stringify({ citations, topic, model_provider: modelProvider, document_id: documentId }),
  });

  if (!response.ok) {
    throw await apiError(response);
  }

  const data = await response.json();
  return data.bibliography;
}

// Generate summary (Περίληψη) for a completed module
export async function generateSummary(
  moduleTitle: string,
  fullContent: string,
  modelProvider: 'claude' | 'openai' = 'claude',
  documentId = '',
): Promise<string> {
  const response = await fetch(apiUrl('/api/generate-summary'), {
    method: 'POST',
    headers: apiHeaders(),
    body: JSON.stringify({
      module_title: moduleTitle,
      full_content: fullContent,
      model_provider: modelProvider,
      document_id: documentId,
    }),
  });

  if (!response.ok) {
    throw await apiError(response);
  }

  const data = await response.json();
  return data.summary;
}

// Shape of a single skill analysis entry returned by /api/review (matches SkillAnalysis in useStore).
interface SkillAnalysisDTO {
  skillCode: string;
  skillName: string;
  skillType: 'essential' | 'optional';
  coverageLevel: 'full' | 'partial' | 'missing';
  evidence: string[];
  contentSections: string[];
  notes: string;
}

// Shape returned by /api/review — same as SkillCoverageReview minus generatedAt (added client-side).
interface ReviewResponseDTO {
  moduleNumber: number;
  totalSkills: number;
  coveredFully: number;
  coveredPartially: number;
  missing: number;
  coveragePercentage: number;
  skillAnalysis: SkillAnalysisDTO[];
  overallAssessment: string;
  recommendations: string[];
}

interface ReviewModuleInput {
  number: number;
  title: string;
  hours?: number;
  skills: Array<{ code: string; name: string; type: string }>;
}

// ESCO skill coverage review — backend builds prompt with ESCO descriptions, calls LLM, returns parsed JSON.
export async function reviewSkillCoverage(
  module: ReviewModuleInput,
  content: string,
  modelProvider: 'claude' | 'openai' = 'claude',
  documentId = '',
  occupation: Occupation | null = null,
): Promise<ReviewResponseDTO> {
  const response = await fetch(apiUrl('/api/review'), {
    method: 'POST',
    headers: apiHeaders(),
    body: JSON.stringify({
      module,
      content,
      model_provider: modelProvider,
      document_id: documentId,
      occupation,
    }),
  });

  if (!response.ok) {
    throw await apiError(response);
  }

  return response.json();
}

export interface PlatformExportResponse {
  status: 'ok';
  platform_status: number;
  platform_response: unknown;
}

export async function exportGeneratedMaterialToPlatform(data: Record<string, unknown>): Promise<PlatformExportResponse> {
  const response = await fetch(apiUrl('/api/export/platform'), {
    method: 'POST',
    headers: apiHeaders(),
    body: JSON.stringify({ data }),
  });

  if (!response.ok) {
    throw await apiError(response);
  }

  return response.json();
}
