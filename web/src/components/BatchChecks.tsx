import { useState } from 'react';
import { AlertTriangle, BookCheck, ChevronDown, ChevronUp, Languages, Plus, Wand2 } from 'lucide-react';
import type { BibliographyEntryCheck, GeneratedContent, GreekSourceGiven, GreekSourceSuggestion } from '../store/useStore';
import { PAGE_CEILING_MARGIN } from '../utils/pages';
import styles from './BatchChecks.module.css';

const STRUCTURE_LABELS: Record<string, string> = {
  activities: 'Δραστηριότητες',
  self_assessment: 'Ερωτήσεις αυτοαξιολόγησης',
  glossary: 'Γλωσσάρι',
  subsection_keywords: 'Βασικές λέξεις ανά υποενότητα',
  in_text_citations: 'Ενδοκειμενικές αναφορές',
};

const STATUS_LABELS: Record<BibliographyEntryCheck['status'], string> = {
  verified: 'Επαληθεύτηκε',
  doi_corrected: 'Διορθώθηκε το DOI',
  doi_invalid: 'Λάθος DOI (αφαιρέθηκε)',
  unverified: 'Δεν βρέθηκε',
  placeholder: 'Ελλιπή στοιχεία',
  thesis: 'Πτυχιακή/μεταπτυχιακή εργασία',
};

/** Entries whose existence is in doubt: the work was not found, or it is a forbidden type. */
function needsAttention(entry: BibliographyEntryCheck): boolean {
  return !entry.verified || entry.status === 'thesis';
}

/** Real sources cited for sentences they do not support. */
function isUnrelated(entry: BibliographyEntryCheck): boolean {
  return entry.relevance === 'unrelated' && !needsAttention(entry);
}

function removeElementsInstruction(keys: string[]): string {
  const names = keys.map((k) => STRUCTURE_LABELS[k] ?? k).join(', ');
  return (
    `Αφαίρεσε ΠΛΗΡΩΣ τα παρακάτω στοιχεία, που δεν ζητήθηκαν στη δομή του υλικού: ${names}. ` +
    'Μην αλλάξεις τίποτε άλλο στο κείμενο.'
  );
}

function fixBibliographyInstruction(
  entries: BibliographyEntryCheck[],
  unrelated: BibliographyEntryCheck[],
  uncited: string[],
  orphans: string[],
): string {
  const parts: string[] = [];
  if (entries.length > 0) {
    parts.push(
      'Οι παρακάτω βιβλιογραφικές εγγραφές ΔΕΝ επαληθεύτηκαν σε CrossRef/OpenAlex:\n' +
        entries.map((e) => `- ${e.text}`).join('\n') +
        '\nΑντικατάστησέ τες με πραγματικές, καθιερωμένες πηγές που γνωρίζεις με βεβαιότητα ' +
        '(θεμελιώδη έργα, εγχειρίδια, εκθέσεις διεθνών οργανισμών) ή αφαίρεσέ τες, ' +
        'ενημερώνοντας ΚΑΙ τις αντίστοιχες ενδοκειμενικές αναφορές. DOI μόνο αν είσαι απολύτως βέβαιος.',
    );
  }
  const offTopic = unrelated.filter((e) => e.relevance_scope === 'topic');
  const unsupported = unrelated.filter((e) => e.relevance_scope !== 'topic');
  if (offTopic.length > 0) {
    parts.push(
      'Οι παρακάτω πηγές της Βιβλιογραφίας είναι ΕΚΤΟΣ ΘΕΜΑΤΟΣ της ενότητας:\n' +
        offTopic.map((e) => `- ${e.text}`).join('\n') +
        '\nΑντικατάστησέ τες με πραγματικές πηγές που αφορούν το θέμα (μόνο αν τις γνωρίζεις με βεβαιότητα) ' +
        'ή αφαίρεσέ τες από τη Βιβλιογραφία.',
    );
  }
  if (unsupported.length > 0) {
    parts.push(
      'Οι παρακάτω πηγές είναι πραγματικές, αλλά ΔΕΝ τεκμηριώνουν τις προτάσεις που τις επικαλούνται:\n' +
        unsupported
          .map((e) => `- ${e.text}\n  Προτάσεις: ${(e.contexts ?? []).map((c) => `«${c}»`).join(' ')}`)
          .join('\n') +
        '\nΣε κάθε τέτοια πρόταση, αντικατάστησε την παραπομπή με πηγή που πραγματεύεται ΑΜΕΣΑ τον ισχυρισμό ' +
        '(μόνο αν τη γνωρίζεις με βεβαιότητα) ή αφαίρεσε την παραπομπή. Αν η πηγή δεν χρησιμοποιείται πλέον ' +
        'πουθενά, αφαίρεσέ την και από τη Βιβλιογραφία.',
    );
  }
  if (uncited.length > 0) {
    parts.push(
      'Οι παρακάτω εγγραφές της Βιβλιογραφίας ΔΕΝ αναφέρονται πουθενά στο κείμενο:\n' +
        uncited.map((t) => `- ${t}`).join('\n') +
        '\nΓια καθεμία: πρόσθεσε ενδοκειμενική παραπομπή σε σημείο που πράγματι τεκμηριώνει, ' +
        'ή αφαίρεσέ την από τη Βιβλιογραφία.',
    );
  }
  if (orphans.length > 0) {
    parts.push(
      'Οι εξής ενδοκειμενικές αναφορές δεν έχουν εγγραφή στη Βιβλιογραφία: ' +
        orphans.join('; ') +
        '. Πρόσθεσε την πλήρη εγγραφή (αν η πηγή είναι βέβαιη) ή αφαίρεσε την αναφορά.',
    );
  }
  parts.push('Μην αλλάξεις τίποτε άλλο στο κείμενο.');
  return parts.join('\n\n');
}

