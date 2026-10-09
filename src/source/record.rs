//! Recording and replaying snapshots as JSON lines, for reproducible demos and
//! bug reports, and for realistic test fixtures.
//!
//! A recording is a versioned format, separate from the in-memory [`ProcInfo`]:
//! the first line is `{"procquarium_recording": 1}` and each later line is one
//! snapshot. Keeping a dedicated record type means internal changes never break
//! recordings written by an earlier 1.x, which is part of the 1.0 promise.

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::{ProcInfo, ProcStatus, ProcessSource, Snapshot};

/// The version of the on-disk recording format this build writes and reads.
pub const RECORDING_VERSION: u32 = 1;

/// The header that opens every recording.
#[derive(Debug, Serialize, Deserialize)]
struct Header {
    procquarium_recording: u32,
}

/// A process as stored on disk: a deliberate, stable schema, kept separate from
/// [`ProcInfo`] so that renaming or adding internal fields never breaks an old
/// recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordedProc {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
    pub status: RecordedStatus,
    pub user: Option<String>,
    pub start_time: u64,
    pub kernel: bool,
    pub container: bool,
    pub io: u64,
    pub run_time: u64,
}

/// Coarse process state in a recording. Serialised as `snake_case` strings, so
/// the wire format does not depend on the internal enum's representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordedStatus {
    Running,
    Sleeping,
    Zombie,
    Other,
}

impl From<&ProcInfo> for RecordedProc {
    fn from(p: &ProcInfo) -> Self {
        Self {
            pid: p.pid,
            parent: p.parent,
            name: p.name.clone(),
            cpu: p.cpu,
            memory: p.memory,
            status: p.status.into(),
            user: p.user.clone(),
            start_time: p.start_time,
            kernel: p.kernel,
            container: p.container,
            io: p.io,
            run_time: p.run_time,
        }
    }
}

impl From<RecordedProc> for ProcInfo {
    fn from(p: RecordedProc) -> Self {
        Self {
            pid: p.pid,
            parent: p.parent,
            name: p.name,
            cpu: p.cpu,
            memory: p.memory,
            status: p.status.into(),
            user: p.user,
            start_time: p.start_time,
            kernel: p.kernel,
            container: p.container,
            io: p.io,
            run_time: p.run_time,
        }
    }
}

impl From<ProcStatus> for RecordedStatus {
    fn from(status: ProcStatus) -> Self {
        match status {
            ProcStatus::Running => Self::Running,
            ProcStatus::Sleeping => Self::Sleeping,
            ProcStatus::Zombie => Self::Zombie,
            ProcStatus::Other => Self::Other,
        }
    }
}

impl From<RecordedStatus> for ProcStatus {
    fn from(status: RecordedStatus) -> Self {
        match status {
            RecordedStatus::Running => Self::Running,
            RecordedStatus::Sleeping => Self::Sleeping,
            RecordedStatus::Zombie => Self::Zombie,
            RecordedStatus::Other => Self::Other,
        }
    }
}

/// One snapshot as stored on disk (the map becomes a list so JSON is clean).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordedSnapshot {
    pub load: f32,
    pub procs: Vec<RecordedProc>,
}

impl RecordedSnapshot {
    pub fn from_snapshot(snapshot: &Snapshot) -> Self {
        // Sort by pid so a recording is byte-for-byte reproducible even though
        // the live snapshot is a `HashMap`.
        let mut procs: Vec<RecordedProc> =
            snapshot.procs.values().map(RecordedProc::from).collect();
        procs.sort_by_key(|p| p.pid);
        Self {
            load: snapshot.load,
            procs,
        }
    }

    pub fn into_snapshot(self) -> Snapshot {
        Snapshot::new(
            self.procs
                .into_iter()
                .map(|p| {
                    let info = ProcInfo::from(p);
                    (info.pid, info)
                })
                .collect(),
        )
        .with_load(self.load)
    }
}

/// Writes the header, then each snapshot as a line of JSON.
pub struct Recorder {
    file: BufWriter<File>,
}

