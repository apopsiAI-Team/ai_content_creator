import { useState } from 'react';
import { AlertTriangle, BookCheck, ChevronDown, ChevronUp, Wand2 } from 'lucide-react';
import type { BibliographyEntryCheck, GeneratedContent } from '../store/useStore';
import { PAGE_CEILING_MARGIN } from '../utils/pages';
import styles from './BatchChecks.module.css';

const STRUCTURE_LABELS: Record<string, string> = {
  activities: 'Δραστηριότητες',
  self_assessment: 'Ερωτήσεις αυτοαξιολόγησης',
  glossary: 'Γλωσσάρι',
  subsection_keywords: 'Βασικές λέξεις ανά υποενότητα',
};

const STATUS_LABELS: Record<BibliographyEntryCheck['status'], string> = {
  verified: 'Επαληθεύτηκε',
  doi_corrected: 'Διορθώθηκε το DOI',
  doi_invalid: 'Λάθος DOI (αφαιρέθηκε)',
  unverified: 'Δεν βρέθηκε',
  placeholder: 'Ελλιπή στοιχεία',
  thesis: 'Πτυχιακή/μεταπτυχιακή εργασία',
};

/** Entries the user should look at: the work was not found, or it is a forbidden type. */
function needsAttention(entry: BibliographyEntryCheck): boolean {
  return !entry.verified || entry.status === 'thesis';
}

function removeElementsInstruction(keys: string[]): string {
  const names = keys.map((k) => STRUCTURE_LABELS[k] ?? k).join(', ');
  return (
    `Αφαίρεσε ΠΛΗΡΩΣ τα παρακάτω στοιχεία, που δεν ζητήθηκαν στη δομή του υλικού: ${names}. ` +
    'Μην αλλάξεις τίποτε άλλο στο κείμενο.'
  );
}

function replaceReferencesInstruction(entries: BibliographyEntryCheck[], orphans: string[]): string {
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

interface BatchChecksProps {
  batch: GeneratedContent;
  /** Requested pages for a batch; the warning fires above this + PAGE_CEILING_MARGIN. */
  targetPages: number;
  showPageWarning: boolean;
  /** Only pending batches offer one-click fixes. */
  canFix: boolean;
  disabled: boolean;
  onFix: (instruction: string) => void;
}

export function BatchChecks({ batch, targetPages, showPageWarning, canFix, disabled, onFix }: BatchChecksProps) {
  const [expanded, setExpanded] = useState(false);
  const check = batch.bibliographyCheck;
  const flagged = check ? check.entries.filter(needsAttention) : [];
  const orphans = check?.orphan_citations ?? [];
  const corrected = check ? check.entries.filter((e) => e.status === 'doi_corrected' || e.status === 'doi_invalid') : [];
  const warnings = batch.structureWarnings ?? [];
  const overLimit = showPageWarning && targetPages > 0 && batch.pageCount > targetPages + PAGE_CEILING_MARGIN;

  if (!check && warnings.length === 0 && !overLimit) return null;

  const hasIssues = flagged.length > 0 || orphans.length > 0;

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
              {orphans.length > 0 && ` · ${orphans.length} αναφορές χωρίς εγγραφή`}
            </span>
            {(hasIssues || corrected.length > 0) && (expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />)}
          </button>

          {expanded && (
            <div className={styles.biblioDetails}>
              {flagged.map((entry) => (
                <div key={entry.text} className={styles.entry}>
                  <span className={styles.entryStatus}>{STATUS_LABELS[entry.status]}</span>
                  <span className={styles.entryText}>{entry.text}</span>
                </div>
              ))}
              {corrected.filter((e) => !needsAttention(e)).map((entry) => (
                <div key={entry.text} className={styles.entry}>
                  <span className={styles.entryStatusFixed}>{STATUS_LABELS[entry.status]}</span>
                  <span className={styles.entryText}>{entry.text}</span>
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
                  onClick={() => onFix(replaceReferencesInstruction(flagged, orphans))}
                >
                  <Wand2 size={12} />
                  Αντικατάσταση μη επαληθευμένων
                </button>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
