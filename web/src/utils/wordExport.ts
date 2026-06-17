import {
  Document,
  Paragraph,
  TextRun,
  HeadingLevel,
  AlignmentType,
  PageBreak,
  TableOfContents,
  Packer,
  convertInchesToTwip,
  Header,
  Footer,
  PageNumber,
  Table,
  TableRow,
  TableCell,
  WidthType,
  BorderStyle,
} from 'docx';
import { saveAs } from 'file-saver';
import type { GeneratedContent, Module, Reference, SkillCoverageReview } from '../store/useStore';

interface ExportOptions {
  title: string;
  module: Module;
  batches: GeneratedContent[];
  includeTableOfContents: boolean;
  includePageNumbers: boolean;
  skillReview?: SkillCoverageReview;
  includePendingNote?: boolean;
  finalExport?: boolean; // consolidate MCQs, bibliography, glossary across batches
  summary?: string; // Περίληψη text for final export
}

export async function exportToWord(options: ExportOptions): Promise<void> {
  const { title, module, batches, includeTableOfContents, includePageNumbers } = options;

  const children: (Paragraph | TableOfContents)[] = [];

  // Title page with APOPSI e-learning branding
  children.push(
    // Brand logo/name at top
    new Paragraph({
      children: [
        new TextRun({
          text: 'APOPSI e-learning',
          size: 28,
          bold: true,
          color: '4F46E5',
        }),
      ],
      alignment: AlignmentType.CENTER,
      spacing: { after: 600 },
    }),
    // Decorative line
    new Paragraph({
      children: [
        new TextRun({
          text: '\u2014'.repeat(30),
          color: '10B981',
          size: 20,
        }),
      ],
      alignment: AlignmentType.CENTER,
      spacing: { after: 400 },
    }),
    // Main title
    new Paragraph({
      children: [
        new TextRun({
          text: 'ΕΚΠΑΙΔΕΥΤΙΚΟ ΥΛΙΚΟ',
          size: 36,
          bold: true,
          color: '1a1f35',
        }),
      ],
      alignment: AlignmentType.CENTER,
      spacing: { after: 200 },
    }),
    new Paragraph({
      text: title,
      heading: HeadingLevel.TITLE,
      alignment: AlignmentType.CENTER,
      spacing: { after: 400 },
    }),
    // Decorative line
    new Paragraph({
      children: [
        new TextRun({
          text: '\u2014'.repeat(30),
          color: '10B981',
          size: 20,
        }),
      ],
      alignment: AlignmentType.CENTER,
      spacing: { after: 400 },
    }),
    // Module info
    new Paragraph({
      children: [
        new TextRun({
          text: `Ενότητα ${module.number}`,
          size: 28,
          bold: true,
          color: '4F46E5',
        }),
      ],
      alignment: AlignmentType.CENTER,
      spacing: { after: 100 },
    }),
    new Paragraph({
      children: [
        new TextRun({
          text: module.title,
          size: 32,
          bold: true,
        }),
      ],
      alignment: AlignmentType.CENTER,
      spacing: { after: 200 },
    }),
    new Paragraph({
      children: [
        new TextRun({
          text: `Διάρκεια: ${module.hours} ώρες`,
          size: 24,
          italics: true,
          color: '666666',
        }),
      ],
      alignment: AlignmentType.CENTER,
      spacing: { after: 600 },
    }),
    // Copyright notice at bottom of title page
    new Paragraph({
      children: [
        new TextRun({
          text: '© APOPSI e-learning',
          size: 22,
          color: '4F46E5',
          bold: true,
        }),
      ],
      alignment: AlignmentType.CENTER,
      spacing: { before: 400, after: 100 },
    }),
    new Paragraph({
      children: [
        new TextRun({
          text: 'Με επιφύλαξη παντός δικαιώματος',
          size: 18,
          italics: true,
          color: '999999',
        }),
      ],
      alignment: AlignmentType.CENTER,
      spacing: { after: 200 },
    }),
    new Paragraph({
      children: [new PageBreak()],
    })
  );

  // Table of contents
  if (includeTableOfContents) {
    children.push(
      new TableOfContents('Πίνακας Περιεχομένων', {
        hyperlink: true,
        headingStyleRange: '1-3',
      }),
      new Paragraph({
        children: [new PageBreak()],
      })
    );
  }

  if (options.finalExport) {
    // ===== FINAL EXPORT: consolidate MCQs, bibliography, glossary =====
    const allMcqs: string[] = [];
    const allAnswers: string[] = [];
    const glossaryLines: string[] = [];
    const inlineBibliographies: string[] = [];

    for (const batch of batches) {
      const sections = extractSections(batch.content);
      if (sections.glossary) glossaryLines.push(sections.glossary);
      if (sections.bibliography) inlineBibliographies.push(sections.bibliography);
      if (sections.mcqs) allMcqs.push(sections.mcqs);
      if (sections.mcqAnswers) allAnswers.push(sections.mcqAnswers);

      const contentParagraphs = parseMarkdownToDocx(sections.cleanedContent);
      children.push(...contentParagraphs);

      if (batch !== batches[batches.length - 1]) {
        children.push(new Paragraph({ children: [new PageBreak()] }));
      }
    }

    // Summary (Περίληψη Ενότητας) — last subsection before MCQs
    if (options.summary?.trim()) {
      children.push(
        new Paragraph({ children: [new PageBreak()] }),
        new Paragraph({
          text: 'Περίληψη Ενότητας',
          heading: HeadingLevel.HEADING_1,
          spacing: { after: 200 },
        })
      );
      children.push(...parseMarkdownToDocx(options.summary));
    }

    // Consolidated MCQs (renumbered)
    const { questions, answers } = renumberMcqs(allMcqs, allAnswers);
    if (questions.length > 0) {
      children.push(
        new Paragraph({ children: [new PageBreak()] }),
        new Paragraph({
          text: 'Ερωτήσεις Αυτοαξιολόγησης',
          heading: HeadingLevel.HEADING_1,
          spacing: { after: 200 },
        })
      );
      children.push(...parseMarkdownToDocx(questions.join('\n\n')));

      // Answers
      children.push(
        new Paragraph({
          text: 'Απαντήσεις Αυτοαξιολόγησης',
          heading: HeadingLevel.HEADING_2,
          spacing: { before: 400, after: 200 },
        })
      );
      for (const answer of answers) {
        children.push(
          new Paragraph({
            children: [new TextRun({ text: answer, size: 24 })],
            spacing: { after: 40 },
          })
        );
      }
    }

    // Consolidated bibliography
    const allReferences = batches.flatMap(b => b.references);
    const uniqueRefs = deduplicateReferences(allReferences);
    const allContent = batches.map(b => b.content).join('\n');
    const allInTextCitations = extractInTextCitations(allContent);

    // Helper: normalize author for matching — extract surname only
    // "Chaffey, D." → "chaffey", "Chaffey" → "chaffey"
    const normalizeAuthor = (author: string) => {
      const surname = author.includes(',') ? author.split(',')[0] : author;
      return surname.toLowerCase().normalize('NFD').replace(/[\u0300-\u036f]/g, '');
    };

    // Track which in-text citations are covered by bibliography entries
    const coveredKeys = new Set<string>();
    let biblioHeaderAdded = false;

    if (inlineBibliographies.length > 0) {
      // Strategy 1 (preferred): LLM-generated bibliography — already correct APA with italics
      const dedupedBiblio = deduplicateInlineBibliography(inlineBibliographies);
      children.push(
        new Paragraph({ children: [new PageBreak()] }),
        new Paragraph({ text: 'Βιβλιογραφία', heading: HeadingLevel.HEADING_1, spacing: { after: 200 } })
      );
      biblioHeaderAdded = true;

      // Render line-by-line with parseInlineFormatting (supports *italic*) + hanging indent
      const biblioLines = dedupedBiblio.split('\n').map(l => l.trim()).filter(l => l.length > 0);
      for (const line of biblioLines) {
        children.push(
          new Paragraph({
            children: parseInlineFormatting(line, 22),
            spacing: { after: 120 },
            indent: { left: convertInchesToTwip(0.5), hanging: convertInchesToTwip(0.5) },
          })
        );
      }

      // Track covered citations by checking if bibliography text contains author+year
      const biblioLower = dedupedBiblio.toLowerCase()
        .normalize('NFD').replace(/[\u0300-\u036f]/g, '');
      for (const cit of allInTextCitations) {
        const citAuthor = normalizeAuthor(cit.authors?.[0] || '');
        if (citAuthor && biblioLower.includes(citAuthor) && biblioLower.includes(String(cit.year))) {
          coveredKeys.add(citAuthor + '-' + (cit.year || 0));
        }
      }
    } else if (uniqueRefs.length > 0) {
      // Strategy 2 (fallback): Structured refs — filter to only cited ones
      const citedRefs = uniqueRefs.filter(ref => {
        const refAuthor = normalizeAuthor(ref.authors?.[0] || '');
        const refYear = ref.year || 0;
        return allInTextCitations.some(c =>
          normalizeAuthor(c.authors?.[0] || '') === refAuthor && c.year === refYear
        );
      });

      // Fall back to all refs if no matches (avoids empty bibliography)
      const finalRefs = citedRefs.length > 0 ? citedRefs : uniqueRefs;

      children.push(
        new Paragraph({ children: [new PageBreak()] }),
        new Paragraph({ text: 'Βιβλιογραφία', heading: HeadingLevel.HEADING_1, spacing: { after: 200 } })
      );
      biblioHeaderAdded = true;

      for (const ref of finalRefs) {
        const key = normalizeAuthor(ref.authors?.[0] || '') + '-' + (ref.year || 0);
        coveredKeys.add(key);
        children.push(
          new Paragraph({
            children: parseInlineFormatting(formatReferenceAPA(ref), 22),
            spacing: { after: 120 },
            indent: { left: convertInchesToTwip(0.5), hanging: convertInchesToTwip(0.5) },
          })
        );
      }
    } else if (allInTextCitations.length > 0) {
      // Strategy 3: citation-only entries — all covered by definition
      children.push(
        new Paragraph({ children: [new PageBreak()] }),
        new Paragraph({ text: 'Βιβλιογραφία', heading: HeadingLevel.HEADING_1, spacing: { after: 200 } })
      );
      biblioHeaderAdded = true;
      for (const ref of allInTextCitations) {
        coveredKeys.add(normalizeAuthor(ref.authors?.[0] || '') + '-' + (ref.year || 0));
        children.push(
          new Paragraph({
            children: [new TextRun({ text: formatCitationOnlyAPA(ref), size: 22 })],
            spacing: { after: 120 },
            indent: { left: convertInchesToTwip(0.5), hanging: convertInchesToTwip(0.5) },
          })
        );
      }
    }

    // Safety net: add orphan in-text citations not covered by bibliography
    const orphanCitations = allInTextCitations.filter(c => {
      const key = normalizeAuthor(c.authors?.[0] || '') + '-' + (c.year || 0);
      return !coveredKeys.has(key);
    });

    if (orphanCitations.length > 0) {
      if (!biblioHeaderAdded) {
        children.push(
          new Paragraph({ children: [new PageBreak()] }),
          new Paragraph({ text: 'Βιβλιογραφία', heading: HeadingLevel.HEADING_1, spacing: { after: 200 } })
        );
      }
      for (const ref of orphanCitations) {
        children.push(
          new Paragraph({
            children: [new TextRun({ text: formatCitationOnlyAPA(ref), size: 22 })],
            spacing: { after: 120 },
            indent: { left: convertInchesToTwip(0.5), hanging: convertInchesToTwip(0.5) },
          })
        );
      }
    }

    // Consolidated glossary
    if (glossaryLines.length > 0) {
      children.push(
        new Paragraph({ children: [new PageBreak()] }),
        new Paragraph({ text: 'Γλωσσάρι', heading: HeadingLevel.HEADING_1, spacing: { after: 200 } })
      );
      // Merge and sort glossary entries alphabetically
      const mergedGlossary = mergeGlossaryEntries(glossaryLines);
      children.push(...parseMarkdownToDocx(mergedGlossary));
    }
  } else {
    // ===== REGULAR EXPORT: keep per-batch sections inline =====
    for (const batch of batches) {
      if (options.includePendingNote && batch.status !== 'approved') {
        children.push(
          new Paragraph({
            children: [
              new TextRun({
                text: '⚠ ΑΝΑΜΕΝΕΤΑΙ ΕΓΚΡΙΣΗ — Το παρακάτω περιεχόμενο δεν έχει εγκριθεί ακόμα.',
                bold: true,
                color: 'D97706',
                size: 24,
              }),
            ],
            shading: { fill: 'FEF3C7' },
            spacing: { before: 200, after: 200 },
          })
        );
      }

      // Render content as-is (MCQs, glossary stay inline per batch)
      const contentParagraphs = parseMarkdownToDocx(batch.content);
      children.push(...contentParagraphs);

      // Append bibliography from structured references (don't rely on LLM)
      const hasBiblioInContent = /##?\s*Βιβλιογραφία/i.test(batch.content);
      const batchRefs = deduplicateReferences(batch.references);
      if (!hasBiblioInContent) {
        if (batchRefs.length > 0) {
          // Use structured refs from Research Hub
          children.push(
            new Paragraph({ text: 'Βιβλιογραφία', heading: HeadingLevel.HEADING_2, spacing: { before: 400, after: 200 } })
          );
          for (const ref of batchRefs) {
            children.push(
              new Paragraph({
                children: [new TextRun({ text: formatReferenceAPA(ref), size: 22 })],
                spacing: { after: 120 },
                indent: { left: convertInchesToTwip(0.5), hanging: convertInchesToTwip(0.5) },
              })
            );
          }
        } else {
          // Fallback: extract in-text citations from content (experimental mode)
          const citationRefs = extractInTextCitations(batch.content);
          if (citationRefs.length > 0) {
            children.push(
              new Paragraph({ text: 'Βιβλιογραφία', heading: HeadingLevel.HEADING_2, spacing: { before: 400, after: 200 } })
            );
            for (const ref of citationRefs) {
              children.push(
                new Paragraph({
                  children: [new TextRun({ text: formatCitationOnlyAPA(ref), size: 22 })],
                  spacing: { after: 120 },
                  indent: { left: convertInchesToTwip(0.5), hanging: convertInchesToTwip(0.5) },
                })
              );
            }
          }
        }
      }

      if (batch !== batches[batches.length - 1]) {
        children.push(new Paragraph({ children: [new PageBreak()] }));
      }
    }
  }

  // ESCO Skills Coverage Report
  if (options.skillReview) {
    const review = options.skillReview;
    children.push(
      new Paragraph({
        children: [new PageBreak()],
      }),
      new Paragraph({
        text: 'Αναφορά Κάλυψης Δεξιοτήτων ESCO',
        heading: HeadingLevel.HEADING_1,
        spacing: { after: 300 },
      }),
      // Summary stats
      new Paragraph({
        children: [
          new TextRun({
            text: `Συνολική Κάλυψη: ${review.coveragePercentage}%`,
            size: 28,
            bold: true,
            color: '4F46E5',
          }),
        ],
        spacing: { after: 200 },
      }),
      new Paragraph({
        children: [
          new TextRun({ text: `Πλήρης κάλυψη: ${review.coveredFully} δεξιότητες`, size: 22 }),
          new TextRun({ text: '  |  ', size: 22, color: 'CCCCCC' }),
          new TextRun({ text: `Μερική κάλυψη: ${review.coveredPartially} δεξιότητες`, size: 22 }),
          new TextRun({ text: '  |  ', size: 22, color: 'CCCCCC' }),
          new TextRun({ text: `Απούσα: ${review.missing} δεξιότητες`, size: 22 }),
        ],
        spacing: { after: 400 },
      })
    );

    // Skills table
    const tableRows: TableRow[] = [
      // Header row
      new TableRow({
        children: [
          new TableCell({
            children: [new Paragraph({ children: [new TextRun({ text: 'Δεξιότητα', bold: true, size: 22 })] })],
            width: { size: 40, type: WidthType.PERCENTAGE },
            shading: { fill: 'F3F4F6' },
          }),
          new TableCell({
            children: [new Paragraph({ children: [new TextRun({ text: 'Τύπος', bold: true, size: 22 })] })],
            width: { size: 15, type: WidthType.PERCENTAGE },
            shading: { fill: 'F3F4F6' },
          }),
          new TableCell({
            children: [new Paragraph({ children: [new TextRun({ text: 'Κάλυψη', bold: true, size: 22 })] })],
            width: { size: 15, type: WidthType.PERCENTAGE },
            shading: { fill: 'F3F4F6' },
          }),
          new TableCell({
            children: [new Paragraph({ children: [new TextRun({ text: 'Σημειώσεις', bold: true, size: 22 })] })],
            width: { size: 30, type: WidthType.PERCENTAGE },
            shading: { fill: 'F3F4F6' },
          }),
        ],
      }),
    ];

    // Data rows
    for (const skill of review.skillAnalysis) {
      const coverageText = skill.coverageLevel === 'full' ? 'Πλήρης' :
                          skill.coverageLevel === 'partial' ? 'Μερική' : 'Απούσα';
      const coverageColor = skill.coverageLevel === 'full' ? '059669' :
                           skill.coverageLevel === 'partial' ? 'D97706' : 'DC2626';

      tableRows.push(
        new TableRow({
          children: [
            new TableCell({
              children: [new Paragraph({ children: [new TextRun({ text: skill.skillName, size: 20 })] })],
            }),
            new TableCell({
              children: [new Paragraph({ children: [new TextRun({
                text: skill.skillType === 'essential' ? 'Απαραίτητη' : 'Προαιρετική',
                size: 20,
                italics: true,
              })] })],
            }),
            new TableCell({
              children: [new Paragraph({ children: [new TextRun({
                text: coverageText,
                size: 20,
                bold: true,
                color: coverageColor,
              })] })],
            }),
            new TableCell({
              children: [new Paragraph({ children: [new TextRun({ text: skill.notes || '-', size: 20 })] })],
            }),
          ],
        })
      );
    }

    children.push(
      new Table({
        rows: tableRows,
        width: { size: 100, type: WidthType.PERCENTAGE },
        borders: {
          top: { style: BorderStyle.SINGLE, size: 1, color: 'E5E7EB' },
          bottom: { style: BorderStyle.SINGLE, size: 1, color: 'E5E7EB' },
          left: { style: BorderStyle.SINGLE, size: 1, color: 'E5E7EB' },
          right: { style: BorderStyle.SINGLE, size: 1, color: 'E5E7EB' },
          insideHorizontal: { style: BorderStyle.SINGLE, size: 1, color: 'E5E7EB' },
          insideVertical: { style: BorderStyle.SINGLE, size: 1, color: 'E5E7EB' },
        },
      })
    );

    // Overall assessment
    if (review.overallAssessment) {
      children.push(
        new Paragraph({
          text: 'Συνολική Αξιολόγηση',
          heading: HeadingLevel.HEADING_2,
          spacing: { before: 400, after: 200 },
        }),
        new Paragraph({
          children: [new TextRun({ text: review.overallAssessment, size: 22 })],
          spacing: { after: 200 },
        })
      );
    }

    // Recommendations
    if (review.recommendations.length > 0) {
      children.push(
        new Paragraph({
          text: 'Προτάσεις Βελτίωσης',
          heading: HeadingLevel.HEADING_2,
          spacing: { before: 300, after: 200 },
        })
      );

      for (const rec of review.recommendations) {
        children.push(
          new Paragraph({
            children: [new TextRun({ text: rec, size: 22 })],
            bullet: { level: 0 },
            spacing: { after: 100 },
          })
        );
      }
    }
  }

  // Create document with headers and footers
  const doc = new Document({
    numbering: {
      config: [
        {
          reference: 'default-numbering',
          levels: [
            {
              level: 0,
              format: 'decimal',
              text: '%1.',
              alignment: AlignmentType.START,
              style: {
                paragraph: {
                  indent: { left: convertInchesToTwip(0.5), hanging: convertInchesToTwip(0.25) },
                },
              },
            },
          ],
        },
      ],
    },
    styles: {
      default: {
        document: {
          run: {
            font: 'Calibri',
            size: 24,
          },
          paragraph: {
            spacing: { line: 360 },
          },
        },
      },
      paragraphStyles: [
        {
          id: 'Title',
          name: 'Title',
          basedOn: 'Normal',
          next: 'Normal',
          run: {
            font: 'Georgia',
            size: 56,
            bold: true,
            color: '1a1f35',
          },
        },
        {
          id: 'Heading1',
          name: 'Heading 1',
          basedOn: 'Normal',
          next: 'Normal',
          run: {
            font: 'Georgia',
            size: 36,
            bold: true,
            color: '1a1f35',
          },
          paragraph: {
            spacing: { before: 400, after: 200 },
          },
        },
        {
          id: 'Heading2',
          name: 'Heading 2',
          basedOn: 'Normal',
          next: 'Normal',
          run: {
            font: 'Georgia',
            size: 30,
            bold: true,
            color: '2a3050',
          },
          paragraph: {
            spacing: { before: 300, after: 150 },
          },
        },
        {
          id: 'Heading3',
          name: 'Heading 3',
          basedOn: 'Normal',
          next: 'Normal',
          run: {
            font: 'Georgia',
            size: 26,
            bold: true,
            color: '4a4a4a',
          },
          paragraph: {
            spacing: { before: 200, after: 100 },
          },
        },
      ],
    },
    sections: [
      {
        properties: {
          page: {
            margin: {
              top: convertInchesToTwip(1),
              right: convertInchesToTwip(1),
              bottom: convertInchesToTwip(1),
              left: convertInchesToTwip(1.25),
            },
          },
        },
        headers: {
          default: new Header({
            children: [
              new Paragraph({
                children: [
                  new TextRun({
                    text: 'APOPSI e-learning',
                    size: 18,
                    color: '4F46E5',
                    bold: true,
                  }),
                  new TextRun({
                    text: `  |  ${title} - Ενότητα ${module.number}`,
                    size: 18,
                    color: '999999',
                  }),
                ],
                alignment: AlignmentType.RIGHT,
              }),
            ],
          }),
        },
        footers: {
          default: new Footer({
            children: [
              // Copyright watermark line
              new Paragraph({
                children: [
                  new TextRun({
                    text: '© APOPSI e-learning',
                    size: 16,
                    color: '4F46E5',
                    bold: true,
                  }),
                  new TextRun({
                    text: '  •  ',
                    size: 16,
                    color: 'CCCCCC',
                  }),
                  new TextRun({
                    text: 'Εκπαιδευτικό Υλικό',
                    size: 16,
                    color: '999999',
                    italics: true,
                  }),
                  ...(includePageNumbers
                    ? [
                        new TextRun({
                          text: '  •  ',
                          size: 16,
                          color: 'CCCCCC',
                        }),
                        new TextRun({
                          text: 'Σελίδα ',
                          size: 16,
                          color: '666666',
                        }),
                        new TextRun({
                          children: [PageNumber.CURRENT],
                          size: 16,
                          color: '666666',
                        }),
                        new TextRun({
                          text: '/',
                          size: 16,
                          color: '666666',
                        }),
                        new TextRun({
                          children: [PageNumber.TOTAL_PAGES],
                          size: 16,
                          color: '666666',
                        }),
                      ]
                    : []),
                ],
                alignment: AlignmentType.CENTER,
              }),
            ],
          }),
        },
        children,
      },
    ],
  });

  // Generate and save
  const blob = await Packer.toBlob(doc);
  const dateStr = new Date().toISOString().slice(0, 10);
  const filename = `${module.title.replace(/[^a-zA-Zα-ωΑ-Ωά-ώ0-9]/g, '_')}_Ενότητα_${module.number}_${dateStr}.docx`;
  saveAs(blob, filename);
}

