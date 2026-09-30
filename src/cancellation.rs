use std::{error::Error, fmt};

use anyhow::Result;
use tokio::{sync::watch, task::JoinHandle};

/// Shared stop request. Workers finish cleanup and return diagnostics after it fires.
#[derive(Clone)]
pub struct Cancellation(watch::Sender<bool>);

impl Default for Cancellation {
    fn default() -> Self {
        Self(watch::channel(false).0)
    }
}

impl Cancellation {
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }

    pub fn is_cancelled(&self) -> bool {
        *self.0.borrow()
    }

    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            return Err(Interrupted.into());
        }
        Ok(())
    }

    pub async fn cancelled(&self) {
        let mut receiver = self.0.subscribe();
        let _ = receiver.wait_for(|cancelled| *cancelled).await;
    }

    /// Install handlers before snapshotting, including phases with no child process.
    pub fn listen(&self) -> Result<SignalGuard> {
        let cancellation = self.clone();
        #[cfg(unix)]
        let task = {
            use tokio::signal::unix::{SignalKind, signal};
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
        Ok(SignalGuard(task))
    }
}

pub struct SignalGuard(JoinHandle<()>);

impl Drop for SignalGuard {
    fn drop(&mut self) {
        self.0.abort();
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
