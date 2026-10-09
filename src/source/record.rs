//! Recording and replaying snapshots as JSON lines, for reproducible demos and
//! bug reports, and for realistic test fixtures.

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::{ProcInfo, ProcessSource, Snapshot};

/// One snapshot as stored on disk (the map becomes a list so JSON is clean).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedSnapshot {
    pub load: f32,
    pub procs: Vec<ProcInfo>,
}

impl RecordedSnapshot {
    pub fn from_snapshot(snapshot: &Snapshot) -> Self {
        Self {
            load: snapshot.load,
            procs: snapshot.procs.values().cloned().collect(),
        }
    }

    pub fn into_snapshot(self) -> Snapshot {
        Snapshot::new(self.procs.into_iter().map(|p| (p.pid, p)).collect()).with_load(self.load)
    }
}

/// Writes each snapshot as a line of JSON.
pub struct Recorder {
    file: BufWriter<File>,
}

impl Recorder {
    pub fn create(path: &Path) -> Result<Self> {
        let file =
            File::create(path).with_context(|| format!("creating recording {}", path.display()))?;
        Ok(Self {
            file: BufWriter::new(file),
        })
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
pub struct ReplaySource {
    lines: std::vec::IntoIter<String>,
    last: Snapshot,
}

impl ReplaySource {
    pub fn open(path: &Path) -> Result<Self> {
        let file =
            File::open(path).with_context(|| format!("opening recording {}", path.display()))?;
        let lines: Vec<String> = BufReader::new(file)
            .lines()
            .collect::<std::io::Result<_>>()?;
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

    #[test]
    fn round_trips_a_snapshot() {
        let snap = snapshot(vec![
            proc(1, "a").with_cpu(3.0),
            proc(2, "b").with_memory(1024),
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
            "{}\n{}\n",
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
}
