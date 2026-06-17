use futures::stream::StreamExt;
use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook_tokio::Signals;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};
use tracing::{info, instrument, warn};

use super::daemon::ServiceStats;

/// Signal handler for graceful shutdown and reload
pub struct SignalHandler {
    signals: Option<Signals>,
}

impl SignalHandler {
    /// Create a new signal handler
    pub const fn new() -> crate::Result<Self> {
        Ok(Self { signals: None })
    }

    /// Set up signal handlers
    #[instrument(skip_all)]
    pub async fn handle_signals(
        &mut self,
        shutdown_tx: watch::Sender<bool>,
        stats: Arc<RwLock<ServiceStats>>,
    ) -> crate::Result<()> {
        info!("Setting up signal handlers");

        // Register signal handlers
        #[cfg(unix)]
        let signals = Signals::new([SIGTERM, SIGINT, SIGQUIT, SIGHUP])
            .map_err(|e| crate::Error::Service(format!("Failed to register signals: {e}")))?;

        #[cfg(not(unix))]
        let signals = Signals::new([SIGTERM, SIGINT])
            .map_err(|e| crate::Error::Service(format!("Failed to register signals: {e}")))?;

        self.signals = Some(signals);

        // Spawn signal handling task
        tokio::spawn(async move {
            #[cfg(unix)]
            let mut signals = Signals::new([SIGTERM, SIGINT, SIGQUIT, SIGHUP])
                .map_err(|e| crate::Error::Service(format!("Failed to register signals: {e}")))
                .unwrap();

            #[cfg(not(unix))]
            let mut signals = Signals::new([SIGTERM, SIGINT])
                .map_err(|e| crate::Error::Service(format!("Failed to register signals: {e}")))
                .unwrap();

            while let Some(signal) = signals.next().await {
                match signal {
                    SIGTERM => {
                        info!("Received SIGTERM - initiating graceful shutdown");
                        let _ = shutdown_tx.send(true);
                        break;
                    }
                    SIGINT => {
                        info!("Received SIGINT - initiating graceful shutdown");
                        let _ = shutdown_tx.send(true);
                        break;
                    }
                    #[cfg(unix)]
                    SIGQUIT => {
                        info!("Received SIGQUIT - initiating immediate shutdown");
                        let _ = shutdown_tx.send(true);
                        break;
                    }
                    #[cfg(unix)]
                    SIGHUP => {
                        info!("Received SIGHUP - reloading configuration");
                        // In a real implementation, we would trigger config reload here
                        // For now, just log it
                        warn!("Configuration reload not yet implemented");
                    }
                    _ => {
                        warn!("Received unexpected signal: {}", signal);
                    }
                }

                // Update stats
                let mut stats = stats.write().await;
                stats.errors_count += 1;
            }

            info!("Signal handler task exiting");
        });

        Ok(())
    }

    /// Send a signal to a process
    pub fn send_signal(pid: u32, signal: Signal) -> crate::Result<()> {
        #[cfg(unix)]
        {
            use nix::sys::signal;
            use nix::unistd::Pid;

            let nix_signal = match signal {
                Signal::Term => signal::Signal::SIGTERM,
                Signal::Int => signal::Signal::SIGINT,
                Signal::Quit => signal::Signal::SIGQUIT,
                Signal::Hup => signal::Signal::SIGHUP,
                Signal::Usr1 => signal::Signal::SIGUSR1,
                Signal::Usr2 => signal::Signal::SIGUSR2,
            };

            signal::kill(Pid::from_raw(pid as i32), nix_signal)
                .map_err(|e| crate::Error::Service(format!("Failed to send signal: {e}")))?;
        }

        #[cfg(not(unix))]
        {
            return Err(crate::Error::Service(
                "Signal handling not supported on this platform".to_string(),
            ));
        }

        Ok(())
    }

    /// Check if a signal is pending
    pub const fn signal_pending(&self) -> bool {
        // This would check if any signals are pending
        // For now, return false
        false
    }
}

/// Signal types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Term,
    Int,
    Quit,
    Hup,
    Usr1,
    Usr2,
}

impl std::fmt::Display for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Term => write!(f, "SIGTERM"),
            Self::Int => write!(f, "SIGINT"),
            Self::Quit => write!(f, "SIGQUIT"),
            Self::Hup => write!(f, "SIGHUP"),
            Self::Usr1 => write!(f, "SIGUSR1"),
            Self::Usr2 => write!(f, "SIGUSR2"),
        }
    }
}

impl std::fmt::Debug for SignalHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignalHandler")
            .field("signals_registered", &self.signals.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_handler_creation() {
        let handler = SignalHandler::new().unwrap();
        assert!(!handler.signal_pending());
    }

    #[test]
    fn test_signal_display() {
        assert_eq!(Signal::Term.to_string(), "SIGTERM");
        assert_eq!(Signal::Int.to_string(), "SIGINT");
        assert_eq!(Signal::Hup.to_string(), "SIGHUP");
    }

    #[test]
    fn test_signal_equality() {
        assert_eq!(Signal::Term, Signal::Term);
        assert_ne!(Signal::Term, Signal::Int);
    }
}