/**
 * Extract special sections from markdown content: bibliography, glossary, MCQs, answers.
 * Returns cleaned content (main text only) and each extracted section separately.
 */
function extractSections(content: string): {
  cleanedContent: string;
  glossary: string;
  bibliography: string;
  mcqs: string;
  mcqAnswers: string;
} {
  const lines = content.split('\n');
  const cleanedLines: string[] = [];
  const glossaryLines: string[] = [];
  const bibliographyLines: string[] = [];
  const mcqLines: string[] = [];
  const mcqAnswerLines: string[] = [];
  let currentSection: 'content' | 'bibliography' | 'glossary' | 'mcqs' | 'mcqAnswers' = 'content';

  for (const line of lines) {
    const trimmed = line.trim().toLowerCase();
    // Strip diacritical marks for accent-insensitive Greek matching
    const normalized = trimmed.normalize('NFD').replace(/[\u0300-\u036f]/g, '');

    if (/^#{1,3}\s+βιβλιογραφια/.test(normalized)) {
      currentSection = 'bibliography';
      continue;
    }
    if (/^#{1,3}\s+γλωσσαρι/.test(normalized)) {
      currentSection = 'glossary';
      continue;
    }
    if (/^#{1,3}\s+ερωτησεις\s+αυτοαξιολογησης/.test(normalized)) {
      currentSection = 'mcqs';
      continue;
    }
    if (/^#{1,3}\s+απαντησεις\s+αυτοαξιολογησης/.test(normalized)) {
      currentSection = 'mcqAnswers';
      continue;
    }

    // A new heading (not one of the special sections) resets to content
    if (/^#{1,3}\s+/.test(normalized) && currentSection !== 'content') {
      currentSection = 'content';
    }

    switch (currentSection) {
      case 'content': cleanedLines.push(line); break;
      case 'glossary': glossaryLines.push(line); break;
      case 'bibliography': bibliographyLines.push(line); break;
      case 'mcqs': mcqLines.push(line); break;
      case 'mcqAnswers': mcqAnswerLines.push(line); break;
    }
  }

  return {
    cleanedContent: cleanedLines.join('\n'),
    glossary: glossaryLines.join('\n').trim(),
    bibliography: bibliographyLines.join('\n').trim(),
    mcqs: mcqLines.join('\n').trim(),
    mcqAnswers: mcqAnswerLines.join('\n').trim(),
  };
}

