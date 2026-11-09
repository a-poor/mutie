use std::{error::Error, fmt};

use anyhow::Result;
use tokio::{sync::watch, task::JoinHandle};

#[cfg(unix)]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Shared stop request. Workers finish cleanup and return diagnostics after it fires.
#[derive(Clone)]
pub struct Cancellation {
    stopped: watch::Sender<bool>,
    #[cfg(unix)]
    interrupt: Arc<AtomicBool>,
    #[cfg(unix)]
    terminate: Arc<AtomicBool>,
}

impl Default for Cancellation {
    fn default() -> Self {
        Self {
            stopped: watch::channel(false).0,
            #[cfg(unix)]
            interrupt: Arc::default(),
            #[cfg(unix)]
            terminate: Arc::default(),
        }
    }
}

impl Cancellation {
    pub fn cancel(&self) {
        self.stopped.send_replace(true);
    }

    pub fn is_cancelled(&self) -> bool {
        #[cfg(unix)]
        if self.interrupt.load(Ordering::SeqCst) || self.terminate.load(Ordering::SeqCst) {
            return true;
        }
        *self.stopped.borrow()
    }

    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            return Err(Interrupted.into());
        }
        Ok(())
    }

    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        let mut receiver = self.stopped.subscribe();
        let _ = receiver.wait_for(|cancelled| *cancelled).await;
    }

    /// A matching user signal may terminate a child before the async stop task
    /// runs. Internal cancellation and ordinary numeric exit codes are not
    /// evidence of that race and must retain their original classification.
    pub fn interrupted_exit(&self, status: std::process::ExitStatus) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            match status.signal() {
                Some(libc::SIGINT) => self.interrupt.load(Ordering::SeqCst),
                Some(libc::SIGTERM) => self.terminate.load(Ordering::SeqCst),
                _ => false,
            }
        }
        #[cfg(not(unix))]
        {
            let _ = status;
            false
        }
    }

    /// Install handlers before snapshotting, including phases with no child process.
    pub fn listen(&self) -> Result<SignalGuard> {
        let cancellation = self.clone();
        #[cfg(unix)]
        let mut registrations = SignalRegistrations::default();
        #[cfg(unix)]
        let task = {
            use tokio::signal::unix::{SignalKind, signal};
            // Only atomic stores execute inside signal handlers. Keep Tokio's
            // notification task for waking blocked work; flags make receipt
            // visible even when child exit wins before that task is scheduled.
            registrations.0.push(signal_hook::flag::register(
                libc::SIGINT,
                self.interrupt.clone(),
            )?);
            registrations.0.push(signal_hook::flag::register(
                libc::SIGTERM,
                self.terminate.clone(),
            )?);
            let mut interrupt = signal(SignalKind::interrupt())?;
            let mut terminate = signal(SignalKind::terminate())?;
            tokio::spawn(async move {
                tokio::select! {
                    _ = interrupt.recv() => {},
                    _ = terminate.recv() => {},
                }
                cancellation.cancel();
            })
        };
        #[cfg(not(unix))]
        let task = tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                cancellation.cancel();
            }
        });
        Ok(SignalGuard {
            task,
            #[cfg(unix)]
            _registrations: registrations,
        })
    }
}

pub struct SignalGuard {
    task: JoinHandle<()>,
    #[cfg(unix)]
    _registrations: SignalRegistrations,
}

impl Drop for SignalGuard {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[cfg(unix)]
#[derive(Default)]
struct SignalRegistrations(Vec<signal_hook::SigId>);

#[cfg(unix)]
impl Drop for SignalRegistrations {
    fn drop(&mut self) {
        for id in self.0.drain(..) {
            signal_hook::low_level::unregister(id);
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    #[test]
    fn only_matching_received_signals_identify_interrupted_exits() {
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert!(!cancellation.interrupted_exit(std::process::ExitStatus::from_raw(libc::SIGINT)));
        cancellation.interrupt.store(true, Ordering::SeqCst);
        assert!(cancellation.interrupted_exit(std::process::ExitStatus::from_raw(libc::SIGINT)));
        assert!(!cancellation.interrupted_exit(std::process::ExitStatus::from_raw(libc::SIGTERM)));
        cancellation.terminate.store(true, Ordering::SeqCst);
        assert!(cancellation.interrupted_exit(std::process::ExitStatus::from_raw(libc::SIGTERM)));
        for raw in [
            0,
            1 << 8,
            2 << 8,
            129 << 8,
            130 << 8,
            143 << 8,
            libc::SIGKILL,
            libc::SIGABRT,
        ] {
            assert!(
                !cancellation.interrupted_exit(std::process::ExitStatus::from_raw(raw)),
                "{raw}"
            );
        }
        let signal_only = Cancellation::default();
        signal_only.interrupt.store(true, Ordering::SeqCst);
        assert!(signal_only.is_cancelled());
    }
}

#[derive(Debug)]
pub struct Interrupted;

impl fmt::Display for Interrupted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("interrupted")
    }
}

impl Error for Interrupted {}
