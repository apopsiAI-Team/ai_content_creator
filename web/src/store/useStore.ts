import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export interface ESCOSkill {
  code: string;
  name: string;
  type: 'essential' | 'optional';
}

export interface Module {
  number: number;
  title: string;
  hours: number;
  content: string;
  activities: string;
  skills: ESCOSkill[];
}

export interface Reference {
  doi?: string;
  title: string;
  authors: string[];
  year?: number;
  journal?: string;
  url?: string;
  abstract?: string;
  citationAPA?: string;
  status: 'pending' | 'approved' | 'rejected' | 'auto';
  sourceProvider?: string;
}

export interface GeneratedContent {
  moduleNumber: number;
  batchNumber: number;
  content: string;
  references: Reference[];
  pageCount: number;
  status: 'pending' | 'approved' | 'rejected' | 'generating';
}

export interface SkillAnalysis {
  skillCode: string;
  skillName: string;
  skillType: 'essential' | 'optional';
  coverageLevel: 'full' | 'partial' | 'missing';
  evidence: string[];
  contentSections: string[];
  notes: string;
}

export interface SkillCoverageReview {
  moduleNumber: number;
  generatedAt: string;
  totalSkills: number;
  coveredFully: number;
  coveredPartially: number;
  missing: number;
  coveragePercentage: number;
  skillAnalysis: SkillAnalysis[];
  overallAssessment: string;
  recommendations: string[];
}

/**
 * Which OPTIONAL structural elements to include in generated material.
 * The quality core (academic register, paragraph depth, APA citations,
 * mandatory Βιβλιογραφία, anti-hallucination) is always present and not
 * configurable. Field names match the backend `structure_config` contract.
 */
export interface StructureConfig {
  activities: boolean;          // Δραστηριότητες (2 ανά υποενότητα)
  self_assessment: boolean;     // Ερωτήσεις / Απαντήσεις Αυτοαξιολόγησης
  glossary: boolean;            // Γλωσσάρι
  subsection_keywords: boolean; // **Βασικές λέξεις:** ανά υποενότητα
  in_text_citations: boolean;   // Υποχρεωτικές παρενθετικές αναφορές (Επώνυμο, Έτος)
}

export const DEFAULT_STRUCTURE_CONFIG: StructureConfig = {
  activities: true,
  self_assessment: true,
  glossary: true,
  subsection_keywords: true,
  in_text_citations: true,
};

/**
 * A snapshot of an in-progress educational-material session.
 * Saved to localStorage so the user can resume after closing the tab,
 * losing connection, or starting another task in between.
 *
 * Auto-removed when every module in `modules` reaches productionComplete.
 */
export interface PendingTask {
  id: string;
  title: string;
  workflowMode: 'standard' | 'esco';
  contentMode: 'standard' | 'experimental';
  modelProvider: 'claude' | 'openai';
  userInstructions: string;
  modules: Module[];
  totalHours: number;
  selectedModule: number | null;
  currentBatch: number;
  totalModulePages: number;
  targetPages: number;
  learningOutcomes: string;
  keywords: string;
  isEditDoc: boolean;
  structureConfig: StructureConfig;
  generatedBatches: GeneratedContent[];
  approvedReferences: Reference[];
  skillReviews: Record<number, SkillCoverageReview>;
  productionComplete: Record<number, boolean>;
  moduleSummaries: Record<number, string>;
  createdAt: number;
  updatedAt: number;
}

export interface AppState {
  // Workflow mode
  workflowMode: 'standard' | 'esco';

  // Document state
  documentFile: File | null;
  documentTitle: string;
  modules: Module[];
  totalHours: number;

  // Standard mode extras
  totalModulePages: number;
  targetPages: number;
  learningOutcomes: string;
  keywords: string;

  // Edit-doc flow: an existing .docx was uploaded for editing/extension.
  // Enables the "Επέκταση" control (add N pages on top of the uploaded draft).
  isEditDoc: boolean;

  // Optional structural elements to include in generated material (see type).
  structureConfig: StructureConfig;

  // Generation state
  selectedModule: number | null;
  currentBatch: number;
  generatedBatches: GeneratedContent[];
  isGenerating: boolean;
  generationProgress: number;

  // Content generation mode (2 options + optional instructions)
  contentMode: 'standard' | 'experimental';
  userInstructions: string;

