//! [`lava_torrent`] is a library for parsing/encoding bencode and *.torrent* files. It is
//! dual-licensed under Apache 2.0 and MIT. **It is not recommended to use [`lava_torrent`]
//! in any safety-critical system at this point.**
//!
//! Currently, only [v1] torrents are supported. [Merkle tree torrents] can be supported
//! if there's enough demand. [v2] torrents might be supported once it's stablized.
//!
//! Methods for parsing and encoding are generally bound to structs
//! (i.e. they are "associated methods"). Methods that are general
//! enough are placed at the module-level (e.g.
//! [`lava_torrent::bencode::write::encode_bytes()`]).
//!
//! # Quick Start
//! Read a torrent ([v1]) and print it and its info hash.
//!
//! ```no_run
//! use lava_torrent::torrent::v1::Torrent;
//!
//! let torrent = Torrent::read_from_file("sample.torrent").unwrap();
//! println!("{}", torrent);
//! println!("Info hash: {}", torrent.info_hash());
//! ```
//!
//! Create a torrent ([v1]) from files in a directory and save the *.torrent* file.
//!
//! ```no_run
//! use lava_torrent::torrent::v1::TorrentBuilder;
//!
//! let torrent = TorrentBuilder::new("announce".to_string(), "dir/", 1048576).build().unwrap();
//! torrent.write_into_file("sample.torrent").unwrap();
//! ```
//!
//! # Performance
//! [`lava_torrent`] is designed with performance and maintenance cost in mind.
//!
//! ## Copying
//! Parsing a *.torrent* ([v1]) file would take at least 2 copies:
//! - load bencode bytes from file
//! - parse from bytes (bytes are copied, for example, when they are converted to `String`)
//!
//! Creating a *.torrent* ([v1]) file and writing its bencoded form to disk
//! would take at least 2 copies:
//! - load file content and construct a [`Torrent`] from it
//! - encode the resulting struct and write it to disk (note: when encoding torrents, converting
//! file paths (`OsStr`) to utf8 strings (`String`) also requires copies, but that should
//! not be significant)
//!
//! It might be possible to further reduce the number of copies, but in my opinion that would
//! make the code harder to maintain. Unless there is evidence suggesting otherwise, I think
//! the current balance between performance and maintenance cost is good enough. Please open
//! a GitHub issue if you have any suggestion.
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
//! 2. Several private methods will panic if something that "just won't happen"
//! happens. For the purpose of full disclosure this behavior is mentioned here,
//! but in reality panic should never be triggered. If you want to locate these
//! private methods try searching for "panic", "unwrap", and "expect" in `*.rs` files.
//!
//! # Implemented BEPs
//! NOTE: Only the parsing/encoding aspects are implemented.
//! - [BEP 3]
//! - [BEP 9] \(partial, only implemented magnet url v1)
//! - [BEP 12]
//! - [BEP 27]
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
//! [v1]: http://bittorrent.org/beps/bep_0003.html
//! [Merkle tree torrents]: http://bittorrent.org/beps/bep_0030.html
//! [v2]: http://bittorrent.org/beps/bep_0052.html
//! [`lava_torrent::bencode::write::encode_bytes()`]: bencode/write/fn.encode_bytes.html
//! [`Torrent`]: torrent/v1/struct.Torrent.html
//! [BitTorrent specification]: http://bittorrent.org/beps/bep_0003.html
//! [BEP 3]: http://bittorrent.org/beps/bep_0003.html
//! [`bigint`]: https://github.com/rust-num/num-bigint
//! [`i64::max_value()`]: https://doc.rust-lang.org/stable/std/primitive.i64.html#method.max_value
//! [BEP 9]: http://bittorrent.org/beps/bep_0009.html
//! [BEP 12]: http://bittorrent.org/beps/bep_0012.html
//! [BEP 27]: http://bittorrent.org/beps/bep_0027.html
//! [bip-rs]: https://github.com/GGist/bip-rs

extern crate conv;
extern crate crypto;
extern crate itertools;
extern crate unicode_normalization;

use std::fmt;
use std::borrow::Cow;
use std::convert::From;

#[macro_use]
pub mod bencode;
pub mod torrent;

/// Custom `Result` type.
pub type Result<T> = std::result::Result<T, Error>;

/// Custom `Error` type.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Error {
    kind: ErrorKind,
    msg: Cow<'static, str>,
}

/// Works with [`Error`](struct.Error.html) to differentiate between different kinds of errors.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ErrorKind {
    /// The bencode is found to be bad before we can parse the torrent,
    /// so the torrent may or may not be malformed.
    MalformedBencode,
    /// IO error occurred. The bencode and the torrent may or may not
    /// be malformed (as we can't verify that).
    IOError,
    /// Bencode is fine, but parsed data is gibberish, so we can't extract
    /// a torrent from it.
    MalformedTorrent,
    /// `TorrentBuilder` encounters problems when building `Torrent`. For
    /// instance, a field is set to an empty string by the caller.
    TorrentBuilderFailure,
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
        write!(f, "{:?}: {}", self.kind, self.msg)
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        &self.msg
    }
}

impl From<std::io::Error> for Error {
    // @todo: better conversion (e.g. save cause)?
    fn from(e: std::io::Error) -> Error {
        Error::new(ErrorKind::IOError, Cow::Owned(format!("IO error: {}.", e)))
    }
}