/**
 * Renumber MCQs from multiple batches into a single sequential list.
 * Returns arrays of question strings and answer strings with new numbering.
 */
function renumberMcqs(mcqBlocks: string[], answerBlocks: string[]): { questions: string[]; answers: string[] } {
  const questions: string[] = [];
  const answers: string[] = [];
  let globalNum = 1;

  for (let i = 0; i < mcqBlocks.length; i++) {
    const block = mcqBlocks[i];
    if (!block) continue;

    // Try splitting by numbered questions first
    let qParts = block.split(/\n(?=\d+\.\s)/);

    // Fallback: if no numbered splits found, split by question boundaries
    // A question boundary = a non-empty line that is NOT an answer option (α/β/γ/δ)
    if (qParts.length <= 1 && block.includes('α)') && block.includes('β)')) {
      qParts = [];
      let currentQ = '';
      for (const line of block.split('\n')) {
        const trimmed = line.trim();
        if (!trimmed) continue;
        // Check if this line is an answer option
        const isOption = /^[αβγδa-d][)\.]\s/i.test(trimmed);
        if (!isOption && currentQ && /[αβγδ][)\.]/i.test(currentQ)) {
          // Previous content has answer options → this non-option line starts a new question
          qParts.push(currentQ);
          currentQ = trimmed;
        } else {
          currentQ += (currentQ ? '\n' : '') + trimmed;
        }
      }
      if (currentQ) qParts.push(currentQ);
    }

    const ansBlock = answerBlocks[i] || '';

    // Parse original answers for this batch (e.g., "1. α, 2. β, 3. γ")
    const ansMap = new Map<number, string>();
    const ansMatches = ansBlock.matchAll(/(\d+)\.\s*([αβγδa-d])/gi);
    for (const m of ansMatches) {
      ansMap.set(parseInt(m[1]), m[2]);
    }

    let localNum = 1;
    for (const qPart of qParts) {
      const trimmed = qPart.trim();
      if (!trimmed) continue;
      // Replace or add number
      const renumbered = /^\d+\./.test(trimmed)
        ? trimmed.replace(/^\d+\./, `${globalNum}.`)
        : `${globalNum}. ${trimmed}`;
      questions.push(renumbered);
      // Map the answer
      const originalAnswer = ansMap.get(localNum);
      if (originalAnswer) {
        answers.push(`${globalNum}. ${originalAnswer}`);
      }
      globalNum++;
      localNum++;
    }
  }

  // Cap MCQs to 20 total; ensure answers array matches questions length
  if (questions.length > 20) {
    questions.length = 20;
  }
  answers.length = Math.min(answers.length, questions.length);

  return { questions, answers };
}

