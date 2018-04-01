//! [`lava_torrent`] is a library for parsing/encoding bencode and *.torrent* files. It is
//! dual-licensed under Apache 2.0 and MIT. **It is not recommended to use [`lava_torrent`]
//! in any safety-critical system at this point.**
//!
//! Methods for parsing and encoding are generally bound to structs
//! (i.e. they are "associated methods"). Methods that are general
//! enough are placed at the module-level (e.g.
//! [`lava_torrent::bencode::write::encode_bytes()`]).
//!
//! # Quick Start
//! Read a torrent and print it and its info hash.
//!
//! ```rust
//! use lava_torrent::Torrent;
//!
//! let torrent = Torrent::read_from_file("sample.torrent").unwrap();
//! println!("{}", torrent);
//! println!("Info hash: {}", torrent.info_hash());
//! ```
//!
//! Create a torrent from files in a directory and save the *.torrent* file.
//!
//! ```rust
//! use lava_torrent::Torrent;
//!
//! let torrent = Torrent::create_from_file("dir/").unwrap();
//! torrent.write_into_file("sample.torrent").unwrap();
//! ```
//!
//! # Performance
//! [`lava_torrent`] is designed with performance and maintenance cost in mind.
//!
//! ## Copying
//! Parsing a *.torrent* file would take at least 3 copies:
//! * load bencode bytes from file
//! * parse from bytes (bytes are copied, for example, when they are converted to `String`)
//! * re-encode `info` (the `info` dictionary has to be converted back to bencode form so that
//! we can do things like calculating info hash)
//!
//! Creating a *.torrent* file and writing its bencoded form to disk would take at least 2 copies:
//! * load file content and construct a [`Torrent`] from it
//! * encode the resulting struct and write it to disk (note: when encoding torrents, converting
//! file paths (`OsStr`) to utf8 strings (`String`) also requires copies, but that should
//! not be significant)
//!
//! It might be possible to further reduce the number of copies, but in my opinion that would
//! make the code harder to maintain. Unless there is evidence suggesting otherwise, I think
//! the current balance between performance and maintenance cost is good enough. Please open
//! a GitHub issue if you have any suggestion.
//!
//! ## Memory Usage
//! The re-encoded `info` dict is stored in a field of the [`Torrent`] struct. Since this
//! `info` dict is fairly large (it occupies the majority of a *.torrent* file), [`lava_torrent`]
//! cannot be considered memory-efficient at this point. An alternative approach would be
//! to calculate everything we could after re-encoding, and store the calculated results
//! instead. However, I think the current approach of storing the encoded `info` dict is
//! more convenient and extensible.
//!
//! Of course, on modern computers this bit of memory inefficiency is mostly irrelevant.
//! But on embedded devices this might actually matter.
//!
//! # Correctness
//! [`lava_torrent`] is written without using any existing parser or parser generator.
//! The [BitTorrent specification] is also rather vague on certain points. Thus, bugs
//! should not be a surprise. If you do find one, please open a GitHub issue.
//!
//! That said, a lot of unit tests and several integration tests are written to minimize the
//! possibility of incorrect behaviors.
//!
//! ## Known Issues
//! 1. [BEP 3] specifies that a bencode integer has no
//! size limit. This is a reasonable choice as it allows the protocol to be used
//! in the future when file sizes grow significantly. However, using a 64-bit signed
//! integer to represent a bencode integer should be more-than sufficient even in 2018.
//! Therefore, while technically we should use something like
//! [`bigint`] to represent bencode integers,
//! `i64` is used in the current implementation. If a bencode integer larger than
//! [`i64::max_value()`]
//! is found, an `Error` will be returned.
//!
//! # Other Stuff
//! - Feature Request: To request a feature please open a GitHub issue (please
//! try to request only 1 feature per issue).
//! - Contribute: PR is always welcome.
//! - What's with "lava": Originally I intended to start a project for downloading/crawling
//! stuff. When downloading files, a stream of bits will be moving around--like lava.
//! - Other "lava" crates: The landscape for downloading/crawling stuff is fairly mature
//! at this point, which means reinventing the wheels now is rather pointless... So this
//! might be the only crate published under the "lava" name.
//! - Similar crates: [bip-rs]
//!
//! [`lava_torrent`]: index.html
//! [`lava_torrent::bencode::write::encode_bytes()`]: bencode/write/fn.encode_bytes.html
//! [`Torrent`]: struct.Torrent.html
//! [BitTorrent specification]: http://bittorrent.org/beps/bep_0003.html
//! [BEP 3]: http://bittorrent.org/beps/bep_0003.html
//! [`bigint`]: https://github.com/rust-num/num-bigint
//! [`i64::max_value()`]: https://doc.rust-lang.org/stable/std/primitive.i64.html#method.max_value
//! [bip-rs]: https://github.com/GGist/bip-rs

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

