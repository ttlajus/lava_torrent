use std::path::{Path, PathBuf};
use std::borrow::Cow;
use conv::ValueFrom;
use {Error, ErrorKind, Result};

pub(crate) fn u64_to_usize(src: u64) -> Result<usize> {
    // @todo: switch to `usize::try_from()` when it's stable
    match usize::value_from(src) {
        Ok(n) => Ok(n),
        Err(_) => Err(Error::new(
            ErrorKind::IOError,
            Cow::Owned(format!("[{}] does not fit into usize.", src)),
        )),
    }
}

pub(crate) fn usize_to_u64(src: usize) -> Result<u64> {
    // @todo: switch to `u64::try_from()` when it's stable
    match u64::value_from(src) {
        Ok(n) => Ok(n),
        Err(_) => Err(Error::new(
            ErrorKind::IOError,
            Cow::Owned(format!("[{}] does not fit into u64.", src)),
        )),
    }
}

pub(crate) fn i64_to_usize(src: i64) -> Result<usize> {
    // @todo: switch to `usize::try_from()` when it's stable
    match usize::value_from(src) {
        Ok(n) => Ok(n),
        Err(_) => Err(Error::new(
            ErrorKind::IOError,
            Cow::Owned(format!("[{}] does not fit into usize.", src)),
        )),
    }
}

pub(crate) fn usize_to_i64(src: usize) -> Result<i64> {
    // @todo: switch to `i64::try_from()` when it's stable
    match i64::value_from(src) {
        Ok(n) => Ok(n),
        Err(_) => Err(Error::new(
            ErrorKind::IOError,
            Cow::Owned(format!("[{}] does not fit into i64.", src)),
        )),
    }
}

// this method is recursive, i.e. entries in subdirectories
// are also returned
//
// *nix hidden files/dirs are ignored
//
// returned vec is sorted by path
pub(crate) fn list_dir<P>(path: P) -> Result<Vec<(PathBuf, usize)>>
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
            entries.push((path, u64_to_usize(metadata.len())?));
        }
    }

    entries.sort_by(|&(ref p1, _), &(ref p2, _)| p1.cmp(p2));
    Ok(entries)
}

pub(crate) fn last_component<P>(path: P) -> Result<String>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    match path.file_name() {
        Some(s) => Ok(s.to_string_lossy().into_owned()),
        None => Err(Error::new(
            ErrorKind::IOError,
            Cow::Owned(format!("[{}] ends in \"..\".", path.display())),
        )),
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
            ].iter()
                .map(PathBuf::from)
                .map(|p| (p.clone(), p.metadata().unwrap().len() as usize))
                .collect::<Vec<(PathBuf, usize)>>()
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
            ].iter()
                .map(PathBuf::from)
                .map(|p| (p.clone(), p.metadata().unwrap().len() as usize))
                .collect::<Vec<(PathBuf, usize)>>()
        );
    }

    #[test]
    fn last_component_ok() {
        assert_eq!(
            last_component("/root/dir/file.ext").unwrap(),
            "file.ext".to_string()
        );
    }

    #[test]
    fn last_component_ok_2() {
        assert_eq!(
            last_component("/root/dir/dir2").unwrap(),
            "dir2".to_string()
        );
    }

    #[test]
    fn last_component_err() {
        match last_component("/root/dir/..") {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::IOError),
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
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::IOError),
        }
    }

    #[test]
    fn usize_to_i64_ok() {
        assert_eq!(usize_to_i64(42).unwrap(), 42);
    }

    #[test]
    fn usize_to_i64_err() {
        match usize_to_i64(usize::max_value()) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::IOError),
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
