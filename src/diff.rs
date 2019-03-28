use std::collections::hash_map::{Entry, HashMap};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use failure::{bail, Fail, ResultExt};
use fs2::{self, FileExt};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::Result;

pub struct Writer {
    start: Instant,
    last: Instant,
    orig: Orig,
    curr: HashMap<Line, LineDataInit>,
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

        let orig = Orig::new(&dir.join("orig").with_extension("json"))?;
        let now = Instant::now();
        Ok(Writer {
            start: now,
            last: now,
            orig,
            curr: HashMap::new(),
        })
    }

    pub fn write_line(&mut self, data: Vec<u8>) -> Result<()> {
        let dur = self.tick();
        let line = Line { data };

        self.orig.write_line(&line);

        let seq = self.curr.len() as u32;
        match self.curr.entry(line) {
            Entry::Occupied(mut entry) => entry.get_mut().dup = true,
            Entry::Vacant(entry) => {
                entry.insert(LineDataInit {
                    data: LineData { seq,
                    dur },
                    dup: false,
                });
            }
        };

        Ok(())
    }

    pub fn finish(&self) -> Result<()> {
        unimplemented!()
    }

    fn tick(&mut self) -> Duration {
        let now = Instant::now();
        let dur = now - self.last;
        self.last = now;
        dur
    }
}

struct Orig {
    existing: Option<(Output, u32)>,
    file: File,
}

impl Orig {
    fn new(path: &Path) -> Result<Self> {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
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

        let meta = file
            .metadata()
            .with_context(|_| format!("failed to get metadata for file '{}'", path.display()))?;
        let existing = if meta.len() == 0 {
            None
        } else {
            let output = json::from_reader(BufReader::new(&file))
                .with_context(|_| format!("failed to read JSON file '{}'", path.display()))?;
            Some((output, 0))
        };

        Ok(Orig { existing, file })
    }

    fn write_line(&mut self, line: &Line) {
        if let Some((ref output, ref mut seq)) = self.existing {
            if let Some(entry) = output.lines.get(&line) {
                if entry.seq == *seq {
                    // TODO
                } else if *seq <= entry.seq {
                    *seq = entry.seq;
                }

                *seq += 1;
            }
        }
    }

    fn update(&mut self, new: HashMap<Line, LineDataInit>) -> Result<()> {
        unimplemented!()
    }
}

impl Drop for Orig {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Output {
    lines: HashMap<Line, LineData>,
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
}

#[derive(Debug, Serialize, Deserialize)]
struct LineData {
    seq: u32,
    dur: Duration,
}

struct LineDataInit {
    data: LineData,
    dup: bool,
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
