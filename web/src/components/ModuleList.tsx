import { motion } from 'framer-motion';
import { Clock, BookOpen, ChevronRight, Settings } from 'lucide-react';
import { useStore } from '../store/useStore';
import styles from './ModuleList.module.css';

export function ModuleList() {
  const {
    documentTitle,
    modules,
    totalHours,
    selectedModule,
    setSelectedModule,
    setCurrentStep,
    setModuleHours,
    totalModulePages,
    setTotalModulePages,
    targetPages,
    setTargetPages,
  } = useStore();


  const handleModuleSelect = (moduleNumber: number) => {
    setSelectedModule(moduleNumber);
  };

  const handleStartGeneration = () => {
    if (selectedModule) {
      setCurrentStep('generate');
    }
  };

  const estimatedBatches = totalModulePages > 0 && targetPages > 0
    ? Math.ceil(totalModulePages / targetPages)
    : 1;

  return (
    <div className={styles.container}>
      <motion.div
        initial={{ opacity: 0, y: -20 }}
        animate={{ opacity: 1, y: 0 }}
        className={styles.header}
      >
        <div className={styles.brandSection}>
          <div className={styles.logo}>
            <div className={styles.logoIconWrapper}>
              <div className={styles.logoBook}></div>
              <div className={styles.logoBook}></div>
              <div className={styles.logoBook}></div>
            </div>
            <div className={styles.logoTextGroup}>
              <span className={styles.logoText}>APOPSI</span>
              <span className={styles.logoSubtext}>e-learning</span>
            </div>
          </div>
        </div>
        <div className={styles.titleSection}>
          <h1 className={styles.title}>{documentTitle}</h1>
          <div className={styles.meta}>
            <span className={styles.metaItem}>
              <BookOpen size={18} />
              {modules.length} Ενότητες
            </span>
            <span className={styles.metaItem}>
              <Clock size={18} />
              {totalHours} Ώρες
            </span>
          </div>
        </div>
      </motion.div>

      <div className={styles.content}>
        <motion.div
          initial={{ opacity: 0, x: -20 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ delay: 0.1 }}
          className={styles.modulesSection}
        >
          <h2 className={styles.sectionTitle}>Επιλέξτε Ενότητα</h2>
          <div className={styles.moduleGrid}>
            {modules.map((module, index) => (
              <motion.button
                key={module.number}
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.05 * index }}
                className={`${styles.moduleCard} ${selectedModule === module.number ? styles.selected : ''}`}
                onClick={() => handleModuleSelect(module.number)}
              >
                <div className={styles.moduleNumber}>{module.number}</div>
                <div className={styles.moduleContent}>
                  <h3 className={styles.moduleTitle}>{module.title}</h3>
                  <div className={styles.moduleInfo}>
                    <span className={styles.moduleHours} onClick={(e) => e.stopPropagation()}>
                      <Clock size={14} />
                      <input
                        type="number"
                        value={module.hours}
                        onChange={(e) => {
                          e.stopPropagation();
                          setModuleHours(module.number, Math.max(1, parseInt(e.target.value) || 1));
                        }}
                        onClick={(e) => e.stopPropagation()}
                        min={1}
                        className={styles.hoursInput}
                      />
                      ώρες
                    </span>
                  </div>
                </div>
                <ChevronRight className={styles.moduleArrow} size={24} />
              </motion.button>
            ))}
          </div>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, x: 20 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ delay: 0.2 }}
          className={styles.settingsSection}
        >
          <div className={styles.settingsCard}>
            <h2 className={styles.sectionTitle}>
              <Settings size={20} />
              Ρυθμίσεις Δημιουργίας
            </h2>

            <div className={styles.settingGroup}>
              <label className={styles.settingLabel}>
                Σελίδες Ενότητας
              </label>
              <div className={styles.pageInputRow}>
                <div className={styles.pageInputGroup}>
                  <label className={styles.pageInputLabel}>Συνολικές σελίδες</label>
                  <input
                    type="number"
                    value={totalModulePages || ''}
                    onChange={(e) => setTotalModulePages(Math.max(1, parseInt(e.target.value) || 0))}
                    min={1}
                    placeholder="π.χ. 55"
                    className={styles.settingInput}
                  />
                </div>
                <div className={styles.pageInputGroup}>
                  <label className={styles.pageInputLabel}>Σελίδες / τμήμα</label>
                  <input
                    type="number"
                    value={targetPages || ''}
                    onChange={(e) => setTargetPages(Math.max(1, parseInt(e.target.value) || 0))}
                    min={1}
                    max={totalModulePages || undefined}
                    placeholder="π.χ. 20"
                    className={styles.settingInput}
                  />
                </div>
              </div>
              {totalModulePages > 0 && targetPages > 0 && (
                <span className={styles.settingHint}>
                  Εκτιμώμενα τμήματα: {estimatedBatches}
                </span>
              )}
            </div>

            <div className={styles.selectedInfo}>
              {selectedModule ? (
                <div className={styles.selectedModule}>
                  <span className={styles.selectedLabel}>Επιλεγμένη ενότητα:</span>
                  <span className={styles.selectedTitle}>
                    {modules.find(m => m.number === selectedModule)?.title}
                  </span>
                </div>
              ) : (
                <span className={styles.selectedHint}>Επιλέξτε ενότητα από αριστερά</span>
              )}

              <button
                className={styles.startButton}
                onClick={handleStartGeneration}
                disabled={!selectedModule}
              >
                Έναρξη Δημιουργίας
                <ChevronRight size={20} />
              </button>
            </div>
          </div>
        </motion.div>
      </div>
    </div>
  );
}
