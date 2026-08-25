import type { ESCOSkill, Module } from '../store/useStore';

const BOOTSTRAP_STORAGE_KEY = 'edu-material-platform-start';

interface PlatformStartPayload {
  proposal: unknown;
  moduleIndex: number | null;
  clientContext: unknown | null;
}

export interface NormalizedPlatformStart {
  title: string;
  modules: Module[];
  totalHours: number;
  selectedModule: number | null;
  clientContext: unknown | null;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function stringFrom(obj: Record<string, unknown>, keys: string[], fallback = ''): string {
  for (const key of keys) {
    const value = obj[key];
    if (typeof value === 'string' && value.trim()) return value.trim();
    if (typeof value === 'number' && Number.isFinite(value)) return String(value);
  }
  return fallback;
}

function numberFrom(obj: Record<string, unknown>, keys: string[], fallback = 0): number {
  for (const key of keys) {
    const value = obj[key];
    if (typeof value === 'number' && Number.isFinite(value)) return value;
    if (typeof value === 'string' && value.trim()) {
      const parsed = Number.parseInt(value, 10);
      if (Number.isFinite(parsed)) return parsed;
    }
  }
  return fallback;
}

function normalizeSkill(raw: unknown): ESCOSkill | null {
  if (typeof raw === 'string' && raw.trim()) {
    return { code: raw.trim(), name: raw.trim(), type: 'essential' };
  }
  if (!isObject(raw)) return null;

  const name = stringFrom(raw, [
    'name',
    'preferredLabel',
    'preferred_label_el',
    'preferred_label_en',
    'label',
    'title',
    'skill',
    'code',
    'uri',
    'concept_uri',
  ]);
  if (!name) return null;

  const code = stringFrom(raw, ['code', 'uri', 'concept_uri', 'id'], name);
  const rawType = stringFrom(raw, ['type', 'skillType', 'skill_type', 'relationType', 'relation_type'], 'essential').toLowerCase();
  const type: ESCOSkill['type'] = rawType.includes('optional') ? 'optional' : 'essential';
  return { code, name, type };
}

function flattenChapterSkills(chapter: Record<string, unknown>): unknown[] {
  const sections = chapter.sections;
  if (!Array.isArray(sections)) return [];
  return sections.flatMap((section) => {
    if (!isObject(section)) return [];
    const matched = section.matched_skills;
    return Array.isArray(matched) ? matched : [];
  });
}

function rawSkillsForModule(raw: Record<string, unknown>): unknown[] {
  const candidates = [
    raw.skills,
    raw.esco_skills_used,
    raw.esco_skills,
    raw.matched_skills,
    raw.primary_competences,
  ];
  for (const candidate of candidates) {
    if (Array.isArray(candidate)) return candidate;
  }
  return flattenChapterSkills(raw);
}

function normalizeModule(raw: unknown, index: number): Module {
  const obj = isObject(raw) ? raw : {};
  const number = numberFrom(obj, ['number', 'module_number', 'chapter_number', 'order'], index + 1);
  const title = stringFrom(obj, ['title', 'module_title', 'chapter_title', 'name', 'unit_title'], `Ενότητα ${number}`);
  const hours = Math.max(1, numberFrom(obj, ['hours', 'total_hours', 'duration'], 1));
  const content = stringFrom(obj, ['content', 'description', 'scope'], '');
  const activities = stringFrom(obj, ['activities', 'suggested_activity', 'activity'], '');
  const skills = rawSkillsForModule(obj)
    .map(normalizeSkill)
    .filter((skill): skill is ESCOSkill => Boolean(skill));

  return { number, title, hours, content, activities, skills };
}

function proposalModules(proposal: Record<string, unknown>): unknown[] {
  if (Array.isArray(proposal.modules)) return proposal.modules;
  if (Array.isArray(proposal.chapters)) return proposal.chapters;
  return [];
}

export function consumePlatformStart(): NormalizedPlatformStart | null {
  const raw = sessionStorage.getItem(BOOTSTRAP_STORAGE_KEY);
  if (!raw) return null;
  sessionStorage.removeItem(BOOTSTRAP_STORAGE_KEY);

  const payload = JSON.parse(raw) as PlatformStartPayload;
  if (!isObject(payload.proposal)) {
    throw new Error('Το proposal που στάλθηκε από την πλατφόρμα δεν είναι JSON object.');
  }

  const proposal = payload.proposal;
  const allModules = proposalModules(proposal);
  if (allModules.length === 0) {
    throw new Error('Το proposal δεν περιέχει modules.');
  }

  const moduleIndex = typeof payload.moduleIndex === 'number' ? payload.moduleIndex : null;
  if (moduleIndex !== null && (moduleIndex < 0 || moduleIndex >= allModules.length)) {
    throw new Error('Το moduleIndex είναι εκτός ορίων.');
  }

  const selectedRawModules = moduleIndex === null ? allModules : [allModules[moduleIndex]];
  const modules = selectedRawModules.map((module, index) => normalizeModule(module, moduleIndex ?? index));
  const totalHours = numberFrom(proposal, ['totalHours', 'total_hours'], modules.reduce((sum, module) => sum + module.hours, 0));
  const title = stringFrom(proposal, ['documentTitle', 'course_title', 'title', 'name'], 'Εκπαιδευτικός Σχεδιασμός');
  const selectedModule = moduleIndex === null ? null : modules[0]?.number ?? null;

  return {
    title,
    modules,
    totalHours,
    selectedModule,
    clientContext: payload.clientContext ?? null,
  };
}
