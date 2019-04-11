use std::fs::{File, Metadata, OpenOptions};
use std::path::Path;

use failure::ResultExt;

use crate::Result;

pub enum FileEntry {
    Existing(File),
    New(File),
}

impl AsRef<File> for FileEntry {
    fn as_ref(&self) -> &File {
        match self {
            FileEntry::Existing(ref file) => file,
            FileEntry::New(ref file) => file,
        }
    }
}

impl Into<File> for FileEntry {
    fn into(self) -> File {
        match self {
            FileEntry::Existing(file) => file,
            FileEntry::New(file) => file,
        }
    }
}

pub fn open_or_create<P>(path: P) -> Result<(FileEntry, Metadata)>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
        .with_context(|_| format!("failed to open or create file '{}'", path.display()))?;
    let meta = file
        .metadata()
        .with_context(|_| format!("failed to get metadata for file '{}'", path.display()))?;
    if meta.len() == 0 {
        Ok((FileEntry::New(file), meta))
    } else {
        Ok((FileEntry::Existing(file), meta))
    }
}
