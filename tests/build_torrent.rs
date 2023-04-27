extern crate lava_torrent;
extern crate rand;

use lava_torrent::bencode::BencodeElem;
use lava_torrent::torrent::v1::{Integer, Torrent, TorrentBuilder};
use lava_torrent::LavaTorrentError;
use rand::Rng;
use std::path::PathBuf;

const OUTPUT_ROOT: &str = "tests/tmp/";
const PIECE_LENGTH: Integer = 32 * 1024; // n * 1024 KiB

fn rand_file_name() -> String {
    OUTPUT_ROOT.to_owned() + &rand::thread_rng().gen::<u16>().to_string()
}

#[test]
fn build_single_file_ok() {
    let output_name = rand_file_name() + ".torrent";

    TorrentBuilder::new(
        PathBuf::from("tests/files/tails-amd64-3.6.1.torrent"),
        PIECE_LENGTH,
    )
    .set_announce(Some(
        "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
    ))
    .add_extra_field("creation date".to_owned(), BencodeElem::Integer(1523448537))
    .add_extra_field(
        "encoding".to_owned(),
        BencodeElem::String("UTF-8".to_owned()),
    )
    .add_extra_info_field("private".to_owned(), BencodeElem::Integer(0))
    .set_num_threads(1)
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
fn build_single_file_non_blocking_ok() {
    let output_name = rand_file_name() + ".torrent";

    let build = TorrentBuilder::new(
        PathBuf::from("tests/files/tails-amd64-3.6.1.torrent"),
        PIECE_LENGTH,
    )
    .set_announce(Some(
        "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
    ))
    .add_extra_field("creation date".to_owned(), BencodeElem::Integer(1523448537))
    .add_extra_field(
        "encoding".to_owned(),
        BencodeElem::String("UTF-8".to_owned()),
    )
    .add_extra_info_field("private".to_owned(), BencodeElem::Integer(0))
    .set_num_threads(1)
    .build_non_blocking()
    .unwrap();

    let mut prev_progress = 0;
    while !build.is_finished() {
        let curr_progress = build.get_progress();
        assert!(prev_progress <= curr_progress);
        prev_progress = curr_progress;
    }
    assert_eq!(prev_progress, 100);

    build
        .get_output()
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
fn build_single_file_non_blocking_cancel() {
    let build = TorrentBuilder::new(
        PathBuf::from("tests/files/tails-amd64-3.6.1.torrent"),
        PIECE_LENGTH,
    )
    .set_num_threads(1)
    .build_non_blocking()
    .unwrap();

    build.cancel();

    match build.get_output() {
        Err(LavaTorrentError::TorrentBuilderFailure(m)) => {
            assert_eq!(m, "build canceled by client")
        }
        _ => panic!(),
    }
}

#[test]
fn build_single_file_parallel_ok() {
    let output_name = rand_file_name() + ".torrent";

    TorrentBuilder::new(
        PathBuf::from("tests/files/tails-amd64-3.6.1.torrent"),
        PIECE_LENGTH,
    )
    .set_announce(Some(
        "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
    ))
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
fn build_single_file_parallel_non_blocking_ok() {
    let output_name = rand_file_name() + ".torrent";

    let build = TorrentBuilder::new(
        PathBuf::from("tests/files/tails-amd64-3.6.1.torrent"),
        PIECE_LENGTH,
    )
    .set_announce(Some(
        "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
    ))
    .add_extra_field("creation date".to_owned(), BencodeElem::Integer(1523448537))
    .add_extra_field(
        "encoding".to_owned(),
        BencodeElem::String("UTF-8".to_owned()),
    )
    .add_extra_info_field("private".to_owned(), BencodeElem::Integer(0))
    .build_non_blocking()
    .unwrap();

    let mut prev_progress = 0;
    while !build.is_finished() {
        let curr_progress = build.get_progress();
        assert!(prev_progress <= curr_progress);
        prev_progress = curr_progress;
    }
    assert_eq!(prev_progress, 100);

    build
        .get_output()
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
fn build_single_file_parallel_non_blocking_cancel() {
    let build = TorrentBuilder::new(
        PathBuf::from("tests/files/tails-amd64-3.6.1.torrent"),
        PIECE_LENGTH,
    )
    .build_non_blocking()
    .unwrap();

    build.cancel();

    match build.get_output() {
        Err(LavaTorrentError::TorrentBuilderFailure(m)) => {
            assert_eq!(m, "build canceled by client")
        }
        _ => panic!(),
    }
}

#[test]
fn build_multi_file_ok() {
    let output_name = rand_file_name() + ".torrent";

    TorrentBuilder::new(PathBuf::from("tests/files"), PIECE_LENGTH)
        .set_announce(Some(
            "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
        ))
        .add_extra_field("creation date".to_owned(), BencodeElem::Integer(1523607302))
        .add_extra_field(
            "encoding".to_owned(),
            BencodeElem::String("UTF-8".to_owned()),
        )
        .add_extra_info_field("private".to_owned(), BencodeElem::Integer(0))
        .set_num_threads(1)
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
fn build_multi_file_non_blocking_ok() {
    let output_name = rand_file_name() + ".torrent";

    let build = TorrentBuilder::new(PathBuf::from("tests/files"), PIECE_LENGTH)
        .set_announce(Some(
            "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
        ))
        .add_extra_field("creation date".to_owned(), BencodeElem::Integer(1523607302))
        .add_extra_field(
            "encoding".to_owned(),
            BencodeElem::String("UTF-8".to_owned()),
        )
        .add_extra_info_field("private".to_owned(), BencodeElem::Integer(0))
        .set_num_threads(1)
        .build_non_blocking()
        .unwrap();

    let mut prev_progress = 0;
    while !build.is_finished() {
        let curr_progress = build.get_progress();
        assert!(prev_progress <= curr_progress);
        prev_progress = curr_progress;
    }
    assert_eq!(prev_progress, 100);

    build
        .get_output()
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
fn build_multi_file_non_blocking_cancel() {
    let build = TorrentBuilder::new(PathBuf::from("tests/files"), PIECE_LENGTH)
        .set_num_threads(1)
        .build_non_blocking()
        .unwrap();

    build.cancel();

    match build.get_output() {
        Err(LavaTorrentError::TorrentBuilderFailure(m)) => {
            assert_eq!(m, "build canceled by client")
        }
        _ => panic!(),
    }
}

#[test]
fn build_multi_file_parallel_ok() {
    let output_name = rand_file_name() + ".torrent";

    TorrentBuilder::new(PathBuf::from("tests/files"), PIECE_LENGTH)
        .set_announce(Some(
            "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
        ))
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
fn build_multi_file_parallel_non_blocking_ok() {
    let output_name = rand_file_name() + ".torrent";

    let build = TorrentBuilder::new(PathBuf::from("tests/files"), PIECE_LENGTH)
        .set_announce(Some(
            "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
        ))
        .add_extra_field("creation date".to_owned(), BencodeElem::Integer(1523607302))
        .add_extra_field(
            "encoding".to_owned(),
            BencodeElem::String("UTF-8".to_owned()),
        )
        .add_extra_info_field("private".to_owned(), BencodeElem::Integer(0))
        .build_non_blocking()
        .unwrap();

    let mut prev_progress = 0;
    while !build.is_finished() {
        let curr_progress = build.get_progress();
        assert!(prev_progress <= curr_progress);
        prev_progress = curr_progress;
    }
    assert_eq!(prev_progress, 100);

    build
        .get_output()
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
fn build_multi_file_parallel_non_blocking_cancel() {
    let build = TorrentBuilder::new(PathBuf::from("tests/files"), PIECE_LENGTH)
        .build_non_blocking()
        .unwrap();

    build.cancel();

    match build.get_output() {
        Err(LavaTorrentError::TorrentBuilderFailure(m)) => {
            assert_eq!(m, "build canceled by client")
        }
        _ => panic!(),
    }
}

#[test]
fn build_with_name() {
    let output_name = rand_file_name() + ".torrent";

    TorrentBuilder::new(
        PathBuf::from("tests/files/tails-amd64-3.6.1.torrent"),
        PIECE_LENGTH,
    )
    .set_announce(Some(
        "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
    ))
    .set_name("file".to_owned())
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

    TorrentBuilder::new(PathBuf::from("tests/files"), PIECE_LENGTH)
        .set_announce(Some(
            "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
        ))
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

    TorrentBuilder::new(PathBuf::from("tests/files/symlink"), PIECE_LENGTH)
        .set_announce(Some(
            "udp://tracker.coppersurfer.tk:6969/announce".to_owned(),
        ))
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
fn build_nested_dir_ok() {
    let output_name = rand_file_name() + ".torrent";

    TorrentBuilder::new(PathBuf::from("tests/nested"), PIECE_LENGTH)
        .add_extra_field("creation date".to_owned(), BencodeElem::Integer(1678689103))
        .build()
        .unwrap()
        .write_into_file(&output_name)
        .unwrap();

    // compare against a sample file created by qBittorrent
    assert_eq!(
        Torrent::read_from_file(output_name).unwrap(),
        Torrent::read_from_file("tests/samples/nested.torrent").unwrap(),
    );
}
