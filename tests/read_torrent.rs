extern crate conv;
extern crate lava_torrent;

use std::path::PathBuf;
use std::io::{BufReader, Read};
use std::collections::HashMap;
use std::iter::FromIterator;
use conv::ValueFrom;
use lava_torrent::bencode::BencodeElem;
use lava_torrent::torrent::v1::{File, Torrent};

#[test]
fn read_from_bytes() {
    let file = std::fs::File::open("tests/files/ubuntu-16.04.4-desktop-amd64.iso.torrent").unwrap();
    let mut bytes = Vec::new();
    BufReader::new(file).read_to_end(&mut bytes).unwrap();

    let parsed = Torrent::read_from_bytes(bytes).unwrap();
    assert_eq!(
        parsed.announce,
        "http://torrent.ubuntu.com:6969/announce".to_owned(),
    );
    assert_eq!(
        parsed.announce_list,
        Some(vec![
            vec!["http://torrent.ubuntu.com:6969/announce".to_owned()],
            vec!["http://ipv6.torrent.ubuntu.com:6969/announce".to_owned()],
        ]),
    );
    assert_eq!(parsed.length, 1_624_211_456);
    assert_eq!(parsed.files, None);
    assert_eq!(parsed.name, "ubuntu-16.04.4-desktop-amd64.iso".to_owned());
    assert_eq!(parsed.piece_length, 524_288);
    // Check the entire `pieces` vec is not very meaningful here...
    // So we check its length instead: len(pieces) == ceil(torrent_size / block_size).
    // The EPSILON comparison comes from
    // https://rust-lang-nursery.github.io/rust-clippy/v0.0.189/index.html#float_cmp
    assert!(
        (f64::value_from(parsed.pieces.len()).unwrap()
            - (f64::value_from(parsed.length).unwrap()
                / f64::value_from(parsed.piece_length).unwrap())
                .ceil())
            .abs() < std::f64::EPSILON
    );
    assert_eq!(
        parsed.extra_fields,
        Some(HashMap::from_iter(
            vec![
                (
                    "comment".to_owned(),
                    BencodeElem::String("Ubuntu CD releases.ubuntu.com".to_owned()),
                ),
                (
                    "creation date".to_owned(),
                    BencodeElem::Integer(1_519_934_077),
                ),
            ].into_iter()
        )),
    );
    assert_eq!(parsed.extra_info_fields, None);
    assert_eq!(
        parsed.info_hash(),
        "778ce280b595e57780ff083f2eb6f897dfa4a4ee".to_owned()
    );
    assert_eq!(
        parsed.magnet_link(),
        "magnet:?xt=urn:btih:778ce280b595e57780ff083f2eb6f897dfa4a4ee\
         &dn=ubuntu-16.04.4-desktop-amd64.iso\
         &tr=http://torrent.ubuntu.com:6969/announce\
         &tr=http://ipv6.torrent.ubuntu.com:6969/announce"
            .to_owned(),
    );
    assert!(!parsed.is_private());
}

#[test]
fn read_from_file() {
    let file = std::fs::File::open("tests/files/ubuntu-16.04.4-desktop-amd64.iso.torrent").unwrap();
    let mut bytes = Vec::new();
    BufReader::new(file).read_to_end(&mut bytes).unwrap();

    assert_eq!(
        Torrent::read_from_bytes(bytes),
        Torrent::read_from_file("tests/files/ubuntu-16.04.4-desktop-amd64.iso.torrent"),
    );
}

#[test]
fn read_from_bytes_multiple_files() {
    let file = std::fs::File::open("tests/files/tails-amd64-3.6.1.torrent").unwrap();
    let mut bytes = Vec::new();
    BufReader::new(file).read_to_end(&mut bytes).unwrap();

    let parsed = Torrent::read_from_bytes(bytes).unwrap();
    assert_eq!(
        parsed.announce,
        "http://linuxtracker.org:2710/00000000000000000000000000000000/announce".to_owned(),
    );
    assert_eq!(
        parsed.announce_list,
        Some(vec![
            vec!["udp://tracker.torrent.eu.org:451".to_owned()],
            vec!["udp://tracker.coppersurfer.tk:6969".to_owned()],
            vec![
                "http://linuxtracker.org:2710/00000000000000000000000000000000/announce".to_owned(),
            ],
        ]),
    );
    assert_eq!(parsed.length, 1_225_568_484);
    assert_eq!(
        parsed.files,
        Some(vec![
            File {
                length: 1_225_568_256,
                path: PathBuf::from("tails-amd64-3.6.1.iso"),
                extra_fields: None,
            },
            File {
                length: 228,
                path: PathBuf::from("tails-amd64-3.6.1.iso.sig"),
                extra_fields: None,
            },
        ])
    );
    assert_eq!(parsed.name, "tails-amd64-3.6.1".to_owned());
    assert_eq!(parsed.piece_length, 262_144);
    // Check the entire `pieces` vec is not very meaningful here...
    // So we check its length instead: len(pieces) == ceil(torrent_size / block_size).
    // The EPSILON comparison comes from
    // https://rust-lang-nursery.github.io/rust-clippy/v0.0.189/index.html#float_cmp
    assert!(
        (f64::value_from(parsed.pieces.len()).unwrap()
            - (f64::value_from(parsed.length).unwrap()
                / f64::value_from(parsed.piece_length).unwrap())
                .ceil())
            .abs() < std::f64::EPSILON
    );
    assert_eq!(
        parsed.extra_fields,
        Some(HashMap::from_iter(
            vec![
                (
                    "created by".to_owned(),
                    BencodeElem::String("mktorrent 1.0".to_owned()),
                ),
                (
                    "creation date".to_owned(),
                    BencodeElem::Integer(1_521_245_346),
                ),
            ].into_iter()
        )),
    );
    assert_eq!(parsed.extra_info_fields, None);
    assert_eq!(
        parsed.info_hash(),
        "a2a8d9b1ba0b1ac3d1ffa8062e02c0f9c23de31a".to_owned()
    );
    assert_eq!(
        parsed.magnet_link(),
        "magnet:?xt=urn:btih:a2a8d9b1ba0b1ac3d1ffa8062e02c0f9c23de31a\
         &dn=tails-amd64-3.6.1\
         &tr=udp://tracker.torrent.eu.org:451\
         &tr=udp://tracker.coppersurfer.tk:6969\
         &tr=http://linuxtracker.org:2710/00000000000000000000000000000000/announce"
            .to_owned(),
    );
    assert!(!parsed.is_private());
}

#[test]
fn read_from_files_multiple_files() {
    let file = std::fs::File::open("tests/files/tails-amd64-3.6.1.torrent").unwrap();
    let mut bytes = Vec::new();
    BufReader::new(file).read_to_end(&mut bytes).unwrap();

    assert_eq!(
        Torrent::read_from_bytes(bytes),
        Torrent::read_from_file("tests/files/tails-amd64-3.6.1.torrent"),
    );
}
