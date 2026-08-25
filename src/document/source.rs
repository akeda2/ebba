use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone)]
pub struct FileSourceRange {
    pub path: PathBuf,
    pub file_offset: u64,
    pub len: usize,
    pub line_feeds: Option<usize>,
}

impl FileSourceRange {
    pub fn new(
        path: impl Into<PathBuf>,
        file_offset: u64,
        len: usize,
        line_feeds: Option<usize>,
    ) -> Self {
        Self {
            path: path.into(),
            file_offset,
            len,
            line_feeds,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Source {
    InMemory {
        bytes: Arc<[u8]>,
        line_feeds: usize,
    },
    FileRange {
        path: PathBuf,
        file_offset: u64,
        len: usize,
        line_feeds: Option<usize>,
        bytes_read: Arc<AtomicUsize>,
    },
}

impl Source {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let line_feeds = count_line_feeds(&bytes);
        Self::InMemory {
            bytes: Arc::<[u8]>::from(bytes),
            line_feeds,
        }
    }

    pub fn from_file_range(range: FileSourceRange) -> Self {
        Self::FileRange {
            path: range.path,
            file_offset: range.file_offset,
            len: range.len,
            line_feeds: range.line_feeds,
            bytes_read: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::InMemory { bytes, .. } => bytes.len(),
            Self::FileRange { len, .. } => *len,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn line_feed_count(&self) -> Option<usize> {
        match self {
            Self::InMemory { line_feeds, .. } => Some(*line_feeds),
            Self::FileRange { line_feeds, .. } => *line_feeds,
        }
    }

    pub fn bytes_read(&self) -> Option<usize> {
        match self {
            Self::InMemory { .. } => None,
            Self::FileRange { bytes_read, .. } => Some(bytes_read.load(Ordering::Relaxed)),
        }
    }

    pub fn read_range(&self, start: usize, len: usize) -> std::io::Result<Vec<u8>> {
        if start > self.len() || len > self.len().saturating_sub(start) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "source range is out of bounds",
            ));
        }

        match self {
            Self::InMemory { bytes, .. } => Ok(bytes[start..start + len].to_vec()),
            Self::FileRange {
                path,
                file_offset,
                bytes_read,
                ..
            } => {
                let mut file = File::open(path)?;
                file.seek(SeekFrom::Start(*file_offset + start as u64))?;
                let mut out = vec![0_u8; len];
                file.read_exact(&mut out)?;
                bytes_read.fetch_add(len, Ordering::Relaxed);
                Ok(out)
            }
        }
    }

    pub fn in_memory_slice_line_feeds(&self, start: usize, len: usize) -> Option<usize> {
        match self {
            Self::InMemory { bytes, .. } => Some(count_line_feeds(&bytes[start..start + len])),
            Self::FileRange { .. } => None,
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::InMemory { .. } => None,
            Self::FileRange { path, .. } => Some(path.as_path()),
        }
    }
}

pub fn count_line_feeds(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte == b'\n').count()
}
