use std::borrow::Cow;
use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use LavaTorrentError;

pub(crate) fn u64_to_usize(src: u64) -> Result<usize, LavaTorrentError> {
    usize::try_from(src).map_err(|_| {
        LavaTorrentError::FailedNumericConv(Cow::Owned(format!(
            "[{}] does not fit into usize.",
            src
        )))
    })
}

pub(crate) fn usize_to_u64(src: usize) -> Result<u64, LavaTorrentError> {
    u64::try_from(src).map_err(|_| {
        LavaTorrentError::FailedNumericConv(Cow::Owned(format!("[{}] does not fit into u64.", src)))
    })
}

pub(crate) fn i64_to_usize(src: i64) -> Result<usize, LavaTorrentError> {
    usize::try_from(src).map_err(|_| {
        LavaTorrentError::FailedNumericConv(Cow::Owned(format!(
            "[{}] does not fit into usize.",
            src
        )))
    })
}

pub(crate) fn i64_to_u64(src: i64) -> Result<u64, LavaTorrentError> {
    u64::try_from(src).map_err(|_| {
        LavaTorrentError::FailedNumericConv(Cow::Owned(format!("[{}] does not fit into u64.", src)))
    })
}

pub(crate) fn u64_to_i64(src: u64) -> Result<i64, LavaTorrentError> {
    i64::try_from(src).map_err(|_| {
        LavaTorrentError::FailedNumericConv(Cow::Owned(format!("[{}] does not fit into i64.", src)))
    })
}

// this method is recursive, i.e. entries in subdirectories
// are also returned
//
// *nix hidden files/dirs are ignored
//
// returned vec is sorted by path
pub(crate) fn list_dir<P>(path: P) -> Result<Vec<(PathBuf, u64)>, LavaTorrentError>
where
    P: AsRef<Path>,
{
    let mut entries = Vec::new();

    for entry in path.as_ref().read_dir()? {
        let entry = entry?;
        let path = entry.path();
        let metadata = path.metadata()?;

        if last_component(&path)?.starts_with('.') {
            continue;
        } // hidden files/dirs are ignored

        if metadata.is_dir() {
            entries.extend(list_dir(path)?);
        } else {
            entries.push((path, metadata.len()));
        }
    }

    entries.sort_by(|(p1, _), (p2, _)| p1.cmp(p2));
    Ok(entries)
}

pub(crate) fn last_component<P>(path: P) -> Result<String, LavaTorrentError>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    match path.file_name() {
        Some(s) => Ok(s.to_string_lossy().into_owned()),
        None => Err(LavaTorrentError::InvalidArgument(Cow::Owned(format!(
            r#"[{}] ends in ".."."#,
            path.display()
        )))),
    }
}

pub(crate) struct ByteBuffer<'a> {
    bytes: &'a [u8],
    position: usize, // current cursor position
    length: usize,   // total buffer length
}

impl<'a> ByteBuffer<'a> {
    pub(crate) fn new(bytes: &[u8]) -> ByteBuffer {
        ByteBuffer {
            bytes,
            position: 0,
            length: bytes.len(),
        }
    }

    pub(crate) fn peek(&self) -> Option<&'a u8> {
        if self.is_empty() {
            None
        } else {
            Some(&self.bytes[self.position])
        }
    }

    pub(crate) fn advance(&mut self, step: usize) {
        self.position += step;
        if self.position > self.length {
            self.position = self.length;
        }
    }

    pub(crate) fn pos(&self) -> usize {
        self.position
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.position >= self.length
    }
}

impl<'a> Iterator for ByteBuffer<'a> {
    type Item = &'a u8;

    fn next(&mut self) -> Option<&'a u8> {
        if self.is_empty() {
            None
        } else {
            self.position += 1;
            Some(&self.bytes[self.position - 1])
        }
    }
}

#[cfg(test)]
mod util_tests {
    use super::*;

    #[test]
    fn list_dir_ok() {
        assert_eq!(
            list_dir("tests/files").unwrap(),
            vec![
                PathBuf::from("tests/files/byte_sequence"),
                PathBuf::from("tests/files/symlink"),
                PathBuf::from("tests/files/tails-amd64-3.6.1.torrent"),
                PathBuf::from("tests/files/ubuntu-16.04.4-desktop-amd64.iso.torrent"),
                // no [.hidden]
            ]
            .iter()
            .map(PathBuf::from)
            .map(|p| (p.clone(), p.metadata().unwrap().len()))
            .collect::<Vec<(PathBuf, u64)>>()
        );
    }

