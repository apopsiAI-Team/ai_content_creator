import { useCallback, useMemo, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Upload,
  FileText,
  X,
  AlertCircle,
  ChevronRight,
  ChevronDown,
  Sparkles,
  FlaskConical,
  BookOpen,
  Database,
  Cpu,
  Zap,
  Clock,
  Trash2,
  Play,
  Layers,
  Edit3,
} from 'lucide-react';
import { useStore, type PendingTask } from '../store/useStore';
import { parseDocx } from '../utils/docxParser';
import { docxToMarkdown } from '../utils/docxToMarkdown';
import styles from './LandingPage.module.css';

export function LandingPage() {
  const {
    setDocumentFile,
    setDocumentData,
    setStandardModule,
    setUserInstructions,
    setError,
    modelProvider,
    setModelProvider,
    pendingTasks,
    loadPendingTask,
    deletePendingTask,
    loadDocForEditing,
  } = useStore();
  const [activeTab, setActiveTab] = useState<'standard' | 'esco' | 'edit-doc'>('standard');

  // Edit-doc upload state
  const [editFile, setEditFile] = useState<File | null>(null);
  const [editProcessing, setEditProcessing] = useState(false);
  const [editError, setEditError] = useState<string | null>(null);
  const [editDragging, setEditDragging] = useState(false);
  const [pendingOpen, setPendingOpen] = useState(true);
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);

  const sortedPending = useMemo(
    () => [...pendingTasks].sort((a, b) => b.updatedAt - a.updatedAt),
    [pendingTasks]
  );

  // Standard form state
  const [topic, setTopic] = useState('');
  const [hours, setHours] = useState<number>(10);
  const [modulePages, setModulePages] = useState<number>(55);
  const [pagesPerBatch, setPagesPerBatch] = useState<number>(20);
  const [contentMode, setContentMode] = useState<'standard' | 'experimental'>('standard');
  const [withInstructions, setWithInstructions] = useState(false);
  const [instructions, setInstructions] = useState('');

  // ESCO upload state
  const [isDragging, setIsDragging] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [uploadedFile, setUploadedFile] = useState<File | null>(null);
  const [parseError, setParseError] = useState<string | null>(null);

  // Standard form submit — store actions handle the reset internally
  const handleStartStandard = useCallback(() => {
    const trimmed = topic.trim();
    if (!trimmed) return;
    setStandardModule(trimmed, hours, modulePages, pagesPerBatch, '', '', contentMode);
    if (withInstructions && instructions.trim()) {
      setUserInstructions(instructions.trim());
    }
  }, [topic, hours, modulePages, pagesPerBatch, contentMode, withInstructions, instructions, setStandardModule, setUserInstructions]);

  // ESCO file handling
  const handleFile = useCallback(async (file: File) => {
    if (!file.name.endsWith('.docx')) {
      setParseError('Παρακαλώ ανεβάστε αρχείο Word (.docx)');
      return;
    }

    setIsProcessing(true);
    setParseError(null);
    setUploadedFile(file);

    try {
      const result = await parseDocx(file);

      if (result.modules.length === 0) {
        setParseError('Δεν βρέθηκαν ενότητες στο έγγραφο. Παρακαλώ ελέγξτε τη μορφή.');
        setIsProcessing(false);
        return;
      }

      setDocumentFile(file);
      setDocumentData(result.title, result.modules, result.totalHours);
    } catch (error) {
      console.error('Parse error:', error);
      setParseError('Σφάλμα κατά την ανάγνωση του εγγράφου. Παρακαλώ δοκιμάστε ξανά.');
      setError('Αποτυχία ανάγνωσης εγγράφου');
    } finally {
      setIsProcessing(false);
    }
  }, [setDocumentFile, setDocumentData, setError]);

  // Edit-doc: upload any .docx, parse to markdown, seed a pending batch.
  const handleEditFile = useCallback(async (file: File) => {
    if (!file.name.toLowerCase().endsWith('.docx')) {
      setEditError('Παρακαλώ ανεβάστε αρχείο Word (.docx)');
      return;
    }
    setEditProcessing(true);
    setEditError(null);
    setEditFile(file);
    try {
      const { title, markdown } = await docxToMarkdown(file);
      if (!markdown.trim()) {
        setEditError('Το έγγραφο δεν περιείχε αναγνώσιμο κείμενο.');
        setEditProcessing(false);
        return;
      }
      loadDocForEditing(title, markdown);
    } catch (error) {
      console.error('Edit-doc parse error:', error);
      setEditError('Σφάλμα κατά την ανάγνωση του εγγράφου.');
      setError('Αποτυχία ανάγνωσης εγγράφου');
    } finally {
      setEditProcessing(false);
    }
  }, [loadDocForEditing, setError]);

  const handleEditDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setEditDragging(false);
    const file = e.dataTransfer.files[0];
    if (file) handleEditFile(file);
  }, [handleEditFile]);

  const handleEditDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setEditDragging(true);
  }, []);

  const handleEditDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setEditDragging(false);
  }, []);

  const handleEditInputChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) handleEditFile(file);
  }, [handleEditFile]);

  const clearEditFile = useCallback(() => {
    setEditFile(null);
    setEditError(null);
  }, []);

  const handleContinueTask = useCallback((task: PendingTask) => {
    loadPendingTask(task.id);
  }, [loadPendingTask]);

  const handleDeleteTask = useCallback((id: string) => {
    deletePendingTask(id);
    setDeleteConfirmId(null);
  }, [deletePendingTask]);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    const file = e.dataTransfer.files[0];
    if (file) handleFile(file);
  }, [handleFile]);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
  }, []);

  const handleInputChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) handleFile(file);
  }, [handleFile]);

  const clearFile = useCallback(() => {
    setUploadedFile(null);
    setParseError(null);
  }, []);

  const isFormValid = topic.trim().length > 0 && hours > 0 && modulePages > 0 && pagesPerBatch > 0;
  const estimatedBatches = Math.ceil(modulePages / pagesPerBatch);

  return (
    <div className={styles.container}>
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.6 }}
        className={styles.header}
      >
        <div className={styles.brand}>
          <div className={styles.brandIconWrapper}>
            <div className={styles.brandBook}></div>
            <div className={styles.brandBook}></div>
            <div className={styles.brandBook}></div>
          </div>
          <div className={styles.brandTextGroup}>
            <span className={styles.brandText}>APOPSI</span>
            <span className={styles.brandSubtext}>e-learning</span>
          </div>
        </div>
        <h1 className={styles.title}>Δημιουργός Εκπαιδευτικού Υλικού</h1>
        <p className={styles.subtitle}>
          Δημιουργήστε αναλυτικό υλικό μελέτης με βιβλιογραφία και ακαδημαϊκή ποιότητα
        </p>
      </motion.div>

      {/* Εκκρεμότητες (pending sessions) — always rendered so the feature is discoverable */}
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.1 }}
        className={`${styles.pendingSection} ${sortedPending.length === 0 ? styles.pendingSectionEmpty : ''}`}
      >
        <div
          className={styles.pendingHeader}
          onClick={() => sortedPending.length > 0 && setPendingOpen((v) => !v)}
          role={sortedPending.length > 0 ? 'button' : undefined}
          tabIndex={sortedPending.length > 0 ? 0 : undefined}
          onKeyDown={(e) => {
            if (sortedPending.length === 0) return;
            if (e.key === 'Enter' || e.key === ' ') setPendingOpen((v) => !v);
          }}
          style={{ cursor: sortedPending.length > 0 ? 'pointer' : 'default' }}
        >
          <div className={styles.pendingTitle}>
            <Clock size={18} />
            <span>Εκκρεμότητες</span>
            <span className={styles.pendingBadge}>{sortedPending.length}</span>
          </div>
          {sortedPending.length > 0 ? (
            <ChevronDown
              size={20}
              className={`${styles.pendingChevron} ${pendingOpen ? styles.pendingChevronOpen : ''}`}
            />
          ) : (
            <span className={styles.pendingEmptyHint}>Δεν υπάρχουν εκκρεμότητες</span>
          )}
        </div>
        {sortedPending.length > 0 && (
          <AnimatePresence initial={false}>
            {pendingOpen && (
              <motion.div
                key="pending-list"
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: 'auto' }}
                exit={{ opacity: 0, height: 0 }}
                transition={{ duration: 0.2 }}
                style={{ overflow: 'hidden' }}
              >
                <div className={styles.pendingList}>
                  {sortedPending.map((task) => {
                    const approvedCount = task.generatedBatches.filter((b) => b.status === 'approved').length;
                    const totalBatches = task.generatedBatches.length;
                    const moduleCount = task.modules.length;
                    const completedModules = task.modules.filter((m) => task.productionComplete[m.number]).length;
                    return (
                      <div key={task.id} className={styles.pendingItem}>
                        <div className={styles.pendingInfo}>
                          <div className={styles.pendingItemTitle} title={task.title}>
                            {task.title || '(Χωρίς τίτλο)'}
                          </div>
                          <div className={styles.pendingItemMeta}>
                            <span className={styles.pendingItemMetaPart}>
                              <Layers size={13} />
                              Παραχθέντα τμήματα: {approvedCount}{totalBatches !== approvedCount ? ` / ${totalBatches}` : ''}
                            </span>
                            <span className={styles.pendingItemMetaPart}>
                              {task.workflowMode === 'esco' ? <Database size={13} /> : <BookOpen size={13} />}
                              {task.workflowMode === 'esco'
                                ? `ESCO · ${completedModules}/${moduleCount} ενότητες`
                                : 'Standard'}
                            </span>
                            <span className={styles.pendingItemMetaPart}>
                              {task.modelProvider === 'openai' ? <Zap size={13} /> : <Cpu size={13} />}
                              {task.modelProvider === 'openai' ? 'GPT-5.4' : 'Claude'}
                            </span>
                          </div>
                        </div>
                        <div className={styles.pendingActions}>
                          {deleteConfirmId === task.id ? (
                            <>
                              <button
                                className={`${styles.pendingButton} ${styles.pendingButtonDanger}`}
                                onClick={() => handleDeleteTask(task.id)}
                                aria-label="Επιβεβαίωση διαγραφής"
                              >
                                Επιβεβαίωση
                              </button>
                              <button
                                className={styles.pendingButton}
                                onClick={() => setDeleteConfirmId(null)}
                                aria-label="Άκυρο"
                                style={{ background: 'rgba(0,0,0,0.05)', color: 'var(--text-primary, #1a1f35)' }}
                              >
                                Άκυρο
                              </button>
                            </>
                          ) : (
                            <>
                              <button
                                className={`${styles.pendingButton} ${styles.pendingButtonPrimary}`}
                                onClick={() => handleContinueTask(task)}
                                aria-label="Συνέχεια εργασίας"
                              >
                                <Play size={14} />
                                Συνέχεια
                              </button>
                              <button
                                className={`${styles.pendingButton} ${styles.pendingButtonDanger}`}
                                onClick={() => setDeleteConfirmId(task.id)}
                                aria-label="Διαγραφή εκκρεμότητας"
                                title="Διαγραφή"
                              >
                                <Trash2 size={14} />
                              </button>
                            </>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        )}
      </motion.div>

      {/* Tab Bar */}
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.15 }}
        className={styles.tabBar}
      >
        <button
          className={`${styles.tabButton} ${activeTab === 'standard' ? styles.tabButtonActive : ''}`}
          onClick={() => setActiveTab('standard')}
        >
          <BookOpen size={18} />
          Standard
        </button>
        <button
          className={`${styles.tabButton} ${activeTab === 'esco' ? styles.tabButtonActive : ''}`}
          onClick={() => setActiveTab('esco')}
        >
          <Database size={18} />
          ESCO Integrated
        </button>
        <button
          className={`${styles.tabButton} ${activeTab === 'edit-doc' ? styles.tabButtonActive : ''}`}
          onClick={() => setActiveTab('edit-doc')}
        >
          <Edit3 size={18} />
          Επεξεργασία υλικού
        </button>
      </motion.div>

      {/* Tab Content */}
      <AnimatePresence mode="wait">
        {activeTab === 'standard' ? (
          <motion.div
            key="standard"
            initial={{ opacity: 0, x: -20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: 20 }}
            transition={{ duration: 0.2 }}
            className={styles.tabContent}
          >
            <div className={styles.formCard}>
              {/* Topic */}
              <div className={styles.formGroup}>
                <label className={styles.formLabel}>
                  Θεματική Ενότητα <span className={styles.formRequired}>*</span>
                </label>
                <input
                  type="text"
                  value={topic}
                  onChange={(e) => setTopic(e.target.value)}
                  placeholder="π.χ. Ψηφιακό Μάρκετινγκ και Στρατηγικές Προώθησης"
                  className={styles.formInput}
                />
              </div>

              {/* Hours + Module Pages row */}
              <div className={styles.formRow}>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel}>
                    Ώρες ενότητας <span className={styles.formRequired}>*</span>
                  </label>
                  <input
                    type="number"
                    value={hours}
                    onChange={(e) => setHours(Math.max(1, parseInt(e.target.value) || 1))}
                    min={1}
                    className={styles.formInput}
                  />
                </div>
                <div className={styles.formGroup}>
                  <label className={styles.formLabel}>
                    Σελίδες ενότητας <span className={styles.formRequired}>*</span>
                  </label>
                  <input
                    type="number"
                    value={modulePages}
                    onChange={(e) => setModulePages(Math.max(1, parseInt(e.target.value) || 1))}
                    min={1}
                    className={styles.formInput}
                  />
                  <span className={styles.formHint}>Συνολικές σελίδες ενότητας</span>
                </div>
              </div>

              {/* Pages per batch */}
              <div className={styles.formGroup}>
                <label className={styles.formLabel}>
                  Σελίδες ανά τμήμα <span className={styles.formRequired}>*</span>
                </label>
                <input
                  type="number"
                  value={pagesPerBatch}
                  onChange={(e) => setPagesPerBatch(Math.max(1, parseInt(e.target.value) || 1))}
                  min={1}
                  max={modulePages}
                  className={styles.formInput}
                />
                <span className={styles.formHint}>
                  Εκτιμώμενα τμήματα: {estimatedBatches}
                </span>
              </div>

              {/* Model Provider Toggle */}
              <div className={styles.modeSection}>
                <label className={styles.modeSectionLabel}>Μοντέλο</label>
                <div className={styles.modeToggle}>
                  <button
                    className={`${styles.modeButton} ${modelProvider === 'claude' ? styles.modeButtonActive : ''}`}
                    onClick={() => setModelProvider('claude')}
                  >
                    <Cpu size={16} />
                    Claude Opus 4.6
                  </button>
                  <button
                    className={`${styles.modeButton} ${modelProvider === 'openai' ? styles.modeButtonActive : ''}`}
                    onClick={() => setModelProvider('openai')}
                  >
                    <Zap size={16} />
                    GPT-5.4
                  </button>
                </div>
              </div>

              {/* Mode Toggle */}
              <div className={styles.modeSection}>
                <label className={styles.modeSectionLabel}>Λειτουργία Δημιουργίας</label>
                <div className={styles.modeToggle}>
                  <button
                    className={`${styles.modeButton} ${contentMode === 'standard' ? styles.modeButtonActive : ''}`}
                    onClick={() => setContentMode('standard')}
                  >
                    <Sparkles size={16} />
                    Κανονική
                  </button>
                  <button
                    className={`${styles.modeButton} ${styles.modeButtonExperimental} ${contentMode === 'experimental' ? styles.modeButtonActive : ''}`}
                    onClick={() => setContentMode('experimental')}
                  >
                    <FlaskConical size={16} />
                    Πειραματική
                  </button>
                </div>
                {contentMode === 'experimental' && (
                  <p className={styles.experimentalHint}>
                    Opus μόνος - αυστηρός έλεγχος με υποχρεωτική βιβλιογραφία
                  </p>
                )}
                <label className={styles.checkboxLabel}>
                  <input
                    type="checkbox"
                    checked={withInstructions}
                    onChange={(e) => setWithInstructions(e.target.checked)}
                    className={styles.checkbox}
                  />
                  Προσθήκη οδηγιών
                </label>
                {withInstructions && (
                  <textarea
                    value={instructions}
                    onChange={(e) => setInstructions(e.target.value)}
                    placeholder="π.χ. Εστίασε στα πρακτικά παραδείγματα, χρησιμοποίησε ελληνικές πηγές..."
                    className={styles.instructionsArea}
                    rows={3}
                  />
                )}
              </div>

              {/* Start Button */}
              <button
                className={styles.startButton}
                onClick={handleStartStandard}
                disabled={!isFormValid}
              >
                Εκκίνηση Δημιουργίας
                <ChevronRight size={20} />
              </button>
            </div>
          </motion.div>
        ) : activeTab === 'esco' ? (
          <motion.div
            key="esco"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            transition={{ duration: 0.2 }}
            className={styles.tabContent}
          >
            <div
              className={`${styles.dropzone} ${isDragging ? styles.dragging : ''} ${uploadedFile ? styles.hasFile : ''}`}
              onDrop={handleDrop}
              onDragOver={handleDragOver}
              onDragLeave={handleDragLeave}
            >
              <input
                type="file"
                accept=".docx"
                onChange={handleInputChange}
                className={styles.dropzoneInput}
                id="file-upload"
                disabled={isProcessing}
              />

              <AnimatePresence mode="wait">
                {isProcessing ? (
                  <motion.div
                    key="processing"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0 }}
                    className={styles.processing}
                  >
                    <div className={styles.spinner} />
                    <span>Ανάλυση εγγράφου...</span>
                  </motion.div>
                ) : uploadedFile && parseError ? (
                  <motion.div
                    key="error"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0 }}
                    className={styles.errorState}
                  >
                    <AlertCircle size={48} />
                    <span className={styles.errorText}>{parseError}</span>
                    <button onClick={clearFile} className={styles.retryButton}>
                      Δοκιμάστε ξανά
                    </button>
                  </motion.div>
                ) : uploadedFile ? (
                  <motion.div
                    key="file"
                    initial={{ opacity: 0, scale: 0.9 }}
                    animate={{ opacity: 1, scale: 1 }}
                    exit={{ opacity: 0, scale: 0.9 }}
                    className={styles.filePreview}
                  >
                    <FileText size={48} />
                    <span className={styles.fileName}>{uploadedFile.name}</span>
                    <button onClick={clearFile} className={styles.clearButton} aria-label="Αφαίρεση αρχείου">
                      <X size={20} />
                    </button>
                  </motion.div>
                ) : (
                  <motion.label
                    key="upload"
                    htmlFor="file-upload"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0 }}
                    className={styles.uploadLabel}
                  >
                    <div className={styles.iconWrapper}>
                      <Upload size={48} />
                    </div>
                    <span className={styles.uploadText}>
                      {isDragging ? 'Αφήστε το αρχείο εδώ' : 'Σύρετε το αρχείο εδώ ή κάντε κλικ για επιλογή'}
                    </span>
                    <span className={styles.uploadHint}>
                      Ανεβάστε εκπαιδευτικό σχεδιασμό (.docx) με δομή ESCO
                    </span>
                  </motion.label>
                )}
              </AnimatePresence>
            </div>
          </motion.div>
        ) : (
          <motion.div
            key="edit-doc"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            transition={{ duration: 0.2 }}
            className={styles.tabContent}
          >
            <div
              className={`${styles.dropzone} ${editDragging ? styles.dragging : ''} ${editFile ? styles.hasFile : ''}`}
              onDrop={handleEditDrop}
              onDragOver={handleEditDragOver}
              onDragLeave={handleEditDragLeave}
            >
              <input
                type="file"
                accept=".docx"
                onChange={handleEditInputChange}
                className={styles.dropzoneInput}
                id="edit-file-upload"
                disabled={editProcessing}
              />

              <AnimatePresence mode="wait">
                {editProcessing ? (
                  <motion.div
                    key="edit-processing"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0 }}
                    className={styles.processing}
                  >
                    <div className={styles.spinner} />
                    <span>Ανάλυση εγγράφου...</span>
                  </motion.div>
                ) : editFile && editError ? (
                  <motion.div
                    key="edit-error"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0 }}
                    className={styles.errorState}
                  >
                    <AlertCircle size={48} />
                    <span className={styles.errorText}>{editError}</span>
                    <button onClick={clearEditFile} className={styles.retryButton}>
                      Δοκιμάστε ξανά
                    </button>
                  </motion.div>
                ) : editFile ? (
                  <motion.div
                    key="edit-file"
                    initial={{ opacity: 0, scale: 0.9 }}
                    animate={{ opacity: 1, scale: 1 }}
                    exit={{ opacity: 0, scale: 0.9 }}
                    className={styles.filePreview}
                  >
                    <FileText size={48} />
                    <span className={styles.fileName}>{editFile.name}</span>
                    <button onClick={clearEditFile} className={styles.clearButton} aria-label="Αφαίρεση αρχείου">
                      <X size={20} />
                    </button>
                  </motion.div>
                ) : (
                  <motion.label
                    key="edit-upload"
                    htmlFor="edit-file-upload"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0 }}
                    className={styles.uploadLabel}
                  >
                    <div className={styles.iconWrapper}>
                      <Edit3 size={48} />
                    </div>
                    <span className={styles.uploadText}>
                      {editDragging ? 'Αφήστε το αρχείο εδώ' : 'Σύρετε .docx εδώ ή κάντε κλικ για επιλογή'}
                    </span>
                    <span className={styles.uploadHint}>
                      Ανεβάστε υπάρχον υλικό για στοχευμένες αλλαγές
                    </span>
                  </motion.label>
                )}
              </AnimatePresence>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Features */}
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.4 }}
        className={styles.features}
      >
        <div className={styles.feature}>
          <div className={styles.featureIcon}>📚</div>
          <div className={styles.featureText}>
            <strong>Αυτόματη Βιβλιογραφία</strong>
            <span>Ενσωμάτωση ακαδημαϊκών πηγών</span>
          </div>
        </div>
        <div className={styles.feature}>
          <div className={styles.featureIcon}>📝</div>
          <div className={styles.featureText}>
            <strong>Υλικό Εκπαιδευόμενου</strong>
            <span>Σημειώσεις, ασκήσεις, case studies</span>
          </div>
        </div>
        <div className={styles.feature}>
          <div className={styles.featureIcon}>📄</div>
          <div className={styles.featureText}>
            <strong>Εξαγωγή σε Word</strong>
            <span>Έτοιμο για εκτύπωση και διανομή</span>
          </div>
        </div>
      </motion.div>
    </div>
  );
}
