//! Module for bencode-related encoding.

use super::*;
use error::*;
use std::fs::File;
use std::hash::BuildHasher;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Encode `string` and write the result to `dst`.
pub fn write_string<W>(string: &str, dst: &mut W) -> Result<()>
where
    W: Write,
{
    dst.write_all(&string.len().to_string().into_bytes())?;
    dst.write_all(&[STRING_DELIMITER])?;
    dst.write_all(string.as_bytes())?;
    Ok(())
}

/// Encode `bytes` and write the result to `dst`.
pub fn write_bytes<W>(bytes: &[u8], dst: &mut W) -> Result<()>
where
    W: Write,
{
    dst.write_all(&bytes.len().to_string().into_bytes())?;
    dst.write_all(&[STRING_DELIMITER])?;
    dst.write_all(bytes)?;
    Ok(())
}

/// Encode `int` and write the result to `dst`.
pub fn write_integer<W>(int: i64, dst: &mut W) -> Result<()>
where
    W: Write,
{
    dst.write_all(&[INTEGER_PREFIX])?;
    dst.write_all(int.to_string().as_bytes())?;
    dst.write_all(&[INTEGER_POSTFIX])?;
    Ok(())
}

/// Encode `list` and write the result to `dst`.
pub fn write_list<W>(list: &[BencodeElem], dst: &mut W) -> Result<()>
where
    W: Write,
{
    dst.write_all(&[LIST_PREFIX])?;
    for item in list {
        item.write_into(dst)?;
    }
    dst.write_all(&[LIST_POSTFIX])?;
    Ok(())
}

/// Encode `dict` and write the result to `dst`.
pub fn write_dictionary<W, S>(dict: &HashMap<String, BencodeElem, S>, dst: &mut W) -> Result<()>
where
    W: Write,
    S: BuildHasher,
{
    // "Keys must be strings and appear in sorted order
    // (sorted as raw strings, not alphanumerics)."
    let mut sorted = dict.iter().collect::<Vec<(&String, &BencodeElem)>>();
    sorted.sort_by_key(|&(key, _)| key.as_bytes());

    dst.write_all(&[DICTIONARY_PREFIX])?;
    for (key, val) in sorted {
        write_string(key, dst)?;
        val.write_into(dst)?;
    }
    dst.write_all(&[DICTIONARY_POSTFIX])?;
    Ok(())
}

/// Encode `string` and return the result in a `Vec`.
pub fn encode_string(string: &str) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(string.len() + 2);
    write_string(string, &mut encoded).expect("Write to vec failed!");
    encoded
}

/// Encode `bytes` and return the result in a `Vec`.
pub fn encode_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(bytes.len() + 2);
    write_bytes(bytes, &mut encoded).expect("Write to vec failed!");
    encoded
}

/// Encode `int` and return the result in a `Vec`.
pub fn encode_integer(int: i64) -> Vec<u8> {
    let mut encoded = Vec::new();
    write_integer(int, &mut encoded).expect("Write to vec failed!");
    encoded
}

/// Encode `list` and return the result in a `Vec`.
pub fn encode_list(list: &[BencodeElem]) -> Vec<u8> {
    let mut encoded = Vec::new();
    write_list(list, &mut encoded).expect("Write to vec failed!");
    encoded
}

/// Encode `dict` and return the result in a `Vec`.
pub fn encode_dictionary<S>(dict: &HashMap<String, BencodeElem, S>) -> Vec<u8>
where
    S: BuildHasher,
{
    let mut encoded = Vec::new();
    write_dictionary(dict, &mut encoded).expect("Write to vec failed!");
    encoded
}

impl BencodeElem {
    /// Encode `self` and write the result to `dst`.
    pub fn write_into<W>(&self, dst: &mut W) -> Result<()>
    where
        W: Write,
    {
        match *self {
            BencodeElem::String(ref string) => write_string(string, dst),
            BencodeElem::Bytes(ref bytes) => write_bytes(bytes, dst),
            BencodeElem::Integer(int) => write_integer(int, dst),
            BencodeElem::List(ref list) => write_list(list, dst),
            BencodeElem::Dictionary(ref dict) => write_dictionary(dict, dst),
        }
    }

    /// Encode `self` and write the result to `path`.
    ///
    /// `path` must be the path to a file.
    ///
    /// "This function will create a file if it does
    /// not exist, and will truncate it if it does."
    pub fn write_into_file<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let file = File::create(&path)?;
        self.write_into(&mut BufWriter::new(&file))?;
        file.sync_all()?;
        Ok(())
    }

    /// Encode `self` and return the result in a `Vec`.
    pub fn encode(&self) -> Vec<u8> {
        match *self {
            BencodeElem::String(ref string) => encode_string(string),
            BencodeElem::Bytes(ref bytes) => encode_bytes(bytes),
            BencodeElem::Integer(int) => encode_integer(int),
            BencodeElem::List(ref list) => encode_list(list),
            BencodeElem::Dictionary(ref dict) => encode_dictionary(dict),
        }
    }
}

#[cfg(test)]
mod bencode_elem_write_tests {
    // @note: `write_into_file()` is not tested as it is best
    // left to integration tests (in `tests/`).
    use super::*;
    use std::collections::hash_map::RandomState;
    use std::iter::FromIterator;

    #[test]
    fn write_string_ok() {
        let mut vec = Vec::new();
        write_string(&"spam".to_owned(), &mut vec).unwrap();
        assert_eq!(vec, "4:spam".as_bytes().to_vec());
    }

