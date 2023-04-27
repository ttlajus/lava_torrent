//! [`lava_torrent`] is a library for parsing/encoding/creating bencode and *.torrent* files. It is
//! dual-licensed under [Apache 2.0] and [MIT].
//!
//! # *Quick Start*
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
//! let torrent = TorrentBuilder::new("dir/", 1048576).build().unwrap();
//! torrent.write_into_file("sample.torrent").unwrap();
//! ```
//!
//! Bencode (de)serialization.
//!
//! ```no_run
//! use lava_torrent::bencode::BencodeElem;
//!
//! let bytes = "d4:spam4:eggse".as_bytes();
//! let dict = BencodeElem::Dictionary([("spam".to_owned(), "eggs".into())].into());
//!
//! assert_eq!(BencodeElem::from_bytes(bytes).unwrap()[0], dict);
//! assert_eq!(dict.encode(), bytes);
//!
//! assert!(dict.write_into_file("/tmp/foo").is_ok());
//! assert_eq!(BencodeElem::from_file("/tmp/foo").unwrap()[0], dict);
//! ```
//!
//! # *Overview*
//! - **It is not recommended to use [`lava_torrent`] in any critical system at this point.**
//! - Currently, only [v1] torrents are supported. [Merkle tree torrents] can be supported
//! if there's enough demand. [v2] torrents might be supported once it's stabilized.
//! - Methods for parsing and encoding are generally bound to structs (i.e. they are
//! "associated methods"). Methods that are general enough are placed at the module-level (e.g.
//! [`lava_torrent::bencode::write::encode_bytes()`]).
//!
//! ## Functionality
//! - bencode parsing/encoding (i.e. "bencoding/bdecoding") => [`BencodeElem`]
//! - torrent parsing/encoding (based on [`BencodeElem`]) => [`Torrent`]
//! - torrent creation => [`TorrentBuilder`]
//! - tracker response parsing => [`tracker`]
//!
//! ## Feature Flags
//! None at the moment.
//!
//! # *Correctness*
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
//! is found, a `LavaTorrentError` will be returned.
//!
//! 2. Several private methods will panic if something that "just won't happen"
//! happens. For the purpose of full disclosure this behavior is mentioned here,
//! but in reality panic should never be triggered. If you want to locate these
//! private methods try searching for "panic", "unwrap", and "expect" in `*.rs` files.
//!
//! # *Implemented BEPs*
//! NOTE: Only the parsing/encoding aspects are implemented.
//! - [BEP 3]
//! - [BEP 9] \(partial, only implemented magnet url v1)
//! - [BEP 12]
//! - [BEP 27]
//!
//! # *Other Stuff*
//! - Feature Request: To request a feature please open a GitHub issue (please
//! try to request only 1 feature per issue).
//! - Contribution: PR is always welcome.
//! - What's with "lava": Originally I intended to start a project for downloading/crawling
//! stuff. When downloading files, a stream of bits will be moving around--like lava.
//! - Other "lava" crates: The landscape for downloading/crawling stuff is fairly mature
//! at this point, which means reinventing the wheels now is rather pointless... So this
//! might be the only crate published under the "lava" name.
//! - Similar crates: [bip-rs]
//!
//! [`lava_torrent`]: index.html
//! [Apache 2.0]: https://www.apache.org/licenses/LICENSE-2.0
//! [MIT]: https://opensource.org/licenses/MIT
//! [profiling]: https://github.com/ttlajus/lava_torrent/wiki/Performance
//! [v1]: http://bittorrent.org/beps/bep_0003.html
//! [Merkle tree torrents]: http://bittorrent.org/beps/bep_0030.html
//! [v2]: http://bittorrent.org/beps/bep_0052.html
//! [`lava_torrent::bencode::write::encode_bytes()`]: bencode/write/fn.encode_bytes.html
//! [`BencodeElem`]: bencode/enum.BencodeElem.html
//! [`Torrent`]: torrent/v1/struct.Torrent.html
//! [`TorrentBuilder`]: torrent/v1/struct.TorrentBuilder.html
//! [`tracker`]: tracker/index.html
//! [BitTorrent specification]: http://bittorrent.org/beps/bep_0003.html
//! [BEP 3]: http://bittorrent.org/beps/bep_0003.html
//! [`bigint`]: https://github.com/rust-num/num-bigint
//! [`i64::max_value()`]: https://doc.rust-lang.org/stable/std/primitive.i64.html#method.max_value
//! [BEP 9]: http://bittorrent.org/beps/bep_0009.html
//! [BEP 12]: http://bittorrent.org/beps/bep_0012.html
//! [BEP 27]: http://bittorrent.org/beps/bep_0027.html
//! [bip-rs]: https://github.com/GGist/bip-rs

extern crate conv;
extern crate itertools;
extern crate percent_encoding;
extern crate rayon;
extern crate sha1;
extern crate thiserror;

pub(crate) mod util;
#[macro_use]
pub mod bencode;
pub mod torrent;
pub mod tracker;

use thiserror::Error;

/// Custom error.
///
#[derive(Error, Debug)]
pub enum LavaTorrentError {
    #[doc = "IO error occurred. \
    The bencode and the torrent may or may not be malformed \
    (as we can't verify that)."]
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[doc = "The bencode is found to be bad before we can parse \
    the torrent, so the torrent may or may not be malformed. \
    This is generally unexpected behavior and thus should be handled."]
    #[error("malformed bencode: {0}")]
    MalformedBencode(std::borrow::Cow<'static, str>),

    #[doc = "Bencode is fine, but parsed data is gibberish, so we \
    can't extract a torrent from it."]
    #[error("malformed torrent: {0}")]
    MalformedTorrent(std::borrow::Cow<'static, str>),

    #[doc = "Bencode is fine, but parsed data is gibberish, so we \
    can't extract a response from it."]
    #[error("malformed response: {0}")]
    MalformedResponse(std::borrow::Cow<'static, str>),

    #[doc = "`TorrentBuilder` encounters problems when \
    building `Torrent`. For instance, a field is set to \
    an empty string by the caller."]
    #[error("failed to build torrent: {0}")]
    TorrentBuilderFailure(std::borrow::Cow<'static, str>),

    #[doc = "An invalid argument is passed to a function."]
    #[error("invalid argument: {0}")]
    InvalidArgument(std::borrow::Cow<'static, str>),

    #[doc = "Conversion between numeric types (e.g. `i64 -> u64`) has failed."]
    #[error("numeric conversion failed: {0}")]
    FailedNumericConv(std::borrow::Cow<'static, str>),
}