/**
 * Merge glossary entries from multiple batches, sort alphabetically, deduplicate.
 */
function mergeGlossaryEntries(glossaryBlocks: string[]): string {
  const entries = new Map<string, string>(); // term -> definition

  for (const block of glossaryBlocks) {
    const lines = block.split('\n');
    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed) continue;
      // Try to parse "**Term**: definition" or "Term: definition" or "- Term: definition"
      const match = trimmed.match(/^[-*]*\s*\*?\*?([^:*]+)\*?\*?\s*[:\-–]\s*(.+)/);
      if (match) {
        const term = match[1].trim();
        const definition = match[2].trim();
        // Keep first occurrence (or could merge)
        if (!entries.has(term.toLowerCase())) {
          entries.set(term.toLowerCase(), `**${term}**: ${definition}`);
        }
      } else {
        // Fallback: just add the line
        entries.set(trimmed.toLowerCase(), trimmed);
      }
    }
  }

  // Sort alphabetically by term
  const sorted = [...entries.values()].sort((a, b) => a.localeCompare(b, 'el'));
  return sorted.join('\n\n');
}

function parseMarkdownToDocx(markdown: string): Paragraph[] {
  const paragraphs: Paragraph[] = [];
  const lines = markdown.split('\n');

  let inCodeBlock = false;
  let codeContent = '';

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // Code block handling
    if (line.startsWith('```')) {
      if (inCodeBlock) {
        paragraphs.push(
          new Paragraph({
            children: [
              new TextRun({
                text: sanitizeText(codeContent.trim()),
                font: 'Courier New',
                size: 20,
              }),
            ],
            shading: { fill: 'f5f5f5' },
            spacing: { before: 100, after: 100 },
          })
        );
        codeContent = '';
        inCodeBlock = false;
      } else {
        inCodeBlock = true;
      }
      continue;
    }

    if (inCodeBlock) {
      codeContent += line + '\n';
      continue;
    }

    // Skip empty lines
    if (!line.trim()) {
      continue;
    }

    // Headings
    if (line.startsWith('### ')) {
      paragraphs.push(
        new Paragraph({
          text: sanitizeText(line.substring(4)),
          heading: HeadingLevel.HEADING_3,
        })
      );
    } else if (line.startsWith('## ')) {
      paragraphs.push(
        new Paragraph({
          text: sanitizeText(line.substring(3)),
          heading: HeadingLevel.HEADING_2,
        })
      );
    } else if (line.startsWith('# ')) {
      paragraphs.push(
        new Paragraph({
          text: sanitizeText(line.substring(2)),
          heading: HeadingLevel.HEADING_1,
        })
      );
    }
    // Bullet items (activity instructions •)
    else if (line.trimStart().startsWith('• ')) {
      paragraphs.push(
        new Paragraph({
          children: parseInlineFormatting(line.trimStart()),
          indent: { left: convertInchesToTwip(0.25) },
          spacing: { after: 60 },
        })
      );
    }
    // Bullet lists
    else if (line.startsWith('- ') || line.startsWith('* ')) {
      paragraphs.push(
        new Paragraph({
          children: parseInlineFormatting(line.substring(2)),
          bullet: { level: 0 },
        })
      );
    }
    // Numbered lists
    else if (/^\d+\.\s/.test(line)) {
      const content = line.replace(/^\d+\.\s/, '');
      paragraphs.push(
        new Paragraph({
          children: parseInlineFormatting(content),
          numbering: { reference: 'default-numbering', level: 0 },
        })
      );
    }
    // Regular paragraphs
    else {
      paragraphs.push(
        new Paragraph({
          children: parseInlineFormatting(line),
          spacing: { after: 120 },
        })
      );
    }
  }

  return paragraphs;
}

