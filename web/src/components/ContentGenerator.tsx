import { useState, useCallback, useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Loader2,
  Check,
  X,
  ChevronRight,
  FileDown,
  RefreshCw,
  Sparkles,
  BookOpen,
  ArrowLeft,
  Target,
  FlaskConical,
  Edit3,
  CheckCircle2,
  RotateCcw,
} from 'lucide-react';
import { useStore } from '../store/useStore';
import type { StructureConfig } from '../store/useStore';

// Optional structural elements the user can toggle off. The quality core and
// the Βιβλιογραφία are always included and intentionally not listed here.
const STRUCTURE_ELEMENTS: { key: keyof StructureConfig; label: string; hint?: string }[] = [
  { key: 'activities', label: 'Δραστηριότητες' },
  { key: 'self_assessment', label: 'Ερωτήσεις αυτοαξιολόγησης' },
  { key: 'glossary', label: 'Γλωσσάρι' },
  { key: 'subsection_keywords', label: 'Βασικές λέξεις ανά υποενότητα' },
  { key: 'in_text_citations', label: 'Ενδοκειμενικές αναφορές', hint: 'Αν απενεργοποιηθούν, οι ισχυρισμοί δεν τεκμηριώνονται ατομικά με (Επώνυμο, Έτος). Η Βιβλιογραφία παραμένει.' },
];
import { generateWithStreaming, generateSkillCoverageReview } from '../services/claudeService';
import { generateSummary } from '../services/api';
import { exportToWord } from '../utils/wordExport';
import { SkillCoverageReview } from './SkillCoverageReview';
import { BatchFeedbackDialog } from './BatchFeedbackDialog';
import styles from './ContentGenerator.module.css';

function ConfirmResetDialog({ isOpen, onConfirm, onCancel }: { isOpen: boolean; onConfirm: () => void; onCancel: () => void }) {
  if (!isOpen) return null;
  return (
    <div className={styles.dialogOverlay} onClick={onCancel}>
      <motion.div
        initial={{ opacity: 0, scale: 0.95 }}
        animate={{ opacity: 1, scale: 1 }}
        className={styles.dialogBox}
        onClick={(e) => e.stopPropagation()}
      >
        <p className={styles.dialogMessage}>Θα χαθεί όλη η τρέχουσα εργασία. Συνέχεια;</p>
        <div className={styles.dialogActions}>
          <button onClick={onCancel} className={styles.dialogCancel}>Ακύρωση</button>
          <button onClick={onConfirm} className={styles.dialogConfirm}>Νέα Εργασία</button>
        </div>
      </motion.div>
    </div>
  );
}