impl Recorder {
    pub fn create(path: &Path) -> Result<Self> {
        let file =
            File::create(path).with_context(|| format!("creating recording {}", path.display()))?;
        let mut file = BufWriter::new(file);
        writeln!(file, r#"{{"procquarium_recording": {RECORDING_VERSION}}}"#)
            .with_context(|| format!("writing the header of recording {}", path.display()))?;
        Ok(Self { file })
    }

    pub fn write(&mut self, snapshot: &Snapshot) -> Result<()> {
        let line = serde_json::to_string(&RecordedSnapshot::from_snapshot(snapshot))?;
        writeln!(self.file, "{line}")?;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        self.file.flush()?;
        Ok(())
    }
}

/// Replays snapshots from a recording, one per sample. When the recording runs
/// out it keeps returning the last snapshot.
#[derive(Debug)]
pub struct ReplaySource {
    lines: std::vec::IntoIter<String>,
    last: Snapshot,
}

impl ReplaySource {
    pub fn open(path: &Path) -> Result<Self> {
        let file =
            File::open(path).with_context(|| format!("opening recording {}", path.display()))?;
        let mut lines = BufReader::new(file).lines();

        let header = lines
            .next()
            .transpose()?
            .ok_or_else(|| anyhow::anyhow!("recording {} is empty", path.display()))?;
        let header: Header = serde_json::from_str(&header)
            .with_context(|| format!("reading the header of recording {}", path.display()))?;
        if header.procquarium_recording != RECORDING_VERSION {
            bail!(
                "recording {} was written in format version {}, but this build replays version {}",
                path.display(),
                header.procquarium_recording,
                RECORDING_VERSION
            );
        }

        let lines: Vec<String> = lines.collect::<std::io::Result<_>>()?;
        Ok(Self {
            lines: lines.into_iter(),
            last: Snapshot::default(),
        })
    }
}

impl ProcessSource for ReplaySource {
    fn snapshot(&mut self) -> Result<Snapshot> {
        if let Some(line) = self.lines.next()
            && !line.trim().is_empty()
        {
            let recorded: RecordedSnapshot = serde_json::from_str(&line)?;
            self.last = recorded.into_snapshot();
        }
        Ok(self.last.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::fake::{proc, snapshot};

    /// The committed fixture, recorded by this code and replayed forever after.
    fn fixture_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/small.jsonl")
    }

    /// Write the fixture with the real recorder. Ignored by default; run it with
    /// `cargo test -- --ignored write_fixture` after a format change to refresh
    /// the committed file.
    #[test]
    #[ignore]
    fn write_fixture() {
        let a = snapshot(vec![
            proc(100, "firefox")
                .with_cpu(3.5)
                .with_memory(1024 * 1024)
                .with_status(ProcStatus::Running)
                .with_user("alice")
                .with_start_time(1000)
                .with_run_time(120),
            proc(200, "bash")
                .with_memory(4096)
                .with_status(ProcStatus::Sleeping),
        ])
        .with_load(0.4);
        let b = snapshot(vec![
            proc(100, "firefox")
                .with_cpu(1.0)
                .with_memory(2 * 1024 * 1024)
                .with_status(ProcStatus::Running)
                .with_user("alice")
                .with_start_time(1000)
                .with_run_time(121),
            proc(300, "rustc")
                .with_cpu(88.0)
                .with_memory(256 * 1024)
                .with_parent(200)
                .with_start_time(1001)
                .with_io(8192),
        ])
        .with_load(0.8);

        let path = fixture_path();
        std::fs::create_dir_all(path.parent().expect("fixtures dir")).expect("create dir");
        let mut recorder = Recorder::create(&path).expect("create recording");
        recorder.write(&a).expect("write a");
        recorder.write(&b).expect("write b");
        recorder.flush().expect("flush");
    }

    #[test]
    fn round_trips_a_snapshot() {
        let snap = snapshot(vec![
            proc(1, "a").with_cpu(3.0),
            proc(2, "b")
                .with_memory(1024)
                .with_status(ProcStatus::Zombie),
        ])
        .with_load(0.4);
        let line =
            serde_json::to_string(&RecordedSnapshot::from_snapshot(&snap)).expect("serialize");
        let back: RecordedSnapshot = serde_json::from_str(&line).expect("deserialize");
        assert_eq!(back.into_snapshot(), snap);
    }

    #[test]
    fn replays_lines_in_order_and_keeps_the_last() {
        let a = snapshot(vec![proc(1, "a")]);
        let b = snapshot(vec![proc(2, "b")]);
        let text = format!(
            "{{\"procquarium_recording\": {RECORDING_VERSION}}}\n{}\n{}\n",
            serde_json::to_string(&RecordedSnapshot::from_snapshot(&a)).expect("serialize"),
            serde_json::to_string(&RecordedSnapshot::from_snapshot(&b)).expect("serialize"),
        );
        let path =
            std::env::temp_dir().join(format!("procquarium-test-{}.jsonl", std::process::id()));
        std::fs::write(&path, text).expect("write recording");

        let mut source = ReplaySource::open(&path).expect("open recording");
        assert_eq!(source.snapshot().expect("sample"), a);
        assert_eq!(source.snapshot().expect("sample"), b);
        assert_eq!(
            source.snapshot().expect("sample"),
            b,
            "keeps the last when exhausted"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_unknown_versions() {
        let path = std::env::temp_dir().join(format!(
            "procquarium-version-test-{}.jsonl",
            std::process::id()
        ));
        std::fs::write(&path, "{\"procquarium_recording\": 99}\n").expect("write recording");

        let error = ReplaySource::open(&path).expect_err("unknown version is rejected");
        let message = format!("{error:#}");
        assert!(
            message.contains("format version 99") && message.contains("replays version 1"),
            "clear version message, got: {message}"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn replays_the_committed_fixture() {
        let mut source = ReplaySource::open(&fixture_path()).expect("open fixture");

        let first = source.snapshot().expect("first snapshot");
        assert_eq!(first.load, 0.4);
        assert_eq!(first.procs.len(), 2);
        let firefox = first.procs.get(&100).expect("firefox");
        assert_eq!(firefox.name, "firefox");
        assert_eq!(firefox.user.as_deref(), Some("alice"));
        assert_eq!(firefox.status, ProcStatus::Running);

        let second = source.snapshot().expect("second snapshot");
        assert_eq!(second.load, 0.8);
        assert_eq!(second.procs.len(), 2);
        assert_eq!(second.procs.get(&300).expect("rustc").parent, Some(200));
        assert!(!second.procs.contains_key(&200), "bash has exited");
    }
}
