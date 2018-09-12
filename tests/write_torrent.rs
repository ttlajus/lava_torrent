extern crate lava_torrent;
extern crate rand;

use lava_torrent::torrent::v1::Torrent;
use rand::Rng;
use std::fs::File;
use std::io::{BufReader, Read};

const OUTPUT_ROOT: &str = "tests/tmp/";

fn rand_file_name() -> String {
    OUTPUT_ROOT.to_owned() + &rand::thread_rng().gen::<u16>().to_string()
}

#[test]
fn encode_torrent_ok() {
    let file = File::open("tests/files/ubuntu-16.04.4-desktop-amd64.iso.torrent").unwrap();
    let mut bytes = Vec::new();
    BufReader::new(file).read_to_end(&mut bytes).unwrap();

    let parsed = Torrent::read_from_bytes(&bytes).unwrap();
    let encoded = parsed.encode().unwrap();
    assert_eq!(encoded, bytes);
}

#[test]
fn write_torrent_to_file_ok() {
    let file = File::open("tests/files/ubuntu-16.04.4-desktop-amd64.iso.torrent").unwrap();
    let mut bytes = Vec::new();
    BufReader::new(file).read_to_end(&mut bytes).unwrap();

    let output = rand_file_name();
    let original = Torrent::read_from_bytes(&bytes).unwrap();
    original.clone().write_into_file(&output).unwrap();
    let duplicate = Torrent::read_from_file(&output).unwrap();
    assert_eq!(original, duplicate);
}

#[test]
fn encode_torrent_multiple_files() {
    let file = File::open("tests/files/tails-amd64-3.6.1.torrent").unwrap();
    let mut bytes = Vec::new();
    BufReader::new(file).read_to_end(&mut bytes).unwrap();

    let parsed = Torrent::read_from_bytes(&bytes).unwrap();
    let encoded = parsed.encode().unwrap();
    assert_eq!(encoded, bytes);
}

#[test]
fn write_torrent_to_file_multiple_files() {
    let file = File::open("tests/files/tails-amd64-3.6.1.torrent").unwrap();
    let mut bytes = Vec::new();
    BufReader::new(file).read_to_end(&mut bytes).unwrap();

    let output = rand_file_name();
    let original = Torrent::read_from_bytes(&bytes).unwrap();
    original.clone().write_into_file(&output).unwrap();
    let duplicate = Torrent::read_from_file(&output).unwrap();
    assert_eq!(original, duplicate);
}
