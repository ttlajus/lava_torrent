//! Module for `.torrent` files ([v1](http://bittorrent.org/beps/bep_0003.html))
//! related parsing/encodeing.

use std::fmt;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use crypto::sha1::Sha1;
use crypto::digest::Digest;
use itertools::Itertools;
use bencode::BencodeElem;
use {Error, ErrorKind, Result};

mod read;
mod write;
mod build;

const PIECE_STRING_LENGTH: usize = 20;

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

/// A file contained in a torrent.
///
/// Modeled after the specifications
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

/// Everything found in a *.torrent* file.
///
/// Modeled after the specifications
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
}

/// Struct type for creating `Torrent`s from files.
///
/// This struct is used for **creating** `Torrent`s, so that you can
/// encode/serialize them to *.torrent* files. If you want to read
/// existing *.torrent* files then use [`Torrent::read_from_file()`]
/// or [`Torrent::read_from_bytes()`].
///
/// Required fields: `announce`, `path`, and `piece_length`.
/// They are set when calling the constructor [`new()`].
///
/// Optional fields can be set by calling the corresponding methods
///  (e.g. [`set_announce()`]). Fields can be updated in the same way.
///
/// **Symbolic links and \*nix hidden files/dirs are ignored.** Reasoning:
///
///
/// [`Torrent::read_from_file()`]: struct.Torrent.html#method.read_from_file
/// [`Torrent::read_from_bytes()`]: struct.Torrent.html#method.read_from_bytes
/// [`new()`]: #method.new
/// [`set_announce()`]: #method.set_announce
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TorrentBuilder {
    announce: String,
    announce_list: Option<AnnounceList>,
    name: Option<String>,
    path: PathBuf,
    piece_length: Integer,
    extra_fields: Option<Dictionary>,
    extra_info_fields: Option<Dictionary>,
    is_private: bool,
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
    /// Construct the `info` dict based on the fields of `self`.
    ///
    /// Certain operations on torrents, such as calculating info
    /// hashs, require the extracted `info` dict. This
    /// convenience method does that.
    ///
    /// Note that the `info` dict
    /// is constructed each time this method is called (i.e.
    /// the return value is not cached). If caching is needed
    /// then the caller should handle that.
    ///
    /// Since `self` is taken by reference, and the result is
    /// returned by value, certain values will be cloned. Please
    /// be aware of this overhead.
    pub fn construct_info(&self) -> BencodeElem {
        let mut info: HashMap<String, BencodeElem> = HashMap::new();

        if let Some(ref files) = self.files {
            info.insert(
                "files".to_string(),
                BencodeElem::List(
                    files
                        .clone()
                        .into_iter()
                        .map(|file| file.into_bencode_elem())
                        .collect(),
                ),
            );
        } else {
            info.insert("length".to_string(), BencodeElem::Integer(self.length));
        }

        info.insert("name".to_string(), BencodeElem::String(self.name.clone()));
        info.insert(
            "piece length".to_string(),
            BencodeElem::Integer(self.piece_length),
        );
        info.insert(
            "pieces".to_string(),
            BencodeElem::Bytes(
                self.pieces
                    .clone()
                    .into_iter()
                    .flat_map(|piece| piece)
                    .collect(),
            ),
        );

        if let Some(ref extra_info_fields) = self.extra_info_fields {
            info.extend(extra_info_fields.clone());
        }

        BencodeElem::Dictionary(info)
    }

    /// Calculate the `Torrent`'s info hash as defined in
    /// [BEP 3](http://bittorrent.org/beps/bep_0003.html).
    ///
    /// Note that the calculated info hash is not cached.
    /// So if this method is called multiple times, multiple
    /// calculations will be performed. To avoid that, the
    /// caller should cache the return value as needed.
    pub fn info_hash(&self) -> String {
        let mut hasher = Sha1::new();
        hasher.input(&self.construct_info().encode());
        hasher.result_str()
    }

    /// Calculate the `Torrent`'s magnet link as defined in
    /// [BEP 9](http://bittorrent.org/beps/bep_0009.html).
    ///
    /// The `dn` parameter is set to `self.name`.
    ///
    /// Either `self.announce` or all trackers in `self.announce_list` will be used,
    /// meaning that there might be multiple `tr` entries. We don't use both because
    /// per [BEP 12](http://bittorrent.org/beps/bep_0012.html):
    /// "If the client is compatible with the multitracker specification, and if the
    /// `announce-list` key is present, the client will ignore the `announce` key
    /// and only use the URLs in `announce-list`."
    ///
    /// The `x.pe` parameter (for peer addresses) is currently not supported.
    pub fn magnet_link(&self) -> String {
        if let Some(ref list) = self.announce_list {
            format!(
                "magnet:?xt=urn:btih:{}&dn={}{}",
                self.info_hash(),
                self.name,
                list.iter().format_with("", |tier, f| f(&format_args!(
                    "{}",
                    tier.iter()
                        .format_with("", |url, f| f(&format_args!("&tr={}", url)))
                ))),
            )
        } else {
            format!(
                "magnet:?xt=urn:btih:{}&dn={}&tr={}",
                self.info_hash(),
                self.name,
                self.announce,
            )
        }
    }

    /// Check if this torrent is private as defined in
    /// [BEP 27](http://bittorrent.org/beps/bep_0027.html).
    ///
    /// Returns `true` if `private` maps to a bencode integer `1`.
    /// Returns `false` otherwise.
    pub fn is_private(&self) -> bool {
        if let Some(ref dict) = self.extra_info_fields {
            match dict.get("private") {
                Some(&BencodeElem::Integer(val)) => val == 1,
                Some(_) => false,
                None => false,
            }
        } else {
            false
        }
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
                    .sorted_by_key(|&(key, _)| key.as_bytes())
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
                    ::itertools::join(tier, ", ")
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
                    .sorted_by_key(|&(key, _)| key.as_bytes())
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
                    .sorted_by_key(|&(key, _)| key.as_bytes())
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
    use std::iter::FromIterator;

    #[test]
    fn construct_info_ok() {
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
        };

        assert_eq!(
            torrent.construct_info(),
            bencode_elem!({
                ("length", 4),
                ("name", "sample"),
                ("piece length", 2),
                ("pieces", (1, 2, 3, 4))}),
        );
    }

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
        };

        assert_eq!(
            torrent.info_hash(),
            "074f42efaf8267f137f114f722d4e7d1dcbfbda5".to_string(),
        );
    }

    #[test]
    fn magnet_link_ok() {
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
        };

        assert_eq!(
            torrent.magnet_link(),
            "magnet:?xt=urn:btih:074f42efaf8267f137f114f722d4e7d1dcbfbda5\
             &dn=sample&tr=url"
                .to_string()
        );
    }

    #[test]
    fn magnet_link_with_announce_list() {
        let torrent = Torrent {
            announce: "url".to_string(),
            announce_list: Some(vec![
                vec!["url1".to_string()],
                vec!["url2".to_string(), "url3".to_string()],
            ]),
            length: 4,
            files: None,
            name: "sample".to_string(),
            piece_length: 2,
            pieces: vec![vec![1, 2], vec![3, 4]],
            extra_fields: None,
            extra_info_fields: None,
        };

        assert_eq!(
            torrent.magnet_link(),
            "magnet:?xt=urn:btih:074f42efaf8267f137f114f722d4e7d1dcbfbda5\
             &dn=sample&tr=url1&tr=url2&tr=url3"
                .to_string()
        );
    }

    #[test]
    fn is_private_ok() {
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
                vec![("private".to_string(), bencode_elem!(1))].into_iter(),
            )),
        };

        assert!(torrent.is_private());
    }

    #[test]
    fn is_private_no_extra_fields() {
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
        };

        assert!(!torrent.is_private());
    }

    #[test]
    fn is_private_no_key() {
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
                vec![("privatee".to_string(), bencode_elem!(1))].into_iter(),
            )),
        };

        assert!(!torrent.is_private());
    }

    #[test]
    fn is_private_incorrect_val_type() {
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
                vec![("privatee".to_string(), bencode_elem!("1"))].into_iter(),
            )),
        };

        assert!(!torrent.is_private());
    }

    #[test]
    fn is_private_incorrect_val() {
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
                vec![("privatee".to_string(), bencode_elem!(2))].into_iter(),
            )),
        };

        assert!(!torrent.is_private());
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
