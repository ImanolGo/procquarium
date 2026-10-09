//! Sampling on a background thread.
//!
//! The worker owns the [`ProcessSource`], samples on its own timer, and sends
//! snapshots back over a channel. Priority nudges are sent to the same worker,
//! so the render loop never blocks on either sampling or `setpriority`.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::source::record::Recorder;
use crate::source::sysinfo_source::SysinfoSource;
use crate::source::{PriorityBoost, ProcessSource, Snapshot};

/// What the worker sends back to the render loop.
#[derive(Debug)]
pub enum Event {
    Snapshot(Snapshot),
    /// The outcome of a priority nudge.
    Boost(PriorityBoost),
    /// A sampling error, to be shown in the status line.
    Error(String),
}

enum Command {
    Boost((u32, u64)),
    Restore((u32, u64)),
    Shutdown,
}

/// Handle to the sampling thread. Dropping it stops the thread and restores any
/// priorities it changed.
pub struct Sampler {
    commands: Sender<Command>,
    events: Receiver<Event>,
    handle: Option<JoinHandle<()>>,
}

impl Sampler {
    pub fn spawn(interval: Duration) -> std::io::Result<Self> {
        Self::spawn_with(interval, SysinfoSource::new())
    }

    /// Spawn with a specific source (used by tests and replay).
    pub fn spawn_with<S>(interval: Duration, source: S) -> std::io::Result<Self>
    where
        S: ProcessSource + Send + 'static,
    {
        Self::spawn_with_recorder(interval, source, None)
    }

    /// Spawn with a source and, optionally, a recorder that logs every snapshot.
    pub fn spawn_with_recorder<S>(
        interval: Duration,
        source: S,
        recorder: Option<Recorder>,
    ) -> std::io::Result<Self>
    where
        S: ProcessSource + Send + 'static,
    {
        let (commands, command_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let handle = thread::Builder::new()
            .name("procquarium-sampler".to_string())
            .spawn(move || worker(interval, source, recorder, command_rx, event_tx))?;
        Ok(Self {
            commands,
            events,
            handle: Some(handle),
        })
    }

    /// Ask the worker to nudge a process's priority.
    pub fn boost(&self, id: (u32, u64)) {
        let _ = self.commands.send(Command::Boost(id));
    }

    /// Ask the worker to undo a nudge.
    pub fn restore(&self, id: (u32, u64)) {
        let _ = self.commands.send(Command::Restore(id));
    }

    /// Non-blocking poll for the next event.
    pub fn try_recv(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    /// Block for up to `timeout` for the next event.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Event, RecvTimeoutError> {
        self.events.recv_timeout(timeout)
    }

    /// Stop the worker (restoring priorities) and wait for it to finish.
    pub fn shutdown(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn worker<S>(
    interval: Duration,
    mut source: S,
    mut recorder: Option<Recorder>,
    commands: Receiver<Command>,
    events: Sender<Event>,
) where
    S: ProcessSource,
{
    loop {
        // Wait out the interval, handling priority commands meanwhile.
        let deadline = Instant::now() + interval;
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            match commands.recv_timeout(deadline - now) {
                Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                    source.restore_all_priorities();
                    return;
                }
                Ok(Command::Boost(id)) => {
                    let outcome = source.boost_priority(id);
                    if events.send(Event::Boost(outcome)).is_err() {
                        source.restore_all_priorities();
                        return;
                    }
                }
                Ok(Command::Restore(id)) => source.restore_priority(id),
                Err(RecvTimeoutError::Timeout) => break,
            }
        }

        match source.snapshot() {
            Ok(snapshot) => {
                if let Some(log) = &mut recorder
                    && let Err(error) = log.write(&snapshot)
                {
                    // Stop recording but keep the aquarium running.
                    let _ = events.send(Event::Error(format!("recording failed: {error}")));
                    recorder = None;
                }
                if events.send(Event::Snapshot(snapshot)).is_err() {
                    source.restore_all_priorities();
                    return;
                }
            }
            Err(error) => {
                if events.send(Event::Error(error.to_string())).is_err() {
                    source.restore_all_priorities();
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::fake::{FakeSource, proc};

    #[test]
    fn delivers_snapshots_and_stops_promptly() {
        let source = FakeSource::constant(vec![proc(1, "shell")]);
        let mut sampler =
            Sampler::spawn_with(Duration::from_millis(20), source).expect("spawn sampler");

        let event = sampler
            .recv_timeout(Duration::from_secs(5))
            .expect("a snapshot arrives");
        assert!(matches!(event, Event::Snapshot(_)));

        let start = Instant::now();
        sampler.shutdown();
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "shutdown is quick"
        );
    }
}
