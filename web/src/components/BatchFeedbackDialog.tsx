import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import styles from './BatchFeedbackDialog.module.css';

interface BatchFeedbackDialogProps {
  isOpen: boolean;
  batchNumber: number;
  batchPreview: string;
  onSubmit: (feedback: string) => void;
  onCancel: () => void;
}

export function BatchFeedbackDialog({
  isOpen,
  batchNumber,
  batchPreview,
  onSubmit,
  onCancel,
}: BatchFeedbackDialogProps) {
  const [feedback, setFeedback] = useState('');

  const handleSubmit = () => {
    if (feedback.trim()) {
      onSubmit(feedback.trim());
      setFeedback('');
    }
  };

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          className={styles.overlay}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={onCancel}
        >
          <motion.div
            className={styles.dialog}
            initial={{ opacity: 0, scale: 0.95, y: 20 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: 20 }}
            transition={{ type: 'spring', damping: 25, stiffness: 300 }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className={styles.header}>
              <h3 className={styles.title}>
                Αίτημα αλλαγών - Τμήμα {batchNumber}
              </h3>
            </div>

            <div className={styles.body}>
              <div className={styles.previewLabel}>Προεπισκόπηση περιεχομένου</div>
              <div className={styles.preview}>
                {batchPreview}...
              </div>

              <label className={styles.feedbackLabel}>
                Περιγράψτε τις αλλαγές
              </label>
              <textarea
                className={styles.textarea}
                value={feedback}
                onChange={(e) => setFeedback(e.target.value)}
                placeholder="π.χ. Κρατήστε την υποενότητα 1.1 αλλά αλλάξτε την 1.2 με περισσότερα παραδείγματα..."
                autoFocus
              />
            </div>

            <div className={styles.footer}>
              <button className={styles.cancelButton} onClick={onCancel}>
                Ακύρωση
              </button>
              <button
                className={styles.submitButton}
                onClick={handleSubmit}
                disabled={!feedback.trim()}
              >
                Αποστολή
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