const KIND_LABELS: Record<GreekSourceSuggestion['kind'], string> = {
  kallipos: 'Σύγγραμμα Κάλλιπος',
  journal: 'Άρθρο περιοδικού',
  conference: 'Πρακτικά συνεδρίου',
  doctoral: 'Διδακτορική διατριβή (ΕΚΤ)',
};

const GREEK_LETTERS = /[\u0370-\u03ff\u1f00-\u1fff]/;

/** APA entries mark italics with *…* (markdown) — render them as italics. */
function withItalics(text: string) {
  return text.split(/(\*[^*]+\*)/g).map((part, i) =>
    part.length > 2 && part.startsWith('*') && part.endsWith('*') ? <em key={i}>{part.slice(1, -1)}</em> : part,
  );
}

/** Did the model use a source it was given? (its DOI or title is in the batch) */
function isUsed(source: GreekSourceGiven, content: string): boolean {
  const text = content.toLowerCase();
  return (!!source.doi && text.includes(source.doi.toLowerCase())) || text.includes(source.title.toLowerCase());
}

interface GreekSourcesProps {
  /** Undefined when the after-batch search did not run or failed. */
  suggestions?: GreekSourceSuggestion[];
  /** Integration mode: sources given to the model before it wrote the batch. */
  given?: GreekSourceGiven[];
  content: string;
  /** Bibliography entries with Greek text (author or title). */
  greekEntries: number;
  totalEntries: number;
  canAdd: boolean;
  disabled: boolean;
  onAdd: (selected: GreekSourceSuggestion[]) => void;
}