  // LLM provider selection
  modelProvider: 'claude' | 'openai';

  // References
  pendingReferences: Reference[];
  approvedReferences: Reference[];

  // ESCO Skill Reviews
  skillReviews: Record<number, SkillCoverageReview>;
  isReviewingSkills: boolean;

  // Production tracking
  productionComplete: Record<number, boolean>;
  moduleSummaries: Record<number, string>;
  isGeneratingSummary: boolean;

  // Queue state (rate limiting feedback)
  queuePosition: number | null;
  estimatedWait: number | null;

  // UI state
  currentStep: 'upload' | 'modules' | 'generate' | 'review' | 'export';
  error: string | null;

  // Pending tasks ("Εκκρεμότητες") — surface in Landing
  currentTaskId: string | null;
  pendingTasks: PendingTask[];

  // Actions
  setDocumentFile: (file: File | null) => void;
  setDocumentData: (title: string, modules: Module[], totalHours: number) => void;
  setStandardModule: (title: string, hours: number, totalModulePages: number, targetPages: number, learningOutcomes: string, keywords: string, contentMode: 'standard' | 'experimental') => void;
  loadDocForEditing: (title: string, markdown: string) => void;
  setSelectedModule: (moduleNumber: number | null) => void;
  setCurrentBatch: (batch: number) => void;
  addGeneratedBatch: (batch: GeneratedContent) => void;
  updateBatchStatus: (moduleNumber: number, batchNumber: number, status: GeneratedContent['status']) => void;
  setIsGenerating: (generating: boolean) => void;
  setGenerationProgress: (progress: number) => void;
  setContentMode: (mode: 'standard' | 'experimental') => void;
  setUserInstructions: (instructions: string) => void;
  setModelProvider: (provider: 'claude' | 'openai') => void;
  addPendingReferences: (refs: Reference[]) => void;
  approveReference: (index: number) => void;
  rejectReference: (index: number) => void;
  approveAllReferences: () => void;
  setCurrentStep: (step: AppState['currentStep']) => void;
  setError: (error: string | null) => void;
  setSkillReview: (moduleNumber: number, review: SkillCoverageReview) => void;
  clearSkillReview: (moduleNumber: number) => void;
  setIsReviewingSkills: (reviewing: boolean) => void;
  setProductionComplete: (moduleNumber: number) => void;
  setModuleSummary: (moduleNumber: number, summary: string) => void;
  setIsGeneratingSummary: (generating: boolean) => void;
  setTotalModulePages: (pages: number) => void;
  setTargetPages: (pages: number) => void;
  setStructureConfig: (cfg: Partial<StructureConfig>) => void;
  setModuleHours: (moduleNumber: number, hours: number) => void;
  setQueueStatus: (position: number, wait: number) => void;
  clearQueueStatus: () => void;
  updateBatchContent: (moduleNumber: number, batchNumber: number, content: string, references: Reference[], pageCount: number) => void;
  loadPendingTask: (id: string) => void;
  deletePendingTask: (id: string) => void;
  reset: () => void;
}

const activeStateDefaults = {
  workflowMode: 'standard' as const,
  documentFile: null,
  documentTitle: '',
  modules: [] as Module[],
  totalHours: 0,
  totalModulePages: 0,
  targetPages: 20,
  learningOutcomes: '',
  keywords: '',
  isEditDoc: false,
  structureConfig: DEFAULT_STRUCTURE_CONFIG,
  selectedModule: null as number | null,
  currentBatch: 1,
  generatedBatches: [] as GeneratedContent[],
  isGenerating: false,
  generationProgress: 0,
  contentMode: 'standard' as const,
  userInstructions: '',
  pendingReferences: [] as Reference[],
  approvedReferences: [] as Reference[],
  skillReviews: {} as Record<number, SkillCoverageReview>,
  isReviewingSkills: false,
  productionComplete: {} as Record<number, boolean>,
  moduleSummaries: {} as Record<number, string>,
  isGeneratingSummary: false,
  queuePosition: null as number | null,
  estimatedWait: null as number | null,
  currentStep: 'upload' as const,
  error: null as string | null,
  currentTaskId: null as string | null,
};

const initialState = {
  ...activeStateDefaults,
  modelProvider: 'claude' as const,
  pendingTasks: [] as PendingTask[],
};

