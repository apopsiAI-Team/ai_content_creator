import { useEffect } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { useStore } from './store/useStore';
import { LandingPage } from './components/LandingPage';
import { ModuleList } from './components/ModuleList';
import { ContentGenerator } from './components/ContentGenerator';
import { initializeAuthFromUrl } from './services/api';
import { consumePlatformStart } from './services/platformStart';
import './App.css';

function App() {
  const { currentStep, error, setError, setPlatformDocumentData } = useStore();

  useEffect(() => {
    initializeAuthFromUrl();
    try {
      const platformStart = consumePlatformStart();
      if (platformStart) {
        setPlatformDocumentData(
          platformStart.title,
          platformStart.modules,
          platformStart.totalHours,
          platformStart.selectedModule,
          platformStart.clientContext
        );
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Αποτυχία φόρτωσης δεδομένων από την πλατφόρμα.';
      setError(message);
    }
  }, [setError, setPlatformDocumentData]);

  const pageVariants = {
    initial: { opacity: 0, x: 20 },
    animate: { opacity: 1, x: 0 },
    exit: { opacity: 0, x: -20 },
  };

  return (
    <div className="app">
      <AnimatePresence mode="wait">
        {error && (
          <motion.div
            initial={{ opacity: 0, y: -50 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -50 }}
            style={{
              position: 'fixed',
              top: '1rem',
              left: '50%',
              transform: 'translateX(-50%)',
              background: 'var(--error-red)',
              color: 'white',
              padding: '0.75rem 1.5rem',
              borderRadius: '8px',
              zIndex: 1000,
              display: 'flex',
              alignItems: 'center',
              gap: '1rem',
              boxShadow: 'var(--shadow-elevated)',
            }}
          >
            <span>{error}</span>
            <button
              onClick={() => setError(null)}
              style={{
                background: 'rgba(255,255,255,0.2)',
                border: 'none',
                borderRadius: '4px',
                padding: '0.25rem 0.5rem',
                color: 'white',
                cursor: 'pointer',
              }}
            >
              ✕
            </button>
          </motion.div>
        )}
      </AnimatePresence>

      <AnimatePresence mode="wait">
        {currentStep === 'upload' && (
          <motion.div
            key="upload"
            variants={pageVariants}
            initial="initial"
            animate="animate"
            exit="exit"
            transition={{ duration: 0.3 }}
          >
            <LandingPage />
          </motion.div>
        )}

        {currentStep === 'modules' && (
          <motion.div
            key="modules"
            variants={pageVariants}
            initial="initial"
            animate="animate"
            exit="exit"
            transition={{ duration: 0.3 }}
          >
            <ModuleList />
          </motion.div>
        )}

        {currentStep === 'generate' && (
          <motion.div
            key="generate"
            variants={pageVariants}
            initial="initial"
            animate="animate"
            exit="exit"
            transition={{ duration: 0.3 }}
            style={{ minHeight: '100vh' }}
          >
            <ContentGenerator />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

export default App;