/// Custom `Result` type.
pub type Result<T> = std::result::Result<T, Error>;
/// Corresponds to a bencode dictionary.
pub type Dictionary = HashMap<String, BencodeElem>;
/// Corresponds to the `announce-list` in [BEP 12](http://bittorrent.org/beps/bep_0012.html).
pub type AnnounceList = Vec<Vec<String>>;
/// A piece in `pieces`--the SHA1 hash of a torrent block.
pub type Piece = Vec<u8>;
/// Corresponds to a bencode integer. The underlying type is `i64`.
/// Technically a bencode integer has no size limit, but it is not
/// so in the current implementation. By using a type alias it is
/// easier to change the underlying type in the future.
pub type Integer = i64;

/// Custom `Error` type.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Error {
    kind: ErrorKind,
    msg: Cow<'static, str>,
}

/// Works with [`Error`](struct.Error.html) to differentiate between different kinds of errors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ErrorKind {
    /// The bencode is found to be bad before we can parse the torrent,
    /// so the torrent may or may not be malformed.
    MalformedBencode,
    /// IO error occured. The bencode and the torrent may or may not
    /// be malformed (as we can't verify that).
    IOError,
    /// Bencode is fine, but parsed data is gibberish, so we can't extract
    /// a torrent from it.
    MalformedTorrent,
}

/// A file contained in a torrent. Modeled after the specifications
/// in [BEP 3](http://bittorrent.org/beps/bep_0003.html). Unknown/extension
/// fields will be placed in `extra_fields`. If you need
/// any of those extra fields you would have to parse it yourself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct File {
    /// File size in bytes.
    pub length: Integer,
    /// File path, relative to [`Torrent`](struct.Torrent.html)'s `name` field.
    pub path: PathBuf,
    /// Fields not defined in [BEP 3](http://bittorrent.org/beps/bep_0003.html).
    pub extra_fields: Option<Dictionary>,
}

/// Everything found in a *.torrent* file. Modeled after the specifications
/// in [BEP 3](http://bittorrent.org/beps/bep_0003.html) and
///  [BEP 12](http://bittorrent.org/beps/bep_0012.html). Unknown/extension
/// fields will be placed in `extra_fields` (if the unknown
/// fields are found in the `info` dictionary then they are placed in
/// `extra_info_fields`). If you need any of those extra fields you would
/// have to parse it yourself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Torrent {
    /// URL of the torrent's tracker.
    pub announce: String,
    /// Announce list as defined in [BEP 12](http://bittorrent.org/beps/bep_0012.html).
    pub announce_list: Option<AnnounceList>,
    /// Total torrent size in bytes (i.e. sum of all files' sizes).
    pub length: Integer,
    /// If the torrent contains only 1 file then `files` is `None`.
    pub files: Option<Vec<File>>,
    /// If the torrent contains only 1 file then `name` is the file name.
    /// Otherwise it's the suggested root directory's name.
    pub name: String,
    /// Block size in bytes.
    pub piece_length: Integer,
    /// SHA1 hashs of each block.
    pub pieces: Vec<Piece>,
    /// Top-level fields not defined in [BEP 3](http://bittorrent.org/beps/bep_0003.html).
    pub extra_fields: Option<Dictionary>,
    /// Fields in `info` not defined in [BEP 3](http://bittorrent.org/beps/bep_0003.html).
    pub extra_info_fields: Option<Dictionary>,
    encoded_info: Vec<u8>, // bencoded `info` dict
}

impl Error {
    fn new(kind: ErrorKind, msg: Cow<'static, str>) -> Error {
        Error { kind, msg }
    }

    /// Return the kind of this error.
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
    /// Construct the `File`'s absolute path using `parent`.
    ///
    /// Caller has to ensure that `parent` is an absolute path.
    /// Otherwise an error would be returned.
    ///
    /// This method effectively appends/joins `self.path` to `parent`.
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
    /// Calculate the `Torrent`'s info hash as defined in
    /// [BEP 3](http://bittorrent.org/beps/bep_0003.html).
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