    #[test]
    fn write_bytes_ok() {
        let mut vec = Vec::new();
        write_bytes(&[0x01, 0x02, 0x03, 0x04], &mut vec).unwrap();
        assert_eq!(vec, vec![b'4', b':', 0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn write_integer_ok() {
        let mut vec = Vec::new();
        write_integer(42, &mut vec).unwrap();
        assert_eq!(vec, vec![b'i', b'4', b'2', b'e']);
    }

    #[test]
    fn write_list_ok() {
        let mut vec = Vec::new();
        write_list(&vec![bencode_elem!(42), bencode_elem!("spam")], &mut vec).unwrap();
        assert_eq!(
            vec,
            vec![
                b'l', b'i', b'4', b'2', b'e', b'4', b':', b's', b'p', b'a', b'm', b'e',
            ]
        );
    }

    #[test]
    fn write_dictionary_ok() {
        let mut vec = Vec::new();
        write_dictionary::<_, RandomState>(
            &HashMap::from_iter(
                vec![
                    ("spam".to_owned(), bencode_elem!(42)),
                    ("cow".to_owned(), bencode_elem!("moo")),
                ].into_iter(),
            ),
            &mut vec,
        ).unwrap();
        assert_eq!(
            vec,
            vec![
                b'd', b'3', b':', b'c', b'o', b'w', b'3', b':', b'm', b'o', b'o', b'4', b':', b's',
                b'p', b'a', b'm', b'i', b'4', b'2', b'e', b'e',
            ]
        );
    }

    #[test]
    fn encode_string_ok() {
        assert_eq!(
            encode_string(&"spam".to_owned()),
            "4:spam".as_bytes().to_vec(),
        )
    }

    #[test]
    fn encode_bytes_ok() {
        assert_eq!(
            encode_bytes(&[0x01, 0x02, 0x03, 0x04]),
            vec![b'4', b':', 0x01, 0x02, 0x03, 0x04],
        )
    }

    #[test]
    fn encode_integer_ok() {
        assert_eq!(encode_integer(42), vec![b'i', b'4', b'2', b'e'])
    }

    #[test]
    fn encode_list_ok() {
        assert_eq!(
            encode_list(&vec![bencode_elem!(42), bencode_elem!("spam")]),
            vec![
                b'l', b'i', b'4', b'2', b'e', b'4', b':', b's', b'p', b'a', b'm', b'e',
            ],
        )
    }

    #[test]
    fn encode_dictionary_ok() {
        assert_eq!(
            encode_dictionary::<RandomState>(&HashMap::from_iter(
                vec![
                    ("spam".to_owned(), bencode_elem!(42)),
                    ("cow".to_owned(), bencode_elem!("moo")),
                ].into_iter()
            )),
            vec![
                b'd', b'3', b':', b'c', b'o', b'w', b'3', b':', b'm', b'o', b'o', b'4', b':', b's',
                b'p', b'a', b'm', b'i', b'4', b'2', b'e', b'e',
            ],
        )
    }

    #[test]
    fn bencode_elem_write_string_ok() {
        let mut vec = Vec::new();
        bencode_elem!("spam").write_into(&mut vec).unwrap();
        assert_eq!(vec, "4:spam".as_bytes().to_vec());
    }

    #[test]
    fn bencode_elem_write_bytes_ok() {
        let mut vec = Vec::new();
        bencode_elem!((0x01, 0x02, 0x03, 0x04))
            .write_into(&mut vec)
            .unwrap();
        assert_eq!(vec, vec![b'4', b':', 0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn bencode_elem_write_integer_ok() {
        let mut vec = Vec::new();
        bencode_elem!(42).write_into(&mut vec).unwrap();
        assert_eq!(vec, vec![b'i', b'4', b'2', b'e']);
    }

    #[test]
    fn bencode_elem_write_list_ok() {
        let mut vec = Vec::new();
        bencode_elem!([42, "spam"]).write_into(&mut vec).unwrap();
        assert_eq!(
            vec,
            vec![
                b'l', b'i', b'4', b'2', b'e', b'4', b':', b's', b'p', b'a', b'm', b'e',
            ]
        );
    }

    #[test]
    fn bencode_elem_write_dictionary_ok() {
        let mut vec = Vec::new();
        bencode_elem!({ ("spam", 42), ("cow", "moo") })
            .write_into(&mut vec)
            .unwrap();
        assert_eq!(
            vec,
            vec![
                b'd', b'3', b':', b'c', b'o', b'w', b'3', b':', b'm', b'o', b'o', b'4', b':', b's',
                b'p', b'a', b'm', b'i', b'4', b'2', b'e', b'e',
            ]
        );
    }

    #[test]
    fn bencode_elem_encode_string_ok() {
        assert_eq!(bencode_elem!("spam").encode(), "4:spam".as_bytes().to_vec(),)
    }

    #[test]
    fn bencode_elem_encode_bytes_ok() {
        assert_eq!(
            bencode_elem!((0x01, 0x02, 0x03, 0x04)).encode(),
            vec![b'4', b':', 0x01, 0x02, 0x03, 0x04],
        )
    }

    #[test]
    fn bencode_elem_encode_integer_ok() {
        assert_eq!(bencode_elem!(42).encode(), vec![b'i', b'4', b'2', b'e'])
    }

    #[test]
    fn bencode_elem_encode_list_ok() {
        assert_eq!(
            bencode_elem!([42, "spam"]).encode(),
            vec![
                b'l', b'i', b'4', b'2', b'e', b'4', b':', b's', b'p', b'a', b'm', b'e',
            ],
        )
    }

    #[test]
    fn bencode_elem_encode_dictionary_ok() {
        assert_eq!(
            bencode_elem!({ ("spam", 42), ("cow", "moo") }).encode(),
            vec![
                b'd', b'3', b':', b'c', b'o', b'w', b'3', b':', b'm', b'o', b'o', b'4', b':', b's',
                b'p', b'a', b'm', b'i', b'4', b'2', b'e', b'e',
            ],
        )
    }
}