export function ContentGenerator() {
  const {
    workflowMode,
    modules,
    selectedModule,
    contentMode,
    userInstructions,
    totalModulePages,
    targetPages,
    learningOutcomes,
    keywords,
    isEditDoc,
    setTotalModulePages,
    structureConfig,
    setStructureConfig,
    generatedBatches,
    addGeneratedBatch,
    updateBatchStatus,
    updateBatchContent,
    isGenerating,
    setIsGenerating,
    approvedReferences,
    modelProvider,
    setCurrentStep,
    documentTitle,
    skillReviews,
    setSkillReview,
    isReviewingSkills,
    setIsReviewingSkills,
    productionComplete,
    setProductionComplete,
    moduleSummaries,
    setModuleSummary,
    isGeneratingSummary,
    setIsGeneratingSummary,
    queuePosition,
    estimatedWait,
    setQueueStatus,
    clearQueueStatus,
    reset,
    currentTaskId,
  } = useStore();

  const isEscoMode = workflowMode === 'esco';

  const [streamingContent, setStreamingContent] = useState('');
  const [showInstructionInput, setShowInstructionInput] = useState(false);
  const [localInstructions, setLocalInstructions] = useState('');
  const [extendPages, setExtendPages] = useState(20);
  const [reviewProgress, setReviewProgress] = useState('');
  const [showReview, setShowReview] = useState(false);
  const [feedbackDialogBatch, setFeedbackDialogBatch] = useState<number | null>(null);
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);
  const abortControllerRef = useRef<AbortController | null>(null);

  const isModuleComplete = selectedModule ? productionComplete[selectedModule] : false;

  const module = modules.find((m) => m.number === selectedModule);
  const currentReview = selectedModule ? skillReviews[selectedModule] : undefined;

  const moduleBatches = generatedBatches.filter(
    (b) => b.moduleNumber === selectedModule
  );

  const currentPendingBatch = moduleBatches.find(
    (b) => b.status === 'pending'
  );

  // Abort any in-flight generation on unmount
  useEffect(() => {
    return () => {
      abortControllerRef.current?.abort();
    };
  }, []);

  useEffect(() => {
    if (contentRef.current && streamingContent) {
      contentRef.current.scrollTop = contentRef.current.scrollHeight;
    }
  }, [streamingContent]);

  // Collect approved content for anti-overlap
  const getApprovedContent = useCallback(() => {
    return moduleBatches
      .filter((b) => b.status === 'approved')
      .map((b) => b.content)
      .join('\n\n---\n\n');
  }, [moduleBatches]);

  const totalBatches = totalModulePages > 0 && targetPages > 0
    ? Math.ceil(totalModulePages / targetPages)
    : 1;

  const handleGenerate = useCallback(async () => {
    if (!module) return;

    abortControllerRef.current?.abort();
    const controller = new AbortController();
    abortControllerRef.current = controller;

    setIsGenerating(true);
    setStreamingContent('');

    const batchNumber = moduleBatches.length + 1;
    const instructions = localInstructions || userInstructions;
    const previousContent = getApprovedContent();

    // Edit-doc "Επέκταση": the user asks for N more pages on top of the
    // uploaded draft. Use that count directly as this batch's target (the
    // model writes only the new pages — the existing draft stays in
    // previous_content) and grow the page budget so the progress bar stays honest.
    let effectiveTarget: number;
    if (isEditDoc) {
      effectiveTarget = Math.max(1, extendPages);
      setTotalModulePages(totalModulePages + effectiveTarget);
    } else {
      // Calculate remaining pages so the last batch doesn't overshoot
      const approvedPages = moduleBatches
        .filter((b) => b.status === 'approved')
        .reduce((sum, b) => sum + b.pageCount, 0);
      const remainingPages = totalModulePages > 0
        ? Math.max(targetPages > 0 ? Math.min(targetPages, totalModulePages - approvedPages) : totalModulePages - approvedPages, 5)
        : targetPages;
      effectiveTarget = remainingPages > 0 ? remainingPages : targetPages;
    }

    // In edit-doc mode the page budget grows on demand, so totalBatches (derived
    // from the original doc size) is stale — pass the actual batch index instead
    // so the continuation header reads "ΤΜΗΜΑ 2 ΑΠΟ ~2", not "ΑΠΟ ~1".
    const effectiveTotalBatches = isEditDoc ? batchNumber : totalBatches;

    try {
      await generateWithStreaming(
        module,
        batchNumber,
        contentMode,
        instructions,
        approvedReferences,
        (chunk) => {
          setStreamingContent((prev) => prev + chunk);
        },
        (result) => {
          addGeneratedBatch({
            moduleNumber: module.number,
            batchNumber,
            content: result.content,
            references: result.references,
            pageCount: result.pageCount,
            status: 'pending',
          });
          setStreamingContent('');
          setShowInstructionInput(false);
          setLocalInstructions('');
        },
        effectiveTarget,
        learningOutcomes,
        keywords,
        previousContent,
        effectiveTotalBatches,
        controller.signal,
        (pos, wait) => { setQueueStatus(pos, wait); },
        modelProvider,
        'generate',
        '',
        currentTaskId || '',
        null,
        structureConfig,
      );
    } catch (error) {
      if (error instanceof DOMException && error.name === 'AbortError') return;
      console.error('Generation error:', error);
      const msg = error instanceof Error ? error.message : 'Σφάλμα κατά τη δημιουργία περιεχομένου';
      useStore.getState().setError(msg);
    } finally {
      setIsGenerating(false);
      clearQueueStatus();
    }
  }, [
    module,
    moduleBatches,
    contentMode,
    localInstructions,
    userInstructions,
    approvedReferences,
    addGeneratedBatch,
    setIsGenerating,
    targetPages,
    totalModulePages,
    isEditDoc,
    extendPages,
    setTotalModulePages,
    learningOutcomes,
    keywords,
    getApprovedContent,
    totalBatches,
    setQueueStatus,
    currentTaskId,
    clearQueueStatus,
    modelProvider,
    structureConfig,
  ]);

  const handleApprove = useCallback(
    (batchNumber: number) => {
      if (selectedModule) {
        updateBatchStatus(selectedModule, batchNumber, 'approved');
      }
    },
    [selectedModule, updateBatchStatus]
  );

  const handleReject = useCallback(
    (batchNumber: number) => {
      if (selectedModule) {
        updateBatchStatus(selectedModule, batchNumber, 'rejected');
      }
    },
    [selectedModule, updateBatchStatus]
  );

  const handleFinalExport = useCallback(async () => {
    if (!module) return;
    const approvedBatches = moduleBatches.filter((b) => b.status === 'approved');
    if (approvedBatches.length === 0) return;

    await exportToWord({
      title: documentTitle,
      module,
      batches: approvedBatches,
      includeTableOfContents: true,
      includePageNumbers: true,
      skillReview: currentReview,
      finalExport: true,
      summary: selectedModule ? (moduleSummaries[selectedModule] || '') : '',
    });
  }, [module, moduleBatches, documentTitle, currentReview, selectedModule, moduleSummaries]);

  const handleApproveAndFinish = useCallback(
    async (batchNumber: number) => {
      if (!selectedModule || !module) return;
      updateBatchStatus(selectedModule, batchNumber, 'approved');

      // Collect all approved content (including the just-approved batch)
      const allApproved = moduleBatches
        .map((b) => b.batchNumber === batchNumber ? { ...b, status: 'approved' as const } : b)
        .filter((b) => b.status === 'approved');

      const fullContent = allApproved.map(b => b.content).join('\n\n---\n\n');

      // Generate summary (Περίληψη)
      let summaryText = '';
      setIsGeneratingSummary(true);
      try {
        summaryText = await generateSummary(module.title, fullContent, modelProvider, currentTaskId || '');
        setModuleSummary(selectedModule, summaryText);
      } catch (error) {
        console.error('Summary generation error:', error);
        const msg = error instanceof Error ? error.message : 'Σφάλμα κατά τη δημιουργία περίληψης';
        useStore.getState().setError(msg);
      } finally {
        setIsGeneratingSummary(false);
      }

      setProductionComplete(selectedModule);

      // Auto-trigger final consolidated export
      if (allApproved.length > 0) {
        await exportToWord({
          title: documentTitle,
          module,
          batches: allApproved,
          includeTableOfContents: true,
          includePageNumbers: true,
          skillReview: currentReview,
          finalExport: true,
          summary: summaryText,
        });
      }
    },
    [selectedModule, module, moduleBatches, documentTitle, currentReview, updateBatchStatus, setProductionComplete, setModuleSummary, setIsGeneratingSummary, modelProvider, currentTaskId]
  );

  const handleFeedbackSubmit = useCallback(async (feedback: string) => {
    const batchNumber = feedbackDialogBatch;
    if (!module || !batchNumber) return;

    // Revision mode: send the existing draft as assistant turn + the feedback
    // as a targeted edit instruction. The model keeps untouched sections verbatim.
    const targetBatch = moduleBatches.find((b) => b.batchNumber === batchNumber);
    const existingDraft = targetBatch?.content || '';

    setFeedbackDialogBatch(null);

    abortControllerRef.current?.abort();
    const controller = new AbortController();
    abortControllerRef.current = controller;

    setIsGenerating(true);
    setStreamingContent('');

    const previousContent = getApprovedContent();

    try {
      await generateWithStreaming(
        module,
        batchNumber,
        contentMode,
        feedback,
        approvedReferences,
        (chunk) => {
          setStreamingContent((prev) => prev + chunk);
        },
        (result) => {
          updateBatchContent(module.number, batchNumber, result.content, result.references, result.pageCount);
          setStreamingContent('');
        },
        targetPages,
        learningOutcomes,
        keywords,
        previousContent,
        totalBatches,
        controller.signal,
        (pos, wait) => { setQueueStatus(pos, wait); },
        modelProvider,
        'revision',
        existingDraft,
        currentTaskId || '',
        null,
        structureConfig,
      );
    } catch (error) {
      if (error instanceof DOMException && error.name === 'AbortError') return;
      console.error('Regeneration error:', error);
      const msg = error instanceof Error ? error.message : 'Σφάλμα κατά την αναδημιουργία περιεχομένου';
      useStore.getState().setError(msg);
    } finally {
      setIsGenerating(false);
      clearQueueStatus();
    }
  }, [feedbackDialogBatch, module, moduleBatches, contentMode, approvedReferences, getApprovedContent, setIsGenerating, updateBatchContent, targetPages, learningOutcomes, keywords, totalBatches, setQueueStatus, clearQueueStatus, modelProvider, currentTaskId, structureConfig]);

  const handleExport = useCallback(async () => {
    if (!module) return;

    const exportableBatches = moduleBatches.filter((b) => b.status !== 'rejected');
    if (exportableBatches.length === 0) return;

    const hasPending = exportableBatches.some((b) => b.status === 'pending');

    await exportToWord({
      title: documentTitle,
      module,
      batches: exportableBatches,
      includeTableOfContents: true,
      includePageNumbers: true,
      skillReview: currentReview,
      includePendingNote: hasPending,
    });
  }, [module, moduleBatches, documentTitle, currentReview]);

  const handleBack = useCallback(() => {
    setCurrentStep(isEscoMode ? 'modules' : 'upload');
  }, [isEscoMode, setCurrentStep]);

  const handleSkillReview = useCallback(async () => {
    if (!module) return;

    const approvedBatches = moduleBatches.filter((b) => b.status === 'approved');
    if (approvedBatches.length === 0) return;

    setIsReviewingSkills(true);
    setReviewProgress('Έναρξη ανάλυσης...');

    try {
      const combinedContent = approvedBatches.map((b) => b.content).join('\n\n---\n\n');

      const review = await generateSkillCoverageReview(
        module,
        combinedContent,
        (status) => setReviewProgress(status),
        modelProvider,
        currentTaskId || '',
      );

      setSkillReview(module.number, review);
      setShowReview(true);
    } catch (error) {
      console.error('Skill review error:', error);
      const msg = error instanceof Error ? error.message : 'Σφάλμα κατά την ανάλυση δεξιοτήτων';
      useStore.getState().setError(msg);
    } finally {
      setIsReviewingSkills(false);
      setReviewProgress('');
    }
  }, [module, moduleBatches, setIsReviewingSkills, setSkillReview, modelProvider, currentTaskId]);

  const approvedCount = moduleBatches.filter((b) => b.status === 'approved').length;
  const totalPages = moduleBatches
    .filter((b) => b.status === 'approved')
    .reduce((sum, b) => sum + b.pageCount, 0);

  if (!module) {
    return (
      <div className={styles.container}>
        <div className={styles.error}>Δεν βρέθηκε η ενότητα</div>
      </div>
    );
  }

  return (
    <div className={styles.container}>
      <motion.div
        initial={{ opacity: 0, y: -20 }}
        animate={{ opacity: 1, y: 0 }}
        className={styles.header}
      >
        <div className={styles.navButtons}>
          <button onClick={handleBack} className={styles.backButton}>
            <ArrowLeft size={20} />
            Πίσω
          </button>
          <button onClick={() => setShowResetConfirm(true)} className={styles.resetButton}>
            <RotateCcw size={16} />
            Νέα Εργασία
          </button>
        </div>
        <div className={styles.moduleInfo}>
          <span className={styles.moduleNumber}>Ενότητα {module.number}</span>
          <h1 className={styles.moduleTitle}>{module.title}</h1>
          <div className={styles.moduleMeta}>
            <span>{module.hours} ώρες</span>
            <span className={styles.separator}>•</span>
            <span>{approvedCount} τμήματα εγκεκριμένα</span>
            <span className={styles.separator}>•</span>
            <span>~{totalPages} σελίδες</span>
          </div>
          {totalModulePages > 0 && (
            <div className={styles.progressBar}>
              <div className={styles.progressTrack}>
                <div
                  className={styles.progressFill}
                  style={{ width: `${Math.min(100, (totalPages / totalModulePages) * 100)}%` }}
                />
              </div>
              <span className={styles.progressLabel}>
                {totalPages}/{totalModulePages} σελ. (~{totalBatches} τμήματα)
              </span>
            </div>
          )}
        </div>

        {moduleBatches.length > 0 && (
          <div className={styles.headerActions}>
            {isEscoMode && (
              <button
                onClick={handleSkillReview}
                className={styles.reviewButton}
                disabled={isReviewingSkills}
              >
                {isReviewingSkills ? (
                  <>
                    <Loader2 size={18} className={styles.spinner} />
                    {reviewProgress || 'Ανάλυση...'}
                  </>
                ) : (
                  <>
                    <Target size={18} />
                    Ανάλυση ESCO
                  </>
                )}
              </button>
            )}
            <button onClick={handleExport} className={styles.exportButton}>
              <FileDown size={18} />
              Εξαγωγή σε Word
            </button>
          </div>
        )}
      </motion.div>

      <div className={styles.content}>
        <div className={styles.batchesPanel}>
          <h2 className={styles.panelTitle}>
            <BookOpen size={20} />
            Τμήματα Υλικού
          </h2>

          <div className={styles.batchList}>
            {moduleBatches.map((batch, index) => (
              <motion.div
                key={batch.batchNumber}
                initial={{ opacity: 0, x: -20 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ delay: index * 0.05 }}
                className={`${styles.batchCard} ${styles[batch.status]}`}
              >
                <div className={styles.batchHeader}>
                  <span className={styles.batchNumber}>
                    Τμήμα {batch.batchNumber}
                  </span>
                  <span className={styles.batchPages}>
                    ~{batch.pageCount} σελ.
                  </span>
                </div>

                <div className={styles.batchPreview}>
                  {batch.content.substring(0, 200)}...
                </div>

                {batch.status === 'pending' && (
                  <>
                    <div className={styles.batchActions}>
                      <button
                        onClick={() => handleApprove(batch.batchNumber)}
                        className={styles.approveButton}
                      >
                        <Check size={16} />
                        Έγκριση
                      </button>
                      <button
                        onClick={() => setFeedbackDialogBatch(batch.batchNumber)}
                        className={styles.feedbackButton}
                      >
                        <Edit3 size={16} />
                        Αλλαγές
                      </button>
                      <button
                        onClick={() => handleReject(batch.batchNumber)}
                        className={styles.rejectButton}
                      >
                        <X size={16} />
                        Απόρριψη
                      </button>
                    </div>
                    <button
                      onClick={() => handleApproveAndFinish(batch.batchNumber)}
                      className={styles.approveFinishButton}
                      disabled={isGeneratingSummary || isModuleComplete}
                    >
                      <CheckCircle2 size={16} />
                      Έγκριση & τέλος παραγωγής
                    </button>
                  </>
                )}

                {batch.status === 'approved' && (
                  <div className={styles.batchStatus}>
                    <Check size={16} />
                    Εγκεκριμένο
                  </div>
                )}

                {batch.status === 'rejected' && (
                  <div className={styles.batchStatusRejected}>
                    <X size={16} />
                    Απορρίφθηκε
                  </div>
                )}
              </motion.div>
            ))}

            {isGenerating && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className={`${styles.batchCard} ${styles.generating}`}
              >
                <div className={styles.batchHeader}>
                  <span className={styles.batchNumber}>
                    Τμήμα {moduleBatches.length + 1}
                  </span>
                  <Loader2 size={16} className={styles.spinner} />
                </div>
                <div className={styles.batchPreview}>
                  {queuePosition != null && queuePosition > 0
                    ? `Θέση ${queuePosition} στην ουρά αναμονής${estimatedWait != null ? ` (~${estimatedWait} δευτερόλεπτα)` : ''}...`
                    : 'Δημιουργία υλικού...'}
                </div>
              </motion.div>
            )}
          </div>

          {isGeneratingSummary && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className={styles.completionNotice}
            >
              <Loader2 size={20} className={styles.spinner} />
              Δημιουργία Περίληψης Ενότητας...
            </motion.div>
          )}

          {isModuleComplete && !isGeneratingSummary && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className={styles.completionNotice}
            >
              <CheckCircle2 size={20} />
              Η παραγωγή υλικού έχει ολοκληρωθεί.
              <button onClick={handleFinalExport} className={styles.finalExportButton}>
                <FileDown size={16} />
                Τελικό Export
              </button>
            </motion.div>
          )}

          <AnimatePresence>
            {!isGenerating && !currentPendingBatch && !isModuleComplete && (
              <motion.div
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -20 }}
                className={styles.generateSection}
              >
                <div className={styles.structureSection}>
                  <span className={styles.structureTitle}>Δομή υλικού</span>
                  <div className={styles.structureGrid}>
                    {STRUCTURE_ELEMENTS.map(({ key, label, hint }) => (
                      <label key={key} className={styles.structureCheckbox} title={hint}>
                        <input
                          type="checkbox"
                          checked={structureConfig[key]}
                          onChange={(e) =>
                            setStructureConfig({ [key]: e.target.checked } as Partial<StructureConfig>)
                          }
                        />
                        <span>{label}{hint ? ' *' : ''}</span>
                      </label>
                    ))}
                  </div>
                </div>

                <div className={styles.instructionSection}>
                  <button
                    onClick={() => setShowInstructionInput(!showInstructionInput)}
                    className={styles.instructionToggle}
                  >
                    <Edit3 size={18} />
                    {showInstructionInput
                      ? 'Απόκρυψη οδηγιών'
                      : 'Προσθήκη οδηγιών'}
                  </button>

                  {showInstructionInput && (
                    <motion.div
                      initial={{ opacity: 0, height: 0 }}
                      animate={{ opacity: 1, height: 'auto' }}
                      exit={{ opacity: 0, height: 0 }}
                    >
                      <textarea
                        value={localInstructions}
                        onChange={(e) => setLocalInstructions(e.target.value)}
                        placeholder="Π.χ. Εστίασε περισσότερο στα πρακτικά παραδείγματα..."
                        className={styles.instructionInput}
                        rows={4}
                      />
                    </motion.div>
                  )}
                </div>

                {isEditDoc && (
                  <div className={styles.extendSection}>
                    <label htmlFor="extendPages" className={styles.extendLabel}>
                      Σελίδες προς προσθήκη
                    </label>
                    <input
                      id="extendPages"
                      type="number"
                      min={1}
                      max={40}
                      value={extendPages}
                      onChange={(e) =>
                        setExtendPages(Math.max(1, Math.min(40, Number(e.target.value) || 1)))
                      }
                      className={styles.extendInput}
                    />
                  </div>
                )}

                <button
                  onClick={handleGenerate}
                  className={`${styles.generateButton} ${contentMode === 'experimental' ? styles.generateButtonExperimental : ''}`}
                  disabled={isGenerating}
                >
                  {moduleBatches.length === 0 ? (
                    <>
                      {contentMode === 'experimental' ? <FlaskConical size={20} /> : <Sparkles size={20} />}
                      Έναρξη Δημιουργίας
                    </>
                  ) : isEditDoc ? (
                    <>
                      <RefreshCw size={20} />
                      Προσθήκη {extendPages} σελίδων
                    </>
                  ) : (
                    <>
                      <RefreshCw size={20} />
                      Δημιουργία Επόμενου Τμήματος
                    </>
                  )}
                </button>
              </motion.div>
            )}

            {currentPendingBatch && !isGenerating && (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className={styles.pendingNotice}
              >
                <span>Εκκρεμεί έγκριση για το Τμήμα {currentPendingBatch.batchNumber}</span>
                <ChevronRight size={16} />
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        <div className={styles.previewPanel}>
          {showReview && currentReview ? (
            <div className={styles.reviewPanel}>
              <button
                onClick={() => setShowReview(false)}
                className={styles.closeReviewButton}
              >
                <ArrowLeft size={16} />
                Πίσω στην προεπισκόπηση
              </button>
              <SkillCoverageReview
                review={currentReview}
                onRegenerateReview={handleSkillReview}
                onExport={handleExport}
                isRegenerating={isReviewingSkills}
              />
            </div>
          ) : (
            <>
              <div className={styles.panelHeader}>
                <h2 className={styles.panelTitle}>Προεπισκόπηση</h2>
                {isEscoMode && currentReview && (
                  <button
                    onClick={() => setShowReview(true)}
                    className={styles.showReviewLink}
                  >
                    <Target size={16} />
                    Δείτε την ανάλυση ESCO ({currentReview.coveragePercentage}%)
                  </button>
                )}
              </div>

              <div ref={contentRef} className={styles.previewContent}>
                {isGenerating && streamingContent && (
                  <div className={styles.streamingContent}>
                    {streamingContent}
                    <span className={styles.cursor}>▊</span>
                  </div>
                )}

                {!isGenerating && currentPendingBatch && (
                  <div className={styles.batchContent}>
                    {currentPendingBatch.content}
                  </div>
                )}

                {!isGenerating && !currentPendingBatch && moduleBatches.length > 0 && (
                  <div className={styles.batchContent}>
                    {moduleBatches[moduleBatches.length - 1]?.content}
                  </div>
                )}

                {!isGenerating && moduleBatches.length === 0 && (
                  <div className={styles.emptyState}>
                    <BookOpen size={48} />
                    <p>Πατήστε "Έναρξη Δημιουργίας" για να δημιουργήσετε το πρώτο τμήμα υλικού</p>
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      </div>

      <BatchFeedbackDialog
        isOpen={feedbackDialogBatch !== null}
        batchNumber={feedbackDialogBatch ?? 0}
        batchPreview={
          moduleBatches.find((b) => b.batchNumber === feedbackDialogBatch)?.content.substring(0, 500) || ''
        }
        onSubmit={handleFeedbackSubmit}
        onCancel={() => setFeedbackDialogBatch(null)}
      />

      <ConfirmResetDialog
        isOpen={showResetConfirm}
        onConfirm={() => { setShowResetConfirm(false); reset(); }}
        onCancel={() => setShowResetConfirm(false)}
      />
    </div>
  );
}
