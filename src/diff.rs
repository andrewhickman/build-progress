use std::collections::hash_map::{Entry, HashMap};
use std::fs::{self, File};
use std::io::{BufReader};
use std::mem::replace;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use failure::{bail, Fail, ResultExt};
use fs2::{self, FileExt};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::Result;

pub struct Writer {
    file: File,
    path: PathBuf,
    orig: Option<OrigOutput>,
    curr: CurrOutput,
}

impl Writer {
    pub fn new(hash: String) -> Result<Self> {
        let dir = if let Some(dir) = dirs::data_dir() {
            dir.join("build-progress").join(hash)
        } else {
            bail!("failed to get user's data directory");
        };

        fs::create_dir_all(&dir)
            .with_context(|_| format!("failed to create directory '{}'", dir.display()))?;

        let path = dir.join("orig").with_extension("json");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .with_context(|_| format!("failed to open or create file '{}'", path.display()))?;

        match file.try_lock_exclusive() {
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
            file,
            path,
            orig,
            curr: CurrOutput::new(),
        })
    }

    pub fn write_line(&mut self, line: Vec<u8>) -> Result<()> {
        if let Some(ref mut orig) = self.orig {
            orig.write_line(&line);
        }

        self.curr.write_line(line);

        Ok(())
    }

    pub fn finish(&mut self) -> Result<()> {
        self.curr.finish(&self.file, &self.path)
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

struct OrigOutput {
    data: OutputData,
    map: HashMap<Vec<u8>, u32>,
    seq: u32,
    elapsed: Duration,
}

impl OrigOutput {
    fn new(file: &File, path: &Path) -> Result<Option<Self>> {
        let meta = file
            .metadata()
            .with_context(|_| format!("failed to get metadata for file '{}'", path.display()))?;
        if meta.len() == 0 {
            Ok(None)
        } else {
            let mut data: OutputData = json::from_reader(BufReader::new(file))
                .with_context(|_| format!("failed to read JSON file '{}'", path.display()))?;
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
        }
    }

    fn write_line(&mut self, line: &[u8]) {
        if let Some(&seq) = self.map.get(line) {
            if seq == self.seq {
                self.elapsed += self.data.lines[seq as usize].dur;
            } else if self.seq <= seq {
                for idx in self.seq..=seq {
                    self.elapsed += self.data.lines[idx as usize].dur;
                }
                self.seq = seq;
            }

            self.seq += 1;
        }
    }
}

struct CurrOutput {
    data: OutputData,
    map: HashMap<Vec<u8>, LineData>,
    last: Instant,
}

struct LineData {
    seq: u32,
    dur: Duration,
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
            last: Instant::now(),
        }
    }

    fn write_line(&mut self, line: Vec<u8>) {
        let dur = self.tick();
        let seq = self.data.lines.len() as u32;
        match self.map.entry(line) {
            Entry::Occupied(mut entry) => entry.get_mut().dup = true,
            Entry::Vacant(entry) => {
                self.data.lines.push(Line {
                    data: Vec::new(),
                    dur,
                });
                entry.insert(LineData {
                    seq,
                    dur,
                    dup: false,
                });
            }
        };
    }

    fn finish(&mut self, file: &File, path: &Path) -> Result<()> {
        for (line, data) in self.map.drain() {
            if !data.dup {
                self.data.lines[data.seq as usize].data = line;
            }
        }

        json::to_writer(file, &self.data)
            .with_context(|_| format!("failed to write to file '{}'", path.display()))?;
        Ok(())
    }

    fn tick(&mut self) -> Duration {
        let now = Instant::now();
        let dur = now - self.last;
        self.last = now;
        dur
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct OutputData {
    lines: Vec<Line>,
    total: Duration,
}

#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
struct Line {
    #[serde(
        flatten,
        serialize_with = "as_base64",
        deserialize_with = "from_base64"
    )]
    data: Vec<u8>,
    dur: Duration,
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
