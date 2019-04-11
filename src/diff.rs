use std::collections::hash_map::{Entry, HashMap};
use std::fmt;
use std::fs::File;
use std::io::{prelude::*, BufReader, SeekFrom};
use std::mem::replace;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use failure::{bail, Fail, ResultExt};
use fs2::{self, FileExt};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::Result;
use crate::util::{open_or_create, FileEntry};

pub struct Writer {
    file: File,
    path: PathBuf,
    orig: Option<OrigOutput>,
    curr: CurrOutput,
}

impl Writer {
    pub fn new(dir: &Path) -> Result<Self> {
        let path = dir.join("orig").with_extension("json");
        log::debug!("opening or creating data file '{}'", path.display());

        let file = open_or_create(&path)?;
        match file.as_ref().try_lock_exclusive() {
            Ok(()) => (),
            Err(ref err) if err.kind() == fs2::lock_contended_error().kind() => bail!(
                "file '{}' is being accessed by another process",
                path.display()
            ),
            Err(err) => {
                return Err(err
                    .context(format!("failed to lock file '{}'", path.display()))
                    .into());
            }
        }

        let orig = OrigOutput::new(&file, &path)?;
        Ok(Writer {
            file: file.into(),
            path,
            orig,
            curr: CurrOutput::new(),
        })
    }

    pub fn len(&self) -> Option<Duration> {
        self.orig.as_ref().map(|orig| orig.data.total)
    }

    pub fn completed(&self) -> Duration {
        self.orig
            .as_ref()
            .map(|orig| orig.elapsed)
            .unwrap_or_default()
    }

    pub fn write_line(&mut self, line: Vec<u8>) -> Result<()> {
        if let Some(ref mut orig) = self.orig {
            orig.write_line(&line);
        }

        self.curr.write_line(line);

        Ok(())
    }

    pub fn finish(&mut self, success: bool) -> Result<()> {
        if success || self.orig.is_none() {
            log::debug!("saving process output to file '{}'", self.path.display());
            log::trace!("current output: {:#?}", self.curr);
            self.file.seek(SeekFrom::Start(0))?;
            self.file.set_len(0)?;
            self.curr.finish(&self.file, &self.path)?;
        }

        Ok(())
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[derive(Debug)]
struct OrigOutput {
    data: OutputData,
    map: HashMap<Vec<u8>, u32>,
    seq: u32,
    elapsed: Duration,
}

impl OrigOutput {
    fn new(file: &FileEntry, path: &Path) -> Result<Option<Self>> {
        if let FileEntry::Existing(file) = file {
            let mut data: OutputData = json::from_reader(BufReader::new(file))
                .with_context(|_| format!("failed to read JSON file '{}'", path.display()))?;
            log::trace!("original output: {:#?}", data);
            let map = data
                .lines
                .iter_mut()
                .enumerate()
                .map(|(seq, line)| (replace(&mut line.data, Vec::new()), seq as u32))
                .collect();
            Ok(Some(OrigOutput {
                data,
                map,
                seq: 0,
                elapsed: Duration::from_secs(0),
            }))
        } else {
            Ok(None)
        }
    }

    fn write_line(&mut self, line: &[u8]) {
        if let Some(&seq) = self.map.get(line) {
            if self.seq <= seq {
                log::trace!("recognized line '{}'", String::from_utf8_lossy(line));
                self.elapsed = self.data.lines[seq as usize].dur;
                log::trace!("elapsed: {:#}", indicatif::HumanDuration(self.elapsed));
            }

            self.seq += 1;
        }
    }
}

#[derive(Debug)]
struct CurrOutput {
    data: OutputData,
    map: HashMap<Vec<u8>, LineData>,
    start: Instant,
}

#[derive(Debug)]
struct LineData {
    seq: u32,
    dup: bool,
}

impl CurrOutput {
    fn new() -> Self {
        CurrOutput {
            data: OutputData {
                lines: Vec::new(),
                total: Duration::from_secs(0),
            },
            map: HashMap::new(),
            start: Instant::now(),
        }
    }

    fn write_line(&mut self, line: Vec<u8>) {
        let dur = self.start.elapsed();
        let seq = self.data.lines.len() as u32;
        match self.map.entry(line) {
            Entry::Occupied(mut entry) => entry.get_mut().dup = true,
            Entry::Vacant(entry) => {
                self.data.lines.push(Line {
                    data: Vec::new(),
                    dur,
                });
                entry.insert(LineData { seq, dup: false });
            }
        };
    }

    fn finish(&mut self, file: &File, path: &Path) -> Result<()> {
        self.data.total = self.start.elapsed();
        for (line, data) in self.map.drain() {
            if !data.dup {
                self.data.lines[data.seq as usize].data = line;
            }
        }
        self.data.lines.retain(|line| !line.data.is_empty());

        json::to_writer(file, &self.data)
            .with_context(|_| format!("failed to write to file '{}'", path.display()))?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct OutputData {
    lines: Vec<Line>,
    total: Duration,
}

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq)]
struct Line {
    #[serde(serialize_with = "as_base64", deserialize_with = "from_base64")]
    data: Vec<u8>,
    dur: Duration,
}

impl fmt::Debug for Line {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Line")
            .field("data", &String::from_utf8_lossy(&self.data))
            .field("dur", &self.dur)
            .finish()
    }
}

fn as_base64<T, S>(key: &T, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    T: AsRef<[u8]>,
    S: Serializer,
{
    serializer.serialize_str(&base64::encode(key.as_ref()))
}

fn from_base64<'de, D>(deserializer: D) -> std::result::Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    let bytes = base64::decode(&string).map_err(de::Error::custom)?;
    Ok(bytes)
}