/**
 * Remove XML-invalid control characters that can corrupt .docx files.
 * Keeps \t (0x09), \n (0x0A), \r (0x0D) which are valid in XML.
 */
function sanitizeText(text: string): string {
  // eslint-disable-next-line no-control-regex
  return text.replace(/[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/g, '');
}

function parseInlineFormatting(text: string, size?: number): TextRun[] {
  const runs: TextRun[] = [];

  // Simple regex-based parsing for bold and italic
  const sanitized = sanitizeText(text);
  const parts = sanitized.split(/(\*\*[^*]+\*\*|\*[^*]+\*|`[^`]+`)/g);

  for (const part of parts) {
    if (!part) continue;

    if (part.startsWith('**') && part.endsWith('**')) {
      runs.push(
        new TextRun({
          text: part.slice(2, -2),
          bold: true,
          ...(size && { size }),
        })
      );
    } else if (part.startsWith('*') && part.endsWith('*')) {
      runs.push(
        new TextRun({
          text: part.slice(1, -1),
          italics: true,
          ...(size && { size }),
        })
      );
    } else if (part.startsWith('`') && part.endsWith('`')) {
      runs.push(
        new TextRun({
          text: part.slice(1, -1),
          font: 'Courier New',
          ...(size && { size }),
        })
      );
    } else {
      runs.push(new TextRun({ text: part, ...(size && { size }) }));
    }
  }

  return runs;
}

/**
 * Deduplicate inline (LLM-generated) bibliography entries across batches.
 * Each entry is a text line in APA format. Dedup by normalized text key.
 */
function deduplicateInlineBibliography(bibliographies: string[]): string {
  const allText = bibliographies.join('\n');
  const lines = allText.split('\n').map(l => l.trim()).filter(l => l.length > 0);

  const seen = new Set<string>();
  const unique: string[] = [];

  for (const line of lines) {
    // Normalize: lowercase, strip accents, collapse spaces
    const key = line.toLowerCase()
      .normalize('NFD').replace(/[\u0300-\u036f]/g, '')
      .replace(/\s+/g, ' ');
    if (!seen.has(key)) {
      seen.add(key);
      unique.push(line);
    }
  }

  // Sort alphabetically
  return unique.sort((a, b) => a.localeCompare(b, 'el')).join('\n');
}

function deduplicateReferences(refs: Reference[]): Reference[] {
  const seen = new Set<string>();
  const unique: Reference[] = [];

  for (const ref of refs) {
    if (!ref.title) continue;

    const key = `${ref.title.toLowerCase()}-${ref.year || 'nd'}`;
    if (!seen.has(key)) {
      seen.add(key);
      unique.push(ref);
    }
  }

  // APA 7th: alphabetical by first author surname, then by year ascending
  return unique.sort((a, b) => {
    const authorA = a.authors?.[0] || '';
    const authorB = b.authors?.[0] || '';
    const authorCmp = authorA.localeCompare(authorB, 'el');
    if (authorCmp !== 0) return authorCmp;

    // Same author: sort by year ascending (oldest first, per APA 7th)
    return (a.year || 0) - (b.year || 0);
  });
}

function formatReferenceAPA(ref: Reference): string {
  // Format authors with APA 7th "&" convention for last author
  const authorList = ref.authors || ['Unknown'];
  let authors: string;
  if (authorList.length === 1) {
    authors = authorList[0];
  } else if (authorList.length === 2) {
    authors = `${authorList[0]}, & ${authorList[1]}`;
  } else {
    const allButLast = authorList.slice(0, -1).join(', ');
    authors = `${allButLast}, & ${authorList[authorList.length - 1]}`;
  }

  const year = ref.year ? `(${ref.year})` : '(n.d.)';
  // Strip trailing period from title to avoid double periods
  const title = (ref.title || 'Untitled').replace(/\.\s*$/, '');

  let entry: string;
  if (ref.journal) {
    // Journal article: title NOT italic, journal IS italic (APA 7th)
    const journal = ref.journal.replace(/\.\s*$/, '');
    entry = `${authors} ${year}. ${title}. *${journal}*.`;
  } else {
    // Book/standalone: title IS italic (APA 7th)
    entry = `${authors} ${year}. *${title}*.`;
  }

  // DOI/URL: no trailing period after DOI or URL (APA 7th)
  if (ref.doi) {
    entry += ` https://doi.org/${ref.doi}`;
  } else if (ref.url) {
    entry += ` ${ref.url}`;
  }

  return entry;
}

/**
 * Extract in-text citations like (Author, Year) from content body.
 * Used as fallback when no structured references or bibliography section exist
 * (e.g., experimental mode without Research Hub).
 */
function extractInTextCitations(content: string): Reference[] {
  const citations = new Map<string, Reference>();

  // Match patterns: (Author, 2020), (Author & Author, 2020), (Author κ.ά., 2020), (Author et al., 2020)
  // Supports Greek and Latin characters
  const pattern = /\(([A-ZΑ-Ω\u0386-\u03CE][A-Za-zΑ-Ωα-ωά-ώ\u0386-\u03CE.\-\s]+?)(?:\s*(?:&|και)\s*([A-ZΑ-Ω\u0386-\u03CE][A-Za-zΑ-Ωα-ωά-ώ\u0386-\u03CE.\-\s]+?))?\s*(?:κ\.ά\.|et\s+al\.)?,\s*(\d{4})\)/g;

  let match;
  while ((match = pattern.exec(content)) !== null) {
    const author1 = match[1].trim();
    const author2 = match[2]?.trim();
    const year = parseInt(match[3], 10);

    // Build authors list
    const authors: string[] = [author1];
    if (author2) authors.push(author2);

    // Build a key to deduplicate
    const key = `${author1.toLowerCase()}-${year}`;
    if (!citations.has(key)) {
      citations.set(key, {
        title: '', // We don't know the title from in-text citations
        authors,
        year,
        status: 'auto' as const,
      });
    }
  }

  // APA 7th: alphabetical by first author surname, then year ascending
  return [...citations.values()].sort((a, b) => {
    const authorCmp = (a.authors?.[0] || '').localeCompare(b.authors?.[0] || '', 'el');
    if (authorCmp !== 0) return authorCmp;
    return (a.year || 0) - (b.year || 0);
  });
}

/**
 * Format a citation-only reference (no title) in APA-like style.
 * e.g., "Κωνσταντινίδης, Γ. (2024)."
 */
function formatCitationOnlyAPA(ref: Reference): string {
  if (ref.title) {
    return formatReferenceAPA(ref);
  }
  // Format authors with APA 7th "&" convention
  const authorList = ref.authors || ['Unknown'];
  let authors: string;
  if (authorList.length === 1) {
    authors = authorList[0];
  } else if (authorList.length === 2) {
    authors = `${authorList[0]}, & ${authorList[1]}`;
  } else {
    const allButLast = authorList.slice(0, -1).join(', ');
    authors = `${allButLast}, & ${authorList[authorList.length - 1]}`;
  }
  const year = ref.year ? `(${ref.year})` : '(n.d.)';
  return `${authors} ${year}.`;
}

export async function exportAllModules(
  title: string,
  modules: Module[],
  batchesByModule: Map<number, GeneratedContent[]>
): Promise<void> {
  for (const module of modules) {
    const batches = batchesByModule.get(module.number) || [];
    if (batches.length > 0) {
      await exportToWord({
        title,
        module,
        batches,
        includeTableOfContents: true,
        includePageNumbers: true,
      });
    }
  }
}