// Generate a short opaque task id. crypto.randomUUID is available in modern browsers.
function makeTaskId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `task-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

// Build a snapshot of the current active session for the Εκκρεμότητες list.
function snapshotTask(state: AppState, id: string, createdAt: number): PendingTask {
  return {
    id,
    title: state.documentTitle,
    workflowMode: state.workflowMode,
    contentMode: state.contentMode,
    modelProvider: state.modelProvider,
    userInstructions: state.userInstructions,
    modules: state.modules,
    totalHours: state.totalHours,
    selectedModule: state.selectedModule,
    currentBatch: state.currentBatch,
    totalModulePages: state.totalModulePages,
    targetPages: state.targetPages,
    learningOutcomes: state.learningOutcomes,
    keywords: state.keywords,
    isEditDoc: state.isEditDoc,
    structureConfig: state.structureConfig,
    generatedBatches: state.generatedBatches,
    approvedReferences: state.approvedReferences,
    skillReviews: state.skillReviews,
    productionComplete: state.productionComplete,
    moduleSummaries: state.moduleSummaries,
    createdAt,
    updatedAt: Date.now(),
  };
}

// Upsert the snapshot for the current task into the pendingTasks list.
// Skipped while no batches have been generated yet — empty starts don't pollute the list.
function upsertCurrent(state: AppState): PendingTask[] {
  const id = state.currentTaskId;
  if (!id) return state.pendingTasks;
  if (state.generatedBatches.length === 0) return state.pendingTasks;
  const existing = state.pendingTasks.find((t) => t.id === id);
  const createdAt = existing?.createdAt ?? Date.now();
  const snap = snapshotTask(state, id, createdAt);
  if (existing) {
    return state.pendingTasks.map((t) => (t.id === id ? snap : t));
  }
  return [...state.pendingTasks, snap];
}

// Clean up stale sessionStorage from previous persist config.
if (typeof window !== 'undefined') {
  sessionStorage.removeItem('edu-material-store');
}

export const useStore = create<AppState>()(
  persist<AppState, [], [], Partial<AppState>>(
    (set) => ({
      ...initialState,

      setDocumentFile: (file) => set({ documentFile: file }),

      // ESCO upload — start a brand-new task. Active state reset so it can't
      // bleed in from the previous session; pending list and provider preserved.
      setDocumentData: (title, modules, totalHours) => set((state) => ({
        ...activeStateDefaults,
        modelProvider: state.modelProvider,
        pendingTasks: state.pendingTasks,
        currentTaskId: makeTaskId(),
        workflowMode: 'esco',
        documentTitle: title,
        modules,
        totalHours,
        currentStep: 'modules',
      })),

      // Standard topic — start a brand-new task.
      setStandardModule: (title, hours, totalModulePages, targetPages, learningOutcomes, keywords, contentMode) => set((state) => ({
        ...activeStateDefaults,
        modelProvider: state.modelProvider,
        pendingTasks: state.pendingTasks,
        currentTaskId: makeTaskId(),
        workflowMode: 'standard',
        documentTitle: title,
        modules: [{
          number: 1,
          title,
          hours,
          content: '',
          activities: '',
          skills: [],
        }],
        totalHours: hours,
        selectedModule: 1,
        totalModulePages,
        targetPages,
        learningOutcomes,
        keywords,
        contentMode,
        currentStep: 'generate',
      })),

      // Edit-doc upload — start a new session with the uploaded content
      // pre-loaded as a single "pending" batch. User can iterate via "Αλλαγές"
      // (revision mode) and then export. Reuses workflowMode='standard'.
      loadDocForEditing: (title, markdown) => set((state) => {
        const estimatedPages = Math.max(1, Math.ceil(markdown.length / 3000));
        const taskId = makeTaskId();
        const pendingBatch: GeneratedContent = {
          moduleNumber: 1,
          batchNumber: 1,
          content: markdown,
          references: [],
          pageCount: estimatedPages,
          status: 'pending',
        };
        return {
          ...activeStateDefaults,
          modelProvider: state.modelProvider,
          pendingTasks: state.pendingTasks,
          currentTaskId: taskId,
          workflowMode: 'standard',
          documentTitle: title,
          modules: [{
            number: 1,
            title,
            hours: 0,
            content: '',
            activities: '',
            skills: [],
          }],
          totalHours: 0,
          selectedModule: 1,
          totalModulePages: estimatedPages,
          targetPages: estimatedPages,
          learningOutcomes: '',
          keywords: '',
          isEditDoc: true,
          contentMode: 'standard',
          generatedBatches: [pendingBatch],
          currentBatch: 2,
          currentStep: 'generate',
        };
      }),

      setSelectedModule: (moduleNumber) => set({ selectedModule: moduleNumber }),

      setCurrentBatch: (batch) => set({ currentBatch: batch }),

      addGeneratedBatch: (batch) => set((state) => {
        const next: AppState = {
          ...state,
          generatedBatches: [...state.generatedBatches, batch],
          currentBatch: state.currentBatch + 1,
        };
        return {
          generatedBatches: next.generatedBatches,
          currentBatch: next.currentBatch,
          pendingTasks: upsertCurrent(next),
        };
      }),

      updateBatchStatus: (moduleNumber, batchNumber, status) => set((state) => {
        const next: AppState = {
          ...state,
          generatedBatches: state.generatedBatches.map((batch) =>
            batch.moduleNumber === moduleNumber && batch.batchNumber === batchNumber
              ? { ...batch, status }
              : batch
          ),
        };
        return {
          generatedBatches: next.generatedBatches,
          pendingTasks: upsertCurrent(next),
        };
      }),

      setIsGenerating: (generating) => set({ isGenerating: generating }),

      setGenerationProgress: (progress) => set({ generationProgress: progress }),

      setContentMode: (mode: 'standard' | 'experimental') => set({ contentMode: mode }),

      setUserInstructions: (instructions) => set({ userInstructions: instructions }),

      setModelProvider: (provider) => set({ modelProvider: provider }),

      addPendingReferences: (refs) => set((state) => ({
        pendingReferences: [...state.pendingReferences, ...refs],
      })),

      approveReference: (index) => set((state) => {
        const ref = state.pendingReferences[index];
        if (!ref) return state;
        return {
          pendingReferences: state.pendingReferences.filter((_, i) => i !== index),
          approvedReferences: [...state.approvedReferences, { ...ref, status: 'approved' as const }],
        };
      }),

      rejectReference: (index) => set((state) => ({
        pendingReferences: state.pendingReferences.filter((_, i) => i !== index),
      })),

      approveAllReferences: () => set((state) => ({
        approvedReferences: [
          ...state.approvedReferences,
          ...state.pendingReferences.map((ref) => ({ ...ref, status: 'approved' as const })),
        ],
        pendingReferences: [],
      })),

      setCurrentStep: (step) => set({ currentStep: step }),

      setError: (error) => set({ error }),

      setSkillReview: (moduleNumber, review) => set((state) => {
        const next: AppState = {
          ...state,
          skillReviews: { ...state.skillReviews, [moduleNumber]: review },
        };
        return {
          skillReviews: next.skillReviews,
          pendingTasks: upsertCurrent(next),
        };
      }),

      clearSkillReview: (moduleNumber) => set((state) => {
        const { [moduleNumber]: _removed, ...rest } = state.skillReviews;
        void _removed;
        return { skillReviews: rest };
      }),

      setIsReviewingSkills: (reviewing) => set({ isReviewingSkills: reviewing }),

      setProductionComplete: (moduleNumber) => set((state) => {
        const newProductionComplete = { ...state.productionComplete, [moduleNumber]: true };
        const allDone = state.modules.length > 0 && state.modules.every((m) => newProductionComplete[m.number]);
        if (allDone && state.currentTaskId) {
          // Whole task finished — drop it from the Εκκρεμότητες list.
          return {
            productionComplete: newProductionComplete,
            pendingTasks: state.pendingTasks.filter((t) => t.id !== state.currentTaskId),
            currentTaskId: null,
          };
        }
        // Still in progress — sync the snapshot.
        const next: AppState = { ...state, productionComplete: newProductionComplete };
        return {
          productionComplete: newProductionComplete,
          pendingTasks: upsertCurrent(next),
        };
      }),

      setModuleSummary: (moduleNumber, summary) => set((state) => {
        const next: AppState = {
          ...state,
          moduleSummaries: { ...state.moduleSummaries, [moduleNumber]: summary },
        };
        return {
          moduleSummaries: next.moduleSummaries,
          pendingTasks: upsertCurrent(next),
        };
      }),

      setIsGeneratingSummary: (generating) => set({ isGeneratingSummary: generating }),

      setTotalModulePages: (pages) => set({ totalModulePages: pages }),

      setTargetPages: (pages) => set({ targetPages: pages }),

      setStructureConfig: (cfg) => set((state) => ({
        structureConfig: { ...state.structureConfig, ...cfg },
      })),

      setModuleHours: (moduleNumber, hours) => set((state) => {
        const modules = state.modules.map((m) =>
          m.number === moduleNumber ? { ...m, hours } : m
        );
        return {
          modules,
          totalHours: modules.reduce((sum, m) => sum + m.hours, 0),
        };
      }),

      setQueueStatus: (position, wait) => set({ queuePosition: position, estimatedWait: wait }),

      clearQueueStatus: () => set({ queuePosition: null, estimatedWait: null }),

      updateBatchContent: (moduleNumber, batchNumber, content, references, pageCount) => set((state) => {
        const next: AppState = {
          ...state,
          generatedBatches: state.generatedBatches.map((batch) =>
            batch.moduleNumber === moduleNumber && batch.batchNumber === batchNumber
              ? { ...batch, content, references, pageCount, status: 'pending' as const }
              : batch
          ),
        };
        return {
          generatedBatches: next.generatedBatches,
          pendingTasks: upsertCurrent(next),
        };
      }),

      loadPendingTask: (id) => set((state) => {
        const task = state.pendingTasks.find((t) => t.id === id);
        if (!task) return state;
        return {
          ...activeStateDefaults,
          modelProvider: task.modelProvider,
          pendingTasks: state.pendingTasks,
          currentTaskId: id,
          workflowMode: task.workflowMode,
          documentTitle: task.title,
          modules: task.modules,
          totalHours: task.totalHours,
          totalModulePages: task.totalModulePages,
          targetPages: task.targetPages,
          learningOutcomes: task.learningOutcomes,
          keywords: task.keywords,
          isEditDoc: task.isEditDoc ?? false,
          structureConfig: task.structureConfig ?? DEFAULT_STRUCTURE_CONFIG,
          selectedModule: task.selectedModule ?? (task.modules[0]?.number ?? null),
          currentBatch: task.currentBatch,
          generatedBatches: task.generatedBatches,
          contentMode: task.contentMode,
          userInstructions: task.userInstructions,
          approvedReferences: task.approvedReferences,
          skillReviews: task.skillReviews,
          productionComplete: task.productionComplete,
          moduleSummaries: task.moduleSummaries,
          currentStep: 'generate',
        };
      }),

      deletePendingTask: (id) => set((state) => ({
        pendingTasks: state.pendingTasks.filter((t) => t.id !== id),
        // Detach if user deleted the active task — Landing reset will follow.
        currentTaskId: state.currentTaskId === id ? null : state.currentTaskId,
      })),

      // Reset only the active session; preserve pendingTasks + provider preference.
      reset: () => set((state) => ({
        ...activeStateDefaults,
        modelProvider: state.modelProvider,
        pendingTasks: state.pendingTasks,
      })),
    }),
    {
      name: 'edu-material-store',
      storage: {
        getItem: (name) => {
          const str = localStorage.getItem(name);
          return str ? JSON.parse(str) : null;
        },
        setItem: (name, value) => {
          localStorage.setItem(name, JSON.stringify(value));
        },
        removeItem: (name) => {
          localStorage.removeItem(name);
        },
      },
      partialize: (state) => ({
        modelProvider: state.modelProvider,
        pendingTasks: state.pendingTasks,
      }),
      // Always boot fresh at the Landing. The user picks a pending task
      // (or starts a new one); active state is loaded explicitly via loadPendingTask.
      onRehydrateStorage: () => (state) => {
        if (!state) return;
        Object.assign(state, activeStateDefaults);
      },
    }
  )
);

// Dev-only escape hatch: lets manual / e2e tests drive store actions directly.
if (typeof window !== 'undefined' && import.meta.env.DEV) {
  (window as unknown as { __store?: typeof useStore }).__store = useStore;
}
