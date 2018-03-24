// read (generally 3 copies): load from file + parse from bytes + re-encode info
// write (generally 2 copies): make torrent from file + write to file
// When bencoding torrents, converting file paths (OsStr) to utf8 strings (String) also requires copies,
// but that should not be significant.
extern crate conv;
extern crate crypto;
extern crate itertools;
extern crate unicode_normalization;

use std::fmt;
use std::path::Path;
use std::borrow::Cow;
use std::convert::From;
use std::collections::HashMap;
use std::path::PathBuf;
use crypto::sha1::Sha1;
use crypto::digest::Digest;
use itertools::Itertools;
use bencode::BencodeElem;

#[macro_use]
pub mod bencode;
mod read;
mod write;

const PIECE_STRING_LENGTH: usize = 20;

pub type Result<T> = std::result::Result<T, Error>;
pub type Dictionary = HashMap<String, BencodeElem>;
pub type AnnounceList = Vec<Vec<String>>;
pub type Piece = Vec<u8>;
pub type Integer = i64;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Error {
    kind: ErrorKind,
    msg: Cow<'static, str>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ErrorKind {
    MalformedBencode,
    IOError,
    MalformedTorrent, // bencode is fine, but parsed data is gibberish
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct File {
    pub length: Integer,                  // in bytes
    pub path: PathBuf,                    // relative to `Torrent.name`
    pub extra_fields: Option<Dictionary>, // fields not defined in BEP 3
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Torrent {
    pub announce: String,
    pub announce_list: Option<AnnounceList>,   // BEP 12
    pub length: Integer,                       // total size in bytes
    pub files: Option<Vec<File>>,              // if single file then `name` is file name
    pub name: String,                          // suggested directory name
    pub piece_length: Integer,                 // block size in bytes
    pub pieces: Vec<Piece>,                    // SHA1 hashs of blocks
    pub extra_fields: Option<Dictionary>,      // top-level fields not defined in BEP 3
    pub extra_info_fields: Option<Dictionary>, // fields in `info` not defined in BEP 3
    encoded_info: Vec<u8>,                     // bencoded `info` dict
}

impl Error {
    fn new(kind: ErrorKind, msg: Cow<'static, str>) -> Error {
        Error { kind, msg }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:?} error when decoding bencode: {}",
            self.kind, self.msg
        )
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        &self.msg
    }
}

impl From<std::io::Error> for Error {
    // @todo: better conversion (e.g. save cause)?
    fn from(_: std::io::Error) -> Error {
        Error::new(
            ErrorKind::IOError,
            Cow::Borrowed("IO error when writing bencode/torrent."),
        )
    }
}

impl File {
    // caller has to ensure that `parent` is an absolute path and
    // that it is the parent of `self.path`
    pub fn absolute_path<P>(&self, parent: P) -> Result<PathBuf>
    where
        P: AsRef<Path>,
    {
        let result = parent.as_ref().join(&self.path);
        if result.is_absolute() {
            Ok(result)
        } else {
            Err(Error::new(
                ErrorKind::IOError,
                Cow::Borrowed("Joined path is not absolute."),
            ))
        }
    }
}

impl Torrent {
    pub fn info_hash(&self) -> String {
        let mut hasher = Sha1::new();
        hasher.input(&self.encoded_info);
        hasher.result_str()
    }
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}\n\
             -size: {} bytes\n",
            self.path.as_path().display(),
            self.length
        )?;

        if let Some(ref fields) = self.extra_fields {
            write!(
                f,
                "{}",
                fields
                    .iter()
                    .sorted_by(|&(k1, _), &(k2, _)| k1.as_bytes().cmp(k2.as_bytes()))
                    .iter()
                    .format_with("", |&(k, v), f| f(&format_args!("-{}: {}\n", k, v)))
            )?;
        }

        write!(f, "========================================\n")
    }
}

impl fmt::Display for Torrent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.torrent\n", self.name)?;
        write!(f, "-announce: {}\n", self.announce)?;
        if let Some(ref tiers) = self.announce_list {
            write!(
                f,
                "-announce-list: [{}]\n",
                tiers.iter().format_with(", ", |tier, f| f(&format_args!(
                    "[{}]",
                    itertools::join(tier, ", ")
                )))
            )?;
        }
        write!(f, "-size: {} bytes\n", self.length)?;
        write!(f, "-piece length: {} bytes\n", self.piece_length)?;

        if let Some(ref fields) = self.extra_fields {
            write!(
                f,
                "{}",
                fields
                    .iter()
                    .sorted_by(|&(k1, _), &(k2, _)| k1.as_bytes().cmp(k2.as_bytes()))
                    .iter()
                    .format_with("", |&(k, v), f| f(&format_args!("-{}: {}\n", k, v)))
            )?;
        }

        if let Some(ref fields) = self.extra_info_fields {
            write!(
                f,
                "{}",
                fields
                    .iter()
                    .sorted_by(|&(k1, _), &(k2, _)| k1.as_bytes().cmp(k2.as_bytes()))
                    .iter()
                    .format_with("", |&(k, v), f| f(&format_args!("-{}: {}\n", k, v)))
            )?;
        }

        if let Some(ref files) = self.files {
            write!(f, "-files:\n")?;
            for (counter, file) in files.iter().enumerate() {
                write!(f, "[{}] {}\n", counter + 1, file)?;
            }
        }

        write!(
            f,
            "-pieces: [{}]\n",
            self.pieces
                .iter()
                .format_with(", ", |piece, f| f(&format_args!(
                    "[{:02x}]",
                    piece.iter().format("")
                ))),
        )
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;

    #[test]
    fn absolute_path_ok() {
        let file = File {
            length: 42,
            path: PathBuf::from("dir1/file"),
            extra_fields: None,
        };

        match file.absolute_path("/root") {
            Ok(joined) => assert_eq!(joined, PathBuf::from("/root/dir1/file")),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn absolute_path_not_absolute() {
        let file = File {
            length: 42,
            path: PathBuf::from("dir1/file"),
            extra_fields: None,
        };

        match file.absolute_path("root") {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::IOError),
        }
    }
}