    #[test]
    fn list_dir_with_subdir() {
        assert_eq!(
            list_dir("src/torrent").unwrap(),
            vec![
                PathBuf::from("src/torrent/mod.rs"),
                PathBuf::from("src/torrent/v1/build.rs"),
                PathBuf::from("src/torrent/v1/mod.rs"),
                PathBuf::from("src/torrent/v1/read.rs"),
                PathBuf::from("src/torrent/v1/write.rs"),
            ]
            .iter()
            .map(PathBuf::from)
            .map(|p| (p.clone(), p.metadata().unwrap().len()))
            .collect::<Vec<(PathBuf, u64)>>()
        );
    }

    #[test]
    fn last_component_ok() {
        assert_eq!(
            last_component("/root/dir/file.ext").unwrap(),
            "file.ext".to_owned()
        );
    }

    #[test]
    fn last_component_ok_2() {
        assert_eq!(last_component("/root/dir/dir2").unwrap(), "dir2".to_owned());
    }

    #[test]
    fn last_component_err() {
        match last_component("/root/dir/..") {
            Err(LavaTorrentError::InvalidArgument(m)) => {
                assert_eq!(m, r#"[/root/dir/..] ends in ".."."#,);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn u64_to_usize_ok() {
        // @todo: add test for err
        assert_eq!(u64_to_usize(42).unwrap(), 42);
    }

    #[test]
    fn usize_to_u64_ok() {
        // @todo: add test for err
        assert_eq!(usize_to_u64(42).unwrap(), 42);
    }

    #[test]
    fn i64_to_usize_ok() {
        assert_eq!(i64_to_usize(42).unwrap(), 42);
    }

    #[test]
    fn i64_to_usize_err() {
        match i64_to_usize(-1) {
            Err(LavaTorrentError::FailedNumericConv(m)) => {
                assert_eq!(m, "[-1] does not fit into usize.");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn i64_to_u64_ok() {
        assert_eq!(i64_to_u64(42).unwrap(), 42);
    }

    #[test]
    fn i64_to_u64_err() {
        match i64_to_u64(-1) {
            Err(LavaTorrentError::FailedNumericConv(m)) => {
                assert_eq!(m, "[-1] does not fit into u64.");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn u64_to_i64_ok() {
        assert_eq!(u64_to_i64(42).unwrap(), 42);
    }

    #[test]
    fn u64_to_i64_err() {
        match u64_to_i64(u64::max_value()) {
            Err(LavaTorrentError::FailedNumericConv(m)) => {
                assert_eq!(m, format!("[{}] does not fit into i64.", u64::max_value()))
            }
            _ => panic!(),
        }
    }
}

#[cfg(test)]
mod byte_buffer_tests {
    use super::*;

    #[test]
    fn byte_buffer_sanity_test() {
        let bytes = vec![1, 2, 3];
        let mut buffer = ByteBuffer::new(&bytes);

        assert!(!buffer.is_empty());
        assert_eq!(buffer.peek(), Some(&1));
        assert_eq!(buffer.pos(), 0);
        buffer.advance(1);

        assert!(!buffer.is_empty());
        assert_eq!(buffer.peek(), Some(&2));
        assert_eq!(buffer.pos(), 1);
        buffer.advance(2);

        assert!(buffer.is_empty());
        assert_eq!(buffer.peek(), None);
        assert_eq!(buffer.pos(), 3);
        buffer.advance(1);

        assert!(buffer.is_empty());
        assert_eq!(buffer.peek(), None);
        assert_eq!(buffer.pos(), 3);
    }

    #[test]
    fn byte_buffer_iterator_test() {
        let bytes = vec![1, 2, 3];
        let mut buffer = ByteBuffer::new(&bytes);
        let mut output = Vec::new();

        for byte in &mut buffer {
            output.push(*byte);
        }

        assert!(buffer.is_empty());
        assert_eq!(buffer.peek(), None);
        assert_eq!(buffer.next(), None);
        assert_eq!(buffer.pos(), 3);
        assert_eq!(bytes, output);
    }
}