/** Verified Greek sources the user may add as EXTRA citations — never replacing one. */
function GreekSources({
  suggestions: maybeSuggestions,
  given,
  content,
  greekEntries,
  totalEntries,
  canAdd,
  disabled,
  onAdd,
}: GreekSourcesProps) {
  const [expanded, setExpanded] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const suggestions = maybeSuggestions ?? [];
  const chosen = suggestions.filter((s) => selected.has(s.key));
  const usedCount = given ? given.filter((g) => isUsed(g, content)).length : 0;

  const toggle = (key: string) => {
    const next = new Set(selected);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    setSelected(next);
  };

  return (
    <div className={styles.greekBox}>
      <button className={styles.biblioToggle} onClick={() => setExpanded(!expanded)}>
        <Languages size={14} />
        <span>
          Ελληνικές πηγές: {greekEntries}/{totalEntries} στη βιβλιογραφία
          {given && ` · δόθηκαν στο μοντέλο ${given.length}, χρησιμοποιήθηκαν ${usedCount}`}
          {maybeSuggestions &&
            (suggestions.length > 0
              ? ` · ${suggestions.length} ${suggestions.length === 1 ? 'προτεινόμενη' : 'προτεινόμενες'}`
              : ' · καμία νέα πρόταση')}
        </span>
        {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
      </button>

      {expanded && (
        <div className={styles.biblioDetails}>
          {given && (
            <>
              <span className={styles.hint}>
                {given.length > 0
                  ? 'Επαληθευμένες ελληνικές πηγές που δόθηκαν στο μοντέλο πριν γράψει (ενσωμάτωση κατά τη συγγραφή):'
                  : 'Ενσωμάτωση κατά τη συγγραφή: δεν βρέθηκαν ελληνικές πηγές άμεσα σχετικές με την ενότητα.'}
              </span>
              {given.map((g) => {
                const used = isUsed(g, content);
                return (
                  <div key={g.key} className={styles.givenSource}>
                    <span className={used ? styles.givenUsed : styles.givenUnused}>
                      {used ? '✓ Χρησιμοποιήθηκε' : '– Δεν χρησιμοποιήθηκε'}
                    </span>
                    <span className={styles.entryText}>{withItalics(g.apa)}</span>
                  </div>
                );
              })}
            </>
          )}
          {!maybeSuggestions ? null : suggestions.length === 0 ? (
            <span className={styles.hint}>
              Δεν βρέθηκαν στον Κάλλιπο και στο ΕΚΤ ελληνικές πηγές που να τεκμηριώνουν άμεσα αυτό το τμήμα.
            </span>
          ) : (
            <span className={styles.hint}>
              Πηγές από τον Κάλλιπο και το ΕΚΤ που τεκμηριώνουν συγκεκριμένα σημεία. Όσες επιλέξετε
              προστίθενται ως επιπλέον παραπομπές: καμία υπάρχουσα πηγή δεν αφαιρείται.
            </span>
          )}
          {suggestions.map((s) => (
            <label key={s.key} className={styles.suggestion}>
              {canAdd && (
                <input
                  type="checkbox"
                  checked={selected.has(s.key)}
                  disabled={disabled}
                  onChange={() => toggle(s.key)}
                />
              )}
              <span className={styles.suggestionBody}>
                <span className={styles.kindBadge}>{KIND_LABELS[s.kind]}</span>
                <span className={styles.entryText}>{withItalics(s.apa)}</span>
                {s.sentence ? (
                  <>
                    <span className={styles.suggestionCitation}>
                      Παραπομπή που θα μπει: <strong>({s.citation})</strong>
                    </span>
                    <span className={styles.suggestionContext}>
                      Στο τέλος της υπάρχουσας πρότασης (δεν αλλάζει): «{s.sentence}»
                    </span>
                  </>
                ) : (
                  <span className={styles.suggestionCitation}>
                    Μόνο εγγραφή στη Βιβλιογραφία{s.section ? ` · Αφορά: ${s.section}` : ''}
                  </span>
                )}
                {s.reason && <span className={styles.suggestionReason}>{s.reason}</span>}
              </span>
            </label>
          ))}
          {canAdd && suggestions.length > 0 && (
            <button
              className={styles.addButton}
              disabled={disabled || chosen.length === 0}
              onClick={() => {
                onAdd(chosen);
                setSelected(new Set());
              }}
            >
              <Plus size={12} />
              Προσθήκη επιλεγμένων{chosen.length > 0 ? ` (${chosen.length})` : ''}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

interface BatchChecksProps {
  batch: GeneratedContent;
  /** Requested pages for a batch; the warning fires above this + PAGE_CEILING_MARGIN. */
  targetPages: number;
  showPageWarning: boolean;
  /** Only pending batches offer one-click fixes. */
  canFix: boolean;
  disabled: boolean;
  onFix: (instruction: string) => void;
  onAddGreekSources: (selected: GreekSourceSuggestion[]) => void;
}

export function BatchChecks({
  batch,
  targetPages,
  showPageWarning,
  canFix,
  disabled,
  onFix,
  onAddGreekSources,
}: BatchChecksProps) {
  const [expanded, setExpanded] = useState(false);
  const check = batch.bibliographyCheck;
  const flagged = check ? check.entries.filter(needsAttention) : [];
  const unrelated = check ? check.entries.filter(isUnrelated) : [];
  const offTopic = unrelated.filter((e) => e.relevance_scope === 'topic');
  const unsupported = unrelated.filter((e) => e.relevance_scope !== 'topic');
  const uncited = check?.uncited_entries ?? [];
  const orphans = check?.orphan_citations ?? [];
  const corrected = check ? check.entries.filter((e) => e.status === 'doi_corrected' || e.status === 'doi_invalid') : [];
  const warnings = batch.structureWarnings ?? [];
  const overLimit = showPageWarning && targetPages > 0 && batch.pageCount > targetPages + PAGE_CEILING_MARGIN;

  const greekSuggestions = batch.greekSuggestions;
  const greekEntries = check ? check.entries.filter((e) => GREEK_LETTERS.test(e.text)).length : 0;

  if (!check && warnings.length === 0 && !overLimit && !greekSuggestions && !batch.greekSourcesGiven) return null;

  const hasIssues = flagged.length > 0 || unrelated.length > 0 || uncited.length > 0 || orphans.length > 0;

  return (
    <div className={styles.checks}>
      {overLimit && (
        <div className={styles.warning}>
          <AlertTriangle size={14} />
          <span>
            Υπέρβαση ορίου σελίδων: ~{batch.pageCount} σελ. (όριο {targetPages + PAGE_CEILING_MARGIN}).
            Μπορείτε να ζητήσετε σύντμηση μέσω «Αλλαγές».
          </span>
        </div>
      )}

      {warnings.length > 0 && (
        <div className={styles.warning}>
          <AlertTriangle size={14} />
          <span>
            Περιέχει στοιχεία που δεν ζητήθηκαν: {warnings.map((k) => STRUCTURE_LABELS[k] ?? k).join(', ')}
          </span>
          {canFix && (
            <button
              className={styles.fixButton}
              disabled={disabled}
              onClick={() => onFix(removeElementsInstruction(warnings))}
            >
              <Wand2 size={12} />
              Αφαίρεση
            </button>
          )}
        </div>
      )}

      {check && (
        <div className={hasIssues ? styles.biblioIssues : styles.biblioOk}>
          <button className={styles.biblioToggle} onClick={() => setExpanded(!expanded)}>
            <BookCheck size={14} />
            <span>
              Βιβλιογραφία: {check.summary.verified}/{check.summary.total} επαληθευμένες
              {unsupported.length > 0 && ` · ${unsupported.length} δεν τεκμηριώνουν το κείμενο`}
              {offTopic.length > 0 && ` · ${offTopic.length} εκτός θέματος`}
              {uncited.length > 0 && ` · ${uncited.length} χωρίς παραπομπή στο κείμενο`}
              {orphans.length > 0 && ` · ${orphans.length} αναφορές χωρίς εγγραφή`}
            </span>
            {(hasIssues || corrected.length > 0) && (expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />)}
          </button>

          {expanded && (
            <div className={styles.biblioDetails}>
              {flagged.map((entry) => (
                <div key={entry.text} className={styles.entry}>
                  <span className={styles.entryStatus}>{STATUS_LABELS[entry.status]}</span>
                  <span className={styles.entryText}>{withItalics(entry.text)}</span>
                </div>
              ))}
              {unrelated.map((entry) => (
                <div key={`rel-${entry.text}`} className={styles.entry}>
                  <span className={styles.entryStatus}>
                    {entry.relevance_scope === 'topic' ? 'Εκτός θέματος ενότητας' : 'Δεν τεκμηριώνει το κείμενο'}
                  </span>
                  <span className={styles.entryText}>{withItalics(entry.text)}</span>
                  {entry.relevance_reason && (
                    <span className={styles.entryReason}>{entry.relevance_reason}</span>
                  )}
                  {(entry.contexts ?? []).map((sentence) => (
                    <span key={sentence} className={styles.entryContext}>«{sentence}»</span>
                  ))}
                </div>
              ))}
              {uncited.map((text) => (
                <div key={`unc-${text}`} className={styles.entry}>
                  <span className={styles.entryStatus}>Χωρίς παραπομπή στο κείμενο</span>
                  <span className={styles.entryText}>{withItalics(text)}</span>
                </div>
              ))}
              {corrected.filter((e) => !needsAttention(e)).map((entry) => (
                <div key={entry.text} className={styles.entry}>
                  <span className={styles.entryStatusFixed}>{STATUS_LABELS[entry.status]}</span>
                  <span className={styles.entryText}>{withItalics(entry.text)}</span>
                </div>
              ))}
              {orphans.length > 0 && (
                <div className={styles.entry}>
                  <span className={styles.entryStatus}>Χωρίς εγγραφή</span>
                  <span className={styles.entryText}>{orphans.join('; ')}</span>
                </div>
              )}
              {canFix && hasIssues && (
                <button
                  className={styles.fixButton}
                  disabled={disabled}
                  onClick={() => onFix(fixBibliographyInstruction(flagged, unrelated, uncited, orphans))}
                >
                  <Wand2 size={12} />
                  Διόρθωση βιβλιογραφίας
                </button>
              )}
            </div>
          )}
        </div>
      )}

      {(greekSuggestions || batch.greekSourcesGiven) && (
        <GreekSources
          suggestions={greekSuggestions}
          given={batch.greekSourcesGiven}
          content={batch.content}
          greekEntries={greekEntries}
          totalEntries={check?.summary.total ?? 0}
          canAdd={canFix}
          disabled={disabled}
          onAdd={onAddGreekSources}
        />
      )}
    </div>
  );
}