#[cfg(test)]
mod torrent_tests {
    use super::*;

    #[test]
    fn info_hash_ok() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
            encoded_info: vec![b'd', b'e'],
        };

        assert_eq!(
            torrent.info_hash(),
            "600ccd1b71569232d01d110bc63e906beab04d8c".to_string(),
        );
    }
}

#[cfg(test)]
mod file_display_tests {
    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn file_display_ok() {
        let file = File {
            length: 42,
            path: PathBuf::from("dir1/file"),
            extra_fields: None,
        };

        assert_eq!(
            file.to_string(),
            "dir1/file\n\
             -size: 42 bytes\n\
             ========================================\n"
        );
    }

    #[test]
    fn file_display_with_extra_fields() {
        let file = File {
            length: 42,
            path: PathBuf::from("dir1/file"),
            extra_fields: Some(HashMap::from_iter(
                vec![
                    ("comment2".to_string(), bencode_elem!("no comment")),
                    ("comment1".to_string(), bencode_elem!("no comment")),
                ].into_iter(),
            )),
        };

        assert_eq!(
            file.to_string(),
            "dir1/file\n\
             -size: 42 bytes\n\
             -comment1: \"no comment\"\n\
             -comment2: \"no comment\"\n\
             ========================================\n"
        );
    }
}

#[cfg(test)]
mod torrent_display_tests {
    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn torrent_display_ok() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
            encoded_info: Vec::new(),
        };

        assert_eq!(
            torrent.to_string(),
            "sample.torrent\n\
             -announce: url\n\
             -size: 4 bytes\n\
             -piece length: 2 bytes\n\
             -pieces: [[0102], [0304]]\n"
        );
    }

    #[test]
    fn torrent_display_with_announce_list() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: Some(vec![
                vec!["url1".to_string(), "url2".to_string()],
                vec!["url3".to_string(), "url4".to_string()],
            ]),
            length: 4,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
            encoded_info: Vec::new(),
        };

        assert_eq!(
            torrent.to_string(),
            "sample.torrent\n\
             -announce: url\n\
             -announce-list: [[url1, url2], [url3, url4]]\n\
             -size: 4 bytes\n\
             -piece length: 2 bytes\n\
             -pieces: [[0102], [0304]]\n"
        );
    }

    #[test]
    fn torrent_display_with_extra_fields() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: Some(HashMap::from_iter(
                vec![
                    ("comment2".to_string(), bencode_elem!("no comment")),
                    ("comment1".to_string(), bencode_elem!("no comment")),
                ].into_iter(),
            )),
            extra_info_fields: None,
            encoded_info: Vec::new(),
        };

        assert_eq!(
            torrent.to_string(),
            "sample.torrent\n\
             -announce: url\n\
             -size: 4 bytes\n\
             -piece length: 2 bytes\n\
             -comment1: \"no comment\"\n\
             -comment2: \"no comment\"\n\
             -pieces: [[0102], [0304]]\n"
        );
    }

    #[test]
    fn torrent_display_with_extra_info_fields() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: None,
            length: 4,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: Some(HashMap::from_iter(
                vec![
                    ("comment2".to_string(), bencode_elem!("no comment")),
                    ("comment1".to_string(), bencode_elem!("no comment")),
                ].into_iter(),
            )),
            encoded_info: Vec::new(),
        };

        assert_eq!(
            torrent.to_string(),
            "sample.torrent\n\
             -announce: url\n\
             -size: 4 bytes\n\
             -piece length: 2 bytes\n\
             -comment1: \"no comment\"\n\
             -comment2: \"no comment\"\n\
             -pieces: [[0102], [0304]]\n"
        );
    }

    #[test]
    fn torrent_display_with_multiple_files() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: None,
            length: 4,
            files: Some(vec![
                File {
                    length: 2,
                    path: PathBuf::from("dir1/dir2/file1"),
                    extra_fields: None,
                },
                File {
                    length: 2,
                    path: PathBuf::from("dir1/dir2/file2"),
                    extra_fields: None,
                },
            ]),
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
            encoded_info: Vec::new(),
        };

        assert_eq!(
            torrent.to_string(),
            "sample.torrent\n\
             -announce: url\n\
             -size: 4 bytes\n\
             -piece length: 2 bytes\n\
             -files:\n\
             [1] dir1/dir2/file1\n\
             -size: 2 bytes\n\
             ========================================\n\
             \n\
             [2] dir1/dir2/file2\n\
             -size: 2 bytes\n\
             ========================================\n\
             \n\
             -pieces: [[0102], [0304]]\n"
        );
    }
}
