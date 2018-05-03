extern crate lava_torrent;
extern crate rand;

use std::path::PathBuf;
use rand::Rng;
use lava_torrent::ErrorKind;
use lava_torrent::bencode::BencodeElem;
use lava_torrent::torrent::v1::{Integer, Torrent, TorrentBuilder};

const OUTPUT_ROOT: &str = "tests/tmp/";
const PIECE_LENGTH: Integer = 32 * 1024; // n * 1024 KiB

fn rand_file_name() -> String {
    OUTPUT_ROOT.to_owned() + &rand::thread_rng().gen::<u16>().to_string()
}

#[test]
fn build_single_file_ok() {
    let output_name = rand_file_name() + ".torrent";

    let builder = TorrentBuilder::new(
        "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
        PathBuf::from("tests/files/tails-amd64-3.6.1.torrent")
            .canonicalize()
            .unwrap(),
        PIECE_LENGTH,
    );
    // create a new chain because rustfmt does
    // weird things when `builder` is not declared separately
    builder
        .add_extra_field("creation date".to_owned(), BencodeElem::Integer(1523448537))
        .add_extra_field(
            "encoding".to_owned(),
            BencodeElem::String("UTF-8".to_owned()),
        )
        .add_extra_info_field("private".to_owned(), BencodeElem::Integer(0))
        .build()
        .unwrap()
        .write_into_file(&output_name)
        .unwrap();

    // compare against a sample file created by Deluge
    assert_eq!(
        Torrent::read_from_file(output_name).unwrap(),
        Torrent::read_from_file("tests/samples/tails-amd64-3.6.1.torrent.torrent").unwrap(),
    );
}

#[test]
fn build_multi_file_ok() {
    let output_name = rand_file_name() + ".torrent";

    let builder = TorrentBuilder::new(
        "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
        PathBuf::from("tests/files").canonicalize().unwrap(),
        PIECE_LENGTH,
    );
    // create a new chain because rustfmt does
    // weird things when `builder` is not declared separately
    builder
        .add_extra_field("creation date".to_owned(), BencodeElem::Integer(1523607302))
        .add_extra_field(
            "encoding".to_owned(),
            BencodeElem::String("UTF-8".to_owned()),
        )
        .add_extra_info_field("private".to_owned(), BencodeElem::Integer(0))
        .build()
        .unwrap()
        .write_into_file(&output_name)
        .unwrap();

    // compare against a sample file created by Deluge
    assert_eq!(
        Torrent::read_from_file(output_name).unwrap(),
        Torrent::read_from_file("tests/samples/files.torrent").unwrap(),
    );
}

#[test]
fn build_with_name() {
    let output_name = rand_file_name() + ".torrent";

    TorrentBuilder::new(
        "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
        PathBuf::from("tests/files/tails-amd64-3.6.1.torrent")
            .canonicalize()
            .unwrap(),
        PIECE_LENGTH,
    ).set_name("file".to_owned())
        .build()
        .unwrap()
        .write_into_file(&output_name)
        .unwrap();

    assert_eq!(
        Torrent::read_from_file(output_name).unwrap().name,
        "file".to_owned(),
    );
}

#[test]
fn build_private() {
    let output_name = rand_file_name() + ".torrent";

    let builder = TorrentBuilder::new(
        "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
        PathBuf::from("tests/files").canonicalize().unwrap(),
        PIECE_LENGTH,
    );
    // create a new chain because rustfmt does
    // weird things when `builder` is not declared separately
    builder
        .add_extra_field("creation date".to_owned(), BencodeElem::Integer(1523607445))
        .add_extra_field(
            "encoding".to_owned(),
            BencodeElem::String("UTF-8".to_owned()),
        )
        .set_privacy(true)
        .build()
        .unwrap()
        .write_into_file(&output_name)
        .unwrap();

    // compare against a sample file created by Deluge
    assert_eq!(
        Torrent::read_from_file(output_name).unwrap(),
        Torrent::read_from_file("tests/samples/files-private.torrent").unwrap(),
    );
}

#[test]
fn build_symbolic_link() {
    let output_name = rand_file_name() + ".torrent";
    // `canonicalize()` follows symbolic links, so we have to do a
    // `push()` separately to avoid that resolution
    let mut path = PathBuf::from("tests/files").canonicalize().unwrap();
    path.push("symlink");

    let builder = TorrentBuilder::new(
        "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
        path,
        PIECE_LENGTH,
    );
    // create a new chain because rustfmt does
    // weird things when `builder` is not declared separately
    builder
        .add_extra_field("creation date".to_owned(), BencodeElem::Integer(1523607602))
        .add_extra_field(
            "encoding".to_owned(),
            BencodeElem::String("UTF-8".to_owned()),
        )
        .add_extra_info_field("private".to_owned(), BencodeElem::Integer(0))
        .build()
        .unwrap()
        .write_into_file(&output_name)
        .unwrap();

    // compare against a sample file created by Deluge
    assert_eq!(
        Torrent::read_from_file(output_name).unwrap(),
        Torrent::read_from_file("tests/samples/symlink.torrent").unwrap(),
    );
}

#[test]
fn build_hidden_file() {
    let result = TorrentBuilder::new(
        "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
        PathBuf::from("tests/files/.hidden").canonicalize().unwrap(),
        PIECE_LENGTH,
    ).build();

    match result {
        Ok(_) => assert!(false),
        Err(e) => assert_eq!(e.kind(), ErrorKind::TorrentBuilderFailure),
    }
}
