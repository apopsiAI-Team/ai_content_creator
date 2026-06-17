import mammoth from 'mammoth';
import type { Module, ESCOSkill } from '../store/useStore';

interface ParsedDocument {
  title: string;
  totalHours: number;
  modules: Module[];
}

export async function parseDocx(file: File): Promise<ParsedDocument> {
  const arrayBuffer = await file.arrayBuffer();
  const result = await mammoth.extractRawText({ arrayBuffer });
  const text = result.value;

  return parseEducationalDesign(text);
}

function parseEducationalDesign(text: string): ParsedDocument {
  const lines = text.split('\n').filter(line => line.trim());

  // Extract occupation title - look for "στελέχη" or "Ειδικός" pattern
  let title = '';

  // Try to find occupation pattern like "στελέχη διοικητικής υποστήριξης"
  const occupationMatch = text.match(/(?:για\s+)?(στελέχη[^,\n.]+|Ειδικός[^,\n.]+)/i);
  if (occupationMatch) {
    const occupation = occupationMatch[1].trim();
    title = `Πρόγραμμα επαγγελματικής κατάρτισης για ${occupation}`;
  } else {
    // Fallback: look for first line with "Ειδικός" or "Πρόγραμμα"
    for (const line of lines) {
      if (line.includes('Ειδικός') || line.includes('Πρόγραμμα')) {
        // Keep only the short version
        const shortMatch = line.match(/(Ειδικός[^,\n]+|Πρόγραμμα[^για]*για[^,\n]+)/i);
        title = shortMatch ? shortMatch[1].trim() : line.trim().substring(0, 80);
        break;
      }
    }
  }

  if (!title && lines.length > 0) {
    title = lines[0].trim().substring(0, 80);
  }

  // Extract total hours
  let totalHours = 0;
  const hoursMatch = text.match(/Συνολικές\s*[Ωώ]ρες[:\s]*(\d+)/i);
  if (hoursMatch) {
    totalHours = parseInt(hoursMatch[1], 10);
  }

  // Extract modules
  const modules = extractModules(text);

  return {
    title,
    totalHours: totalHours || modules.reduce((sum, m) => sum + m.hours, 0),
    modules,
  };
}

function extractModules(text: string): Module[] {
  const modules: Module[] = [];

  // Find "Θεματικές Ενότητες" section and work from there
  const thematicStart = text.indexOf('Θεματικές Ενότητες');
  const workingText = thematicStart >= 0 ? text.substring(thematicStart) : text;

  // Split by module pattern: "NUMBER. TITLE" followed by hours on next line
  // Pattern matches: "1. Title\n⏱️ 15 ώρες" or "1. Title\n15 ώρες"
  const modulePattern = /(\d+)\.\s+([^\n]+)\n[^\d]*?(\d+)\s*ώρες/g;
  const matches = [...workingText.matchAll(modulePattern)];

  if (matches.length === 0) {
    return extractModulesAlternative(text);
  }

  for (let i = 0; i < matches.length; i++) {
    const match = matches[i];
    const number = parseInt(match[1], 10);
    const title = match[2].trim();
    const hours = parseInt(match[3], 10);

    // Get section text (from this match to next match or end)
    const startIdx = match.index! + match[0].length;
    const endIdx = matches[i + 1]?.index ?? workingText.length;
    const sectionText = workingText.substring(startIdx, endIdx);

    // Extract content
    const contentMatch = sectionText.match(/Περιεχόμενο:\s*([^🎮Δ]+?)(?=Δραστηριότητες|ESCO|$)/s);
    const content = contentMatch ? contentMatch[1].trim() : '';

    // Extract activities
    const activitiesMatch = sectionText.match(/Δραστηριότητες:\s*([^🎯E]+?)(?=ESCO|$)/s);
    const activities = activitiesMatch ? activitiesMatch[1].trim() : '';

    // Extract ESCO skills
    const skillsMatch = sectionText.match(/ESCO\s*Skills[^:]*:\s*([\s\S]*?)(?=\d+\.|$)/);
    const skillsText = skillsMatch ? skillsMatch[1] : '';
    const skills = parseSkills(skillsText);

    modules.push({
      number,
      title,
      hours,
      content,
      activities,
      skills,
    });
  }

  return modules.sort((a, b) => a.number - b.number);
}

