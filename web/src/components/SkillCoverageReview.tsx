import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  CheckCircle2,
  AlertCircle,
  XCircle,
  ChevronDown,
  ChevronUp,
  RefreshCw,
  FileText,
  Target,
} from 'lucide-react';
import type { SkillCoverageReview as SkillCoverageReviewType, SkillAnalysis } from '../store/useStore';
import styles from './SkillCoverageReview.module.css';

interface Props {
  review: SkillCoverageReviewType;
  onRegenerateReview: () => void;
  onExport: () => void;
  isRegenerating?: boolean;
}

export function SkillCoverageReview({ review, onRegenerateReview, onExport, isRegenerating }: Props) {
  const [expandedSkills, setExpandedSkills] = useState<Set<string>>(new Set());

  const toggleSkill = (skillCode: string) => {
    setExpandedSkills(prev => {
      const next = new Set(prev);
      if (next.has(skillCode)) {
        next.delete(skillCode);
      } else {
        next.add(skillCode);
      }
      return next;
    });
  };

  const getCoverageIcon = (level: SkillAnalysis['coverageLevel']) => {
    switch (level) {
      case 'full':
        return <CheckCircle2 className={styles.iconFull} size={20} />;
      case 'partial':
        return <AlertCircle className={styles.iconPartial} size={20} />;
      case 'missing':
        return <XCircle className={styles.iconMissing} size={20} />;
    }
  };

  const getCoverageLabel = (level: SkillAnalysis['coverageLevel']) => {
    switch (level) {
      case 'full':
        return 'Πλήρης';
      case 'partial':
        return 'Μερική';
      case 'missing':
        return 'Απούσα';
    }
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className={styles.container}
    >
      <div className={styles.header}>
        <div className={styles.headerIcon}>
          <Target size={24} />
        </div>
        <div className={styles.headerText}>
          <h2 className={styles.title}>Ανάλυση Κάλυψης Δεξιοτήτων ESCO</h2>
          <p className={styles.subtitle}>Ενότητα {review.moduleNumber}</p>
        </div>
      </div>

      <div className={styles.overviewCard}>
        <div className={styles.coverageMain}>
          <div className={styles.percentageWrapper}>
            <span className={styles.percentage}>{review.coveragePercentage}%</span>
            <span className={styles.percentageLabel}>Συνολική Κάλυψη</span>
          </div>
          <div className={styles.progressBar}>
            <div
              className={styles.progressFill}
              style={{ width: `${review.coveragePercentage}%` }}
            />
          </div>
        </div>

        <div className={styles.statsRow}>
          <div className={`${styles.statItem} ${styles.statFull}`}>
            <CheckCircle2 size={18} />
            <span className={styles.statValue}>{review.coveredFully}</span>
            <span className={styles.statLabel}>Πλήρης</span>
          </div>
          <div className={`${styles.statItem} ${styles.statPartial}`}>
            <AlertCircle size={18} />
            <span className={styles.statValue}>{review.coveredPartially}</span>
            <span className={styles.statLabel}>Μερική</span>
          </div>
          <div className={`${styles.statItem} ${styles.statMissing}`}>
            <XCircle size={18} />
            <span className={styles.statValue}>{review.missing}</span>
            <span className={styles.statLabel}>Απούσα</span>
          </div>
        </div>
      </div>

      <div className={styles.skillsList}>
        <h3 className={styles.sectionTitle}>Ανάλυση ανά Δεξιότητα</h3>
        {review.skillAnalysis.map((skill) => (
          <div
            key={skill.skillCode}
            className={`${styles.skillCard} ${styles[`skill${skill.coverageLevel.charAt(0).toUpperCase() + skill.coverageLevel.slice(1)}`]}`}
          >
            <button
              className={styles.skillHeader}
              onClick={() => toggleSkill(skill.skillCode)}
            >
              <div className={styles.skillInfo}>
                {getCoverageIcon(skill.coverageLevel)}
                <div className={styles.skillText}>
                  <span className={styles.skillName}>{skill.skillName}</span>
                  <span className={styles.skillType}>
                    {skill.skillType === 'essential' ? '[Απαραίτητη]' : '[Προαιρετική]'}
                  </span>
                </div>
              </div>
              <div className={styles.skillMeta}>
                <span className={`${styles.coverageBadge} ${styles[`badge${skill.coverageLevel.charAt(0).toUpperCase() + skill.coverageLevel.slice(1)}`]}`}>
                  {getCoverageLabel(skill.coverageLevel)}
                </span>
                {expandedSkills.has(skill.skillCode) ? (
                  <ChevronUp size={20} />
                ) : (
                  <ChevronDown size={20} />
                )}
              </div>
            </button>

            <AnimatePresence>
              {expandedSkills.has(skill.skillCode) && (
                <motion.div
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: 'auto', opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  className={styles.skillDetails}
                >
                  {skill.contentSections.length > 0 && (
                    <div className={styles.detailSection}>
                      <span className={styles.detailLabel}>Ενότητες που καλύπτουν:</span>
                      <ul className={styles.sectionsList}>
                        {skill.contentSections.map((section, i) => (
                          <li key={i}>{section}</li>
                        ))}
                      </ul>
                    </div>
                  )}

                  {skill.evidence.length > 0 && (
                    <div className={styles.detailSection}>
                      <span className={styles.detailLabel}>Αποδεικτικά στοιχεία:</span>
                      <div className={styles.evidenceList}>
                        {skill.evidence.map((quote, i) => (
                          <blockquote key={i} className={styles.evidenceQuote}>
                            "{quote}"
                          </blockquote>
                        ))}
                      </div>
                    </div>
                  )}

                  {skill.notes && (
                    <div className={styles.detailSection}>
                      <span className={styles.detailLabel}>Σημειώσεις:</span>
                      <p className={styles.notes}>{skill.notes}</p>
                    </div>
                  )}
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        ))}
      </div>

      {review.overallAssessment && (
        <div className={styles.assessmentCard}>
          <h3 className={styles.sectionTitle}>Συνολική Αξιολόγηση</h3>
          <p className={styles.assessmentText}>{review.overallAssessment}</p>
        </div>
      )}

      {review.recommendations.length > 0 && (
        <div className={styles.recommendationsCard}>
          <h3 className={styles.sectionTitle}>Προτάσεις Βελτίωσης</h3>
          <ul className={styles.recommendationsList}>
            {review.recommendations.map((rec, i) => (
              <li key={i}>{rec}</li>
            ))}
          </ul>
        </div>
      )}

      <div className={styles.actions}>
        <button
          className={styles.secondaryButton}
          onClick={onRegenerateReview}
          disabled={isRegenerating}
        >
          <RefreshCw size={18} className={isRegenerating ? styles.spinning : ''} />
          {isRegenerating ? 'Ανάλυση...' : 'Επανάληψη Ανάλυσης'}
        </button>
        <button className={styles.primaryButton} onClick={onExport}>
          <FileText size={18} />
          Εξαγωγή σε Word
        </button>
      </div>
    </motion.div>
  );
}
