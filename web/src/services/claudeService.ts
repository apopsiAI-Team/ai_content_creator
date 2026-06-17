import type { Module, Reference, SkillCoverageReview, StructureConfig } from '../store/useStore';
import {
  generateEducationalContentStream,
  generateBibliography,
  reviewSkillCoverage,
  type Occupation,
} from './api';

interface GenerationResult {
  content: string;
  references: Reference[];
  pageCount: number;
}

function extractReferencesFromContent(content: string): Reference[] {
  const references: Reference[] = [];

  // Look for the bibliography section
  const biblioMatch = content.match(/##\s*Βιβλιογραφία\s*([\s\S]*?)(?=##|$)/i);
  if (biblioMatch) {
    const biblioText = biblioMatch[1];

    // Parse APA-style references
    const refPattern = /([^()]+?)\s*\((\d{4})\)\.\s*([^.]+)\.\s*([^.]*\.?)/g;
    let match;

    while ((match = refPattern.exec(biblioText)) !== null) {
      const authorsText = match[1].trim();
      const year = parseInt(match[2], 10);
      const title = match[3].trim();
      const journal = match[4].trim();

      // Parse APA-formatted authors: "Surname, A. B." pattern
      // Supports Latin and Greek characters, hyphenated surnames
      const authorPattern = /([A-ZΑ-Ω\u0386-\u03CE][A-Za-zΑ-Ωα-ωά-ώ\u0386-\u03CE\-]+(?:[\s\-][A-Za-zΑ-Ωα-ωά-ώ\u0386-\u03CE\-]+)*),\s*([A-ZΑ-Ω\u0386-\u03CE]\.(?:\s*[A-ZΑ-Ω\u0386-\u03CE]\.)*)/g;
      const authors: string[] = [];
      let authorMatch;
      while ((authorMatch = authorPattern.exec(authorsText)) !== null) {
        authors.push(authorMatch[0]); // Full match like "Chaffey, D."
      }
      // Fallback: if no APA-style authors matched, use whole string as single author
      if (authors.length === 0) {
        const cleaned = authorsText.replace(/&/g, '').trim();
        if (cleaned) authors.push(cleaned);
      }

      if (title && year) {
        references.push({
          title,
          authors,
          year,
          journal: journal || undefined,
          status: 'auto',
        });
      }
    }

    if (references.length > 0) return references;
  }

  // Fallback: extract in-text citations (Author, Year) from content body
  // This is especially useful for experimental mode where LLM may omit bibliography section
  const citationPattern = /\(([A-ZΑ-Ω\u0386-\u03CE][A-Za-zΑ-Ωα-ωά-ώ\u0386-\u03CE.\-\s]+?)(?:\s*(?:&|και)\s*([A-ZΑ-Ω\u0386-\u03CE][A-Za-zΑ-Ωα-ωά-ώ\u0386-\u03CE.\-\s]+?))?\s*(?:κ\.ά\.|et\s+al\.)?,\s*(\d{4})\)/g;
  const seen = new Set<string>();
  let citMatch;

  while ((citMatch = citationPattern.exec(content)) !== null) {
    const author1 = citMatch[1].trim();
    const author2 = citMatch[2]?.trim();
    const year = parseInt(citMatch[3], 10);

    const key = `${author1.toLowerCase()}-${year}`;
    if (!seen.has(key)) {
      seen.add(key);
      const authors: string[] = [author1];
      if (author2) authors.push(author2);
      references.push({
        title: '',
        authors,
        year,
        status: 'auto',
      });
    }
  }

  return references;
}

/**
 * Extract in-text citation strings like "Deming, 1986" from content.
 * Returns deduplicated list of citation strings for the bibliography follow-up call.
 */
function extractCitationStrings(content: string): string[] {
  const pattern = /\(([A-ZΑ-Ω\u0386-\u03CE][A-Za-zΑ-Ωα-ωά-ώ\u0386-\u03CE.\-\s]+?(?:\s*(?:&|και)\s*[A-ZΑ-Ω\u0386-\u03CE][A-Za-zΑ-Ωα-ωά-ώ\u0386-\u03CE.\-\s]+?)?(?:\s*(?:κ\.ά\.|et\s+al\.))?,\s*\d{4})\)/g;
  const seen = new Set<string>();
  const citations: string[] = [];
  let match;

  while ((match = pattern.exec(content)) !== null) {
    const citation = match[1].trim();
    const key = citation.toLowerCase();
    if (!seen.has(key)) {
      seen.add(key);
      citations.push(citation);
    }
  }

  return citations;
}

export async function generateWithStreaming(
  module: Module,
  batchNumber: number,
  contentMode: 'standard' | 'experimental',
  userInstructions: string,
  _references: Reference[],
  onChunk: (chunk: string) => void,
  onComplete: (result: GenerationResult) => void,
  targetPages?: number | null,
  learningOutcomes?: string,
  keywords?: string,
  previousContent?: string,
  totalBatches?: number,
  signal?: AbortSignal,
  onQueue?: (position: number, estimatedWait: number) => void,
  modelProvider: 'claude' | 'openai' = 'claude',
  mode: 'generate' | 'revision' = 'generate',
  currentDraft = '',
  documentId = '',
  occupation: Occupation | null = null,
  structureConfig?: StructureConfig,
): Promise<void> {
  let fullContent = '';
  let researchRefs: Reference[] = [];

  const isExperimental = contentMode === 'experimental';
  const isRevision = mode === 'revision';
  // Revision mode never queries Research Hub — the draft already has refs.
  const useResearchHub = !isExperimental && !isRevision;
  const instructions = userInstructions || '';

  await generateEducationalContentStream(
    {
      number: module.number,
      title: module.title,
      hours: module.hours,
      content: module.content || '',
      activities: module.activities || '',
      skills: module.skills.map(s => ({
        code: s.code,
        name: s.name,
        type: s.type,
      })),
    },
    (text) => {
      fullContent += text;
      onChunk(text);
    },
    (refs) => {
      researchRefs = refs.map(ref => ({
        title: ref.title,
        authors: ref.authors,
        year: ref.year,
        journal: ref.journal,
        doi: ref.doi,
        status: 'approved' as const,
      }));
    },
    useResearchHub,
    isExperimental,
    instructions,
    targetPages,
    learningOutcomes,
    keywords,
    previousContent,
    batchNumber,
    totalBatches || 1,
    signal,
    onQueue,
    modelProvider,
    mode,
    currentDraft,
    documentId,
    occupation,
    structureConfig,
  );

  // Check if bibliography section exists in the generated content
  const hasBibliography = /##?\s*Βιβλιογραφία/i.test(fullContent);

  // If no bibliography and no structured refs, do a follow-up call to generate it.
  // Skip in revision mode: the draft already had a bibliography that the model preserves.
  if (!isRevision && !hasBibliography && researchRefs.length === 0) {
    try {
      const citations = extractCitationStrings(fullContent);
      if (citations.length > 0) {
        const biblioText = await generateBibliography(citations, module.title, modelProvider, documentId);
        if (biblioText.trim()) {
          fullContent += '\n\n## Βιβλιογραφία\n\n' + biblioText.trim();
          onChunk('\n\n## Βιβλιογραφία\n\n' + biblioText.trim());
        }
      }
    } catch (err) {
      console.error('Follow-up bibliography generation failed:', err);
    }
  }

  const pageCount = Math.ceil(fullContent.length / 3000);

  onComplete({
    content: fullContent,
    references: researchRefs.length > 0 ? researchRefs : extractReferencesFromContent(fullContent),
    pageCount,
  });
}

export async function generateSkillCoverageReview(
  module: Module,
  approvedContent: string,
  onProgress?: (status: string) => void,
  modelProvider: 'claude' | 'openai' = 'claude',
  documentId = '',
  occupation: Occupation | null = null,
): Promise<SkillCoverageReview> {
  onProgress?.('Ανάλυση κάλυψης δεξιοτήτων...');

  try {
    const review = await reviewSkillCoverage(
      {
        number: module.number,
        title: module.title,
        hours: module.hours,
        skills: module.skills.map(s => ({ code: s.code, name: s.name, type: s.type })),
      },
      approvedContent,
      modelProvider,
      documentId,
      occupation,
    );

    onProgress?.('Επεξεργασία αποτελεσμάτων...');

    return {
      ...review,
      generatedAt: new Date().toISOString(),
    };
  } catch (error) {
    console.error('Skill coverage review failed:', error);
    return {
      moduleNumber: module.number,
      generatedAt: new Date().toISOString(),
      totalSkills: module.skills.length,
      coveredFully: 0,
      coveredPartially: module.skills.length,
      missing: 0,
      coveragePercentage: 50,
      skillAnalysis: module.skills.map(skill => ({
        skillCode: skill.code,
        skillName: skill.name,
        skillType: skill.type,
        coverageLevel: 'partial' as const,
        evidence: [],
        contentSections: [],
        notes: 'Δεν ήταν δυνατή η αυτόματη ανάλυση. Παρακαλώ ελέγξτε χειροκίνητα.',
      })),
      overallAssessment: 'Η αυτόματη ανάλυση δεν ολοκληρώθηκε επιτυχώς. Παρακαλώ επαναλάβετε ή ελέγξτε χειροκίνητα.',
      recommendations: ['Επαναλάβετε την ανάλυση κάλυψης'],
    };
  }
}