function extractModulesAlternative(text: string): Module[] {
  const modules: Module[] = [];

  // Look for numbered sections with hours mentioned
  const lines = text.split('\n');
  let currentModule: Partial<Module> | null = null;
  let contentBuffer = '';

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    const nextLine = lines[i + 1]?.trim() || '';

    // Check for module header patterns - MUST have hours mentioned on same or next line
    // Pattern: "1. Title - 8 ώρες" or "1. Title" followed by "8 ώρες" on next line
    const moduleMatch = line.match(/^(\d+)\.\s+(.+?)(?:\s*[-–]\s*(\d+)\s*ώρες)?$/);

    if (moduleMatch) {
      const hasHoursInLine = moduleMatch[3] !== undefined;
      const hasHoursInNextLine = nextLine.match(/^\s*(\d+)\s*ώρες/i);
      const hasModuleKeyword = line.toLowerCase().includes('ενότητα') ||
                               line.toLowerCase().includes('θεματική') ||
                               line.toLowerCase().includes('κεφάλαιο');

      // Only consider it a module if it has hours OR module keywords
      if (!hasHoursInLine && !hasHoursInNextLine && !hasModuleKeyword) {
        // This is likely a learning objective, not a module - skip
        if (currentModule) {
          contentBuffer += line + '\n';
        }
        continue;
      }

      // Save previous module
      if (currentModule && currentModule.number && currentModule.title) {
        currentModule.content = contentBuffer.trim();
        modules.push(currentModule as Module);
      }

      const hours = hasHoursInLine
        ? parseInt(moduleMatch[3], 10)
        : (hasHoursInNextLine ? parseInt(hasHoursInNextLine[1], 10) : 0);

      currentModule = {
        number: parseInt(moduleMatch[1], 10),
        title: moduleMatch[2].trim(),
        hours,
        content: '',
        activities: '',
        skills: [],
      };
      contentBuffer = '';
      continue;
    }

    // Check for hours pattern
    if (currentModule && line.match(/(\d+)\s*ώρες/i)) {
      const hoursMatch = line.match(/(\d+)\s*ώρες/i);
      if (hoursMatch && !currentModule.hours) {
        currentModule.hours = parseInt(hoursMatch[1], 10);
      }
    }

    // Check for ESCO skills
    if (currentModule && (line.includes('ESCO') || line.includes('[E]') || line.includes('[O]'))) {
      const skills = parseSkills(line);
      currentModule.skills = [...(currentModule.skills || []), ...skills];
    }

    // Accumulate content
    if (currentModule) {
      contentBuffer += line + '\n';
    }
  }

  // Save last module
  if (currentModule && currentModule.number && currentModule.title) {
    currentModule.content = contentBuffer.trim();
    modules.push(currentModule as Module);
  }

  return modules;
}

function parseSkills(text: string): ESCOSkill[] {
  const skills: ESCOSkill[] = [];

  // Pattern for skills like "διοίκηση γραφείου [E]" or "διαχείριση συμβολαίων [O]"
  const skillPattern = /([^[\]\n,]+)\s*\[([EO])\]/g;
  let match;

  while ((match = skillPattern.exec(text)) !== null) {
    const name = match[1].trim();
    const type = match[2] === 'E' ? 'essential' : 'optional';

    if (name && name.length > 2) {
      skills.push({
        code: generateSkillCode(name),
        name,
        type,
      });
    }
  }

  // If no bracketed skills found, try comma-separated list
  if (skills.length === 0) {
    const parts = text.split(/[,\n]/);
    for (const part of parts) {
      const name = part.trim();
      if (name && name.length > 3 && !name.match(/^\d/) && !name.includes('ESCO')) {
        skills.push({
          code: generateSkillCode(name),
          name,
          type: 'essential',
        });
      }
    }
  }

  return skills;
}

function generateSkillCode(name: string): string {
  // Generate a simple code from the skill name
  return name
    .toLowerCase()
    .replace(/[^\w\sα-ωά-ώ]/g, '')
    .split(/\s+/)
    .map(word => word.substring(0, 3))
    .join('-')
    .substring(0, 20);
}

export function formatModuleForGeneration(module: Module): string {
  return `
## Ενότητα ${module.number}: ${module.title}

**Διάρκεια:** ${module.hours} ώρες

**Περιεχόμενο:**
${module.content}

**Δραστηριότητες:**
${module.activities}

**Δεξιότητες ESCO:**
${module.skills.map(s => `- ${s.name} (${s.type === 'essential' ? 'Απαραίτητη' : 'Προαιρετική'})`).join('\n')}
`.trim();
}
