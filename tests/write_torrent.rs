extern crate lava_torrent;
extern crate rand;

use std::fs::File;
use std::io::{BufReader, Read};
use rand::Rng;
use lava_torrent::torrent::v1::Torrent;

const OUTPUT_ROOT: &str = "tests/tmp/";

fn rand_file_name() -> String {
    OUTPUT_ROOT.to_string() + &rand::thread_rng().gen::<u16>().to_string()
}

#[test]
fn encode_torrent_ok() {
    let file = File::open("tests/files/ubuntu-16.04.4-desktop-amd64.iso.torrent").unwrap();
    let mut bytes = Vec::new();
    BufReader::new(file).read_to_end(&mut bytes).unwrap();

    match Torrent::read_from_bytes(&bytes) {
        Ok(parsed) => match parsed.encode() {
            Ok(encoded) => assert_eq!(encoded, bytes),
            Err(_) => assert!(false),
        },
        Err(_) => assert!(false),
    }
}

#[test]
fn write_torrent_to_file_ok() {
    let file = File::open("tests/files/ubuntu-16.04.4-desktop-amd64.iso.torrent").unwrap();
    let mut bytes = Vec::new();
    BufReader::new(file).read_to_end(&mut bytes).unwrap();

    let output = rand_file_name();
    match Torrent::read_from_bytes(&bytes) {
        Ok(original) => match original.clone().write_into_file(&output) {
            Ok(_) => match Torrent::read_from_file(&output) {
                Ok(duplicate) => assert_eq!(original, duplicate),
                Err(_) => assert!(false),
            },
            Err(_) => assert!(false),
        },
        Err(_) => assert!(false),
    }
}

#[test]
fn encode_torrent_multiple_files() {
    let file = File::open("tests/files/tails-amd64-3.6.1.torrent").unwrap();
    let mut bytes = Vec::new();
    BufReader::new(file).read_to_end(&mut bytes).unwrap();

    match Torrent::read_from_bytes(&bytes) {
        Ok(parsed) => match parsed.encode() {
            Ok(encoded) => assert_eq!(encoded, bytes),
            Err(_) => assert!(false),
        },
        Err(_) => assert!(false),
    }
}

#[test]
fn write_torrent_to_file_multiple_files() {
    let file = File::open("tests/files/tails-amd64-3.6.1.torrent").unwrap();
    let mut bytes = Vec::new();
    BufReader::new(file).read_to_end(&mut bytes).unwrap();

    let output = rand_file_name();
    match Torrent::read_from_bytes(&bytes) {
        Ok(original) => match original.clone().write_into_file(&output) {
            Ok(_) => match Torrent::read_from_file(&output) {
                Ok(duplicate) => assert_eq!(original, duplicate),
                Err(_) => assert!(false),
            },
            Err(_) => assert!(false),
        },
        Err(_) => assert!(false),
    }
}
