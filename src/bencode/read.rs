use super::*;
use error::*;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::iter::FromIterator;
use std::path::Path;
use unicode_normalization::UnicodeNormalization;
use util;
use util::ByteBuffer;

impl BencodeElem {
    /// Parse `bytes` and return all `BencodeElem` found.
    ///
    /// If `bytes` is empty, then `Ok(vec)` will be returned, but
    /// `vec` would be empty as well.
    ///
    /// If `bytes` contains any malformed bencode, or if any other
    /// error is encountered (e.g. `IOError`), then `Err(error)`
    /// will be returned.
    pub fn from_bytes<B>(bytes: B) -> Result<Vec<BencodeElem>>
    where
        B: AsRef<[u8]>,
    {
        let mut bytes = ByteBuffer::new(bytes.as_ref());
        let mut elements = Vec::new();

        while !bytes.is_empty() {
            let element = BencodeElem::parse(&mut bytes)?;
            elements.push(element);
        }

        Ok(elements)
    }

    /// Parse the content of the file at `path` and return all `BencodeElem` found.
    ///
    /// If the file at `path` is empty, then `Ok(vec)` will be returned, but
    /// `vec` would be empty as well.
    ///
    /// If the file at `path` contains any malformed bencode, or if any other
    /// error is encountered (e.g. `IOError`), then `Err(error)`
    /// will be returned.
    pub fn from_file<P>(path: P) -> Result<Vec<BencodeElem>>
    where
        P: AsRef<Path>,
    {
        let file = File::open(&path)?;
        let mut bytes = Vec::new();

        BufReader::new(file).read_to_end(&mut bytes)?;
        Self::from_bytes(bytes)
    }

    fn peek_byte(bytes: &mut ByteBuffer) -> Result<u8> {
        match bytes.peek() {
            Some(&byte) => Ok(byte),
            None => bail!(ErrorKind::MalformedBencode(Cow::Borrowed(
                "Expected more bytes, but none found."
            ))),
        }
    }

    fn parse(bytes: &mut ByteBuffer) -> Result<BencodeElem> {
        match Self::peek_byte(bytes)? {
            DICTIONARY_PREFIX => {
                bytes.advance(1);
                Ok(Self::decode_dictionary(bytes)?)
            }
            LIST_PREFIX => {
                bytes.advance(1);
                Ok(Self::decode_list(bytes)?)
            }
            INTEGER_PREFIX => {
                bytes.advance(1);
                Ok(Self::decode_integer(bytes, INTEGER_POSTFIX)?)
            }
            _ => Ok(Self::decode_string(bytes)?),
        }
    }

    fn decode_dictionary(bytes: &mut ByteBuffer) -> Result<BencodeElem> {
        let mut entries = Vec::new();

        while Self::peek_byte(bytes)? != DICTIONARY_POSTFIX {
            // more to parse
            match Self::decode_string(bytes) {
                Ok(BencodeElem::Bytes(string)) => entries.push((string, Self::parse(bytes)?)),
                Ok(BencodeElem::String(string)) => {
                    entries.push((string.into_bytes(), Self::parse(bytes)?))
                }
                Ok(_) => bail!(ErrorKind::MalformedBencode(Cow::Borrowed(
                    "Non-string dictionary key."
                ))),
                Err(e) => bail!(e),
            }
        }
        bytes.advance(1); // consume the postfix

        // check that the dictionary is sorted
        for (i, j) in (1..entries.len()).enumerate() {
            let (&(ref k1, _), &(ref k2, _)) = (&entries[i], &entries[j]);
            // "sorted as raw strings, not alphanumerics"
            if k1 > k2 {
                bail!(ErrorKind::MalformedBencode(Cow::Borrowed(
                    "A dictionary is not properly sorted."
                )));
            }
        }

        // convert to Dictionary if possible
        let mut entries2 = Vec::new();
        for (k, v) in &entries {
            match String::from_utf8(k.to_owned()) {
                Ok(s) => entries2.push((s, v.to_owned())),
                Err(_) => {
                    return Ok(BencodeElem::RawDictionary(HashMap::from_iter(
                        entries.into_iter(),
                    )));
                }
            }
        }
        Ok(BencodeElem::Dictionary(HashMap::from_iter(
            entries2.into_iter(),
        )))
    }

    fn decode_list(bytes: &mut ByteBuffer) -> Result<BencodeElem> {
        let mut list = Vec::new();

        while Self::peek_byte(bytes)? != LIST_POSTFIX {
            // more to parse
            list.push(Self::parse(bytes)?);
        }
        bytes.advance(1); //consume the postfix

        Ok(BencodeElem::List(list))
    }

    fn decode_integer(bytes: &mut ByteBuffer, delimiter: u8) -> Result<BencodeElem> {
        let old_pos = bytes.pos();
        let read: Vec<u8> = bytes.take_while(|&&b| b != delimiter).cloned().collect();
        let bytes_read = bytes.pos() - old_pos;

        if read.len() == bytes_read {
            bail!(ErrorKind::MalformedBencode(Cow::Borrowed(
                "Integer delimiter not found."
            )));
        } else {
            match String::from_utf8(read) {
                Ok(int_string) => {
                    if int_string.starts_with("-0") {
                        bail!(ErrorKind::MalformedBencode(Cow::Borrowed("-0 found.")));
                    } else if (int_string.starts_with('0')) && (int_string.len() != 1) {
                        bail!(ErrorKind::MalformedBencode(Cow::Borrowed(
                            "Integer with leading zero(s) found."
                        )));
                    } else {
                        match int_string.parse() {
                            Ok(int) => Ok(BencodeElem::Integer(int)),
                            Err(_) => bail!(ErrorKind::MalformedBencode(Cow::Owned(format!(
                                "Input contains invalid integer: {}.",
                                int_string
                            )))),
                        }
                    }
                }
                Err(_) => bail!(ErrorKind::MalformedBencode(Cow::Borrowed(
                    "Input contains invalid UTF-8."
                ))),
            }
        }
    }

    fn decode_string(bytes: &mut ByteBuffer) -> Result<BencodeElem> {
        match Self::decode_integer(bytes, STRING_DELIMITER) {
            Ok(BencodeElem::Integer(len)) => {
                if let Ok(len) = util::i64_to_usize(len) {
                    let string_bytes = bytes.take(len).cloned().collect();

                    // Since the SHA1 hash values are not valid UTF8,
                    // we can't really say that an invalid UTF8 string
                    // indicates malformed bencode. In that case, we
                    // can only return the bytes as-is, and the client
                    // has to decide if the bencode is indeed malformed.
                    //
                    // Valid UTF8 strings are normalizd to NFC forms.
                    match String::from_utf8(string_bytes) {
                        Ok(string) => Ok(BencodeElem::String(string.chars().nfc().collect())),
                        Err(e) => Ok(BencodeElem::Bytes(e.into_bytes())),
                    }
                } else {
                    bail!(ErrorKind::MalformedBencode(Cow::Borrowed(
                        "A string's length does not fit into `usize`."
                    )));
                }
            }
            Ok(_) => panic!("decode_integer() did not return an integer."),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod bencode_elem_read_tests {
    // @note: `from_bytes()` and `from_file()` are not tested
    // as they are best left to integration tests (in `tests/`,
    // implicitly tested with `Torrent::read_from_bytes()`
    // and `Torrent::read_from_file()`).
    use super::*;

    #[test]
    fn peek_byte_ok() {
        let bytes = "a".as_bytes();
        assert_eq!(
            BencodeElem::peek_byte(&mut ByteBuffer::new(bytes)).unwrap(),
            b'a'
        );
    }

    #[test]
    fn peek_byte_err() {
        let bytes = "".as_bytes();
        match BencodeElem::peek_byte(&mut ByteBuffer::new(bytes)) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "Expected more bytes, but none found.");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_integer_ok() {
        let bytes = "0e".as_bytes();
        assert_eq!(
            BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX).unwrap(),
            bencode_elem!(0_i64)
        );
    }

    #[test]
    fn decode_integer_ok_2() {
        let bytes = "-4e".as_bytes();
        assert_eq!(
            BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX).unwrap(),
            bencode_elem!(-4_i64)
        );
    }

    #[test]
    fn decode_integer_invalid_int() {
        let bytes = "4ae".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "Input contains invalid integer: 4a.");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_integer_invalid_int_2() {
        let bytes = "--1e".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "Input contains invalid integer: --1.");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_integer_invalid_int_3() {
        let bytes = "03e".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "Integer with leading zero(s) found.");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_integer_invalid_int_4() {
        let bytes = "-0e".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => assert_eq!(m, "-0 found."),
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_integer_invalid_int_5() {
        let bytes = "-01e".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => assert_eq!(m, "-0 found."),
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_integer_overflow() {
        let bytes = "9223372036854775808e".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "Input contains invalid integer: 9223372036854775808.");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_integer_no_delimiter() {
        let bytes = "9223372036854775807".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "Integer delimiter not found.");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_integer_bad_utf8() {
        let bytes = vec![b'4', 0xff, 0xf8, INTEGER_POSTFIX];
        match BencodeElem::decode_integer(&mut ByteBuffer::new(&bytes), INTEGER_POSTFIX) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "Input contains invalid UTF-8.");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_string_ok() {
        let bytes = "4:spam".as_bytes();
        assert_eq!(
            BencodeElem::decode_string(&mut ByteBuffer::new(bytes)).unwrap(),
            bencode_elem!("spam")
        );
    }

    #[test]
    fn decode_string_invalid_len() {
        let bytes = "a:spam".as_bytes();
        match BencodeElem::decode_string(&mut ByteBuffer::new(bytes)) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "Input contains invalid integer: a.");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_string_no_len() {
        let bytes = ":spam".as_bytes();
        match BencodeElem::decode_string(&mut ByteBuffer::new(bytes)) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "Input contains invalid integer: .");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_string_negative_len() {
        let bytes = "-1:spam".as_bytes();
        match BencodeElem::decode_string(&mut ByteBuffer::new(bytes)) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "A string's length does not fit into `usize`.");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_string_no_delimiter() {
        let bytes = "4spam".as_bytes();
        match BencodeElem::decode_string(&mut ByteBuffer::new(bytes)) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "Integer delimiter not found.");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_string_no_delimiter_2() {
        let bytes = "456".as_bytes();
        match BencodeElem::decode_string(&mut ByteBuffer::new(bytes)) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "Integer delimiter not found.");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_string_as_bytes() {
        let bytes = vec![b'4', b':', 0xff, 0xf8, 0xff, 0xee]; // bad UTF8 gives bytes
        assert_eq!(
            BencodeElem::decode_string(&mut ByteBuffer::new(&bytes)).unwrap(),
            bencode_elem!((0xff, 0xf8, 0xff, 0xee))
        );
    }

    #[test]
    fn decode_list_ok() {
        let bytes = "4:spam4:eggse".as_bytes();
        assert_eq!(
            BencodeElem::decode_list(&mut ByteBuffer::new(bytes)).unwrap(),
            bencode_elem!(["spam", "eggs"])
        );
    }

    #[test]
    fn decode_list_nested() {
        let bytes = "4:spaml6:cheesee4:eggse".as_bytes();
        assert_eq!(
            BencodeElem::decode_list(&mut ByteBuffer::new(bytes)).unwrap(),
            bencode_elem!(["spam", ["cheese"], "eggs"])
        );
    }

    #[test]
    fn decode_list_empty() {
        let bytes = "e".as_bytes();
        assert_eq!(
            BencodeElem::decode_list(&mut ByteBuffer::new(bytes)).unwrap(),
            bencode_elem!([])
        );
    }

    #[test]
    fn decode_list_bad_structure() {
        let bytes = "4:spaml6:cheese4:eggse".as_bytes();
        match BencodeElem::decode_list(&mut ByteBuffer::new(bytes)) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "Expected more bytes, but none found.");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_dictionary_ok() {
        let bytes = "3:cow3:moo4:spam4:eggse".as_bytes();
        assert_eq!(
            BencodeElem::decode_dictionary(&mut ByteBuffer::new(bytes)).unwrap(),
            bencode_elem!({ ("cow", "moo"), ("spam", "eggs") })
        );
    }

    #[test]
    fn decode_dictionary_nested() {
        let bytes = "3:cowd3:mooi4ee4:spam4:eggse".as_bytes();
        assert_eq!(
            BencodeElem::decode_dictionary(&mut ByteBuffer::new(bytes)).unwrap(),
            bencode_elem!({ ("cow", { ("moo", 4_i64) }), ("spam", "eggs") })
        );
    }

    #[test]
    fn decode_dictionary_empty() {
        let bytes = "e".as_bytes();
        assert_eq!(
            BencodeElem::decode_dictionary(&mut ByteBuffer::new(bytes)).unwrap(),
            bencode_elem!({})
        );
    }

    #[test]
    fn decode_dictionary_bad_structure() {
        let bytes = "3:cow3:moo4:spame".as_bytes();
        match BencodeElem::decode_dictionary(&mut ByteBuffer::new(bytes)) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "Integer delimiter not found.");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_dictionary_non_string_key_1() {
        let bytes = "i4e3:moo4:spam4:eggse".as_bytes();
        match BencodeElem::decode_dictionary(&mut ByteBuffer::new(bytes)) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "Input contains invalid integer: i4e3.");
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn decode_dictionary_not_sorted() {
        let bytes = "3:zoo3:moo4:spam4:eggse".as_bytes();
        match BencodeElem::decode_dictionary(&mut ByteBuffer::new(bytes)) {
            Err(Error(ErrorKind::MalformedBencode(m), _)) => {
                assert_eq!(m, "A dictionary is not properly sorted.");
            }
            _ => assert!(false),
        }
    }

    // @note: `parse()` is called by other `decode_*()` methods, so
    // it is implicitly tested by other tests. Still, the following tests
    // are provided. Though these tests are not as comprehensive.
    #[test]
    fn parse_integer_ok() {
        let bytes = "i0e".as_bytes();
        assert_eq!(
            BencodeElem::parse(&mut ByteBuffer::new(bytes)).unwrap(),
            bencode_elem!(0_i64)
        );
    }

    #[test]
    fn parse_string_ok() {
        let bytes = "4:spam".as_bytes();
        assert_eq!(
            BencodeElem::parse(&mut ByteBuffer::new(bytes)).unwrap(),
            bencode_elem!("spam")
        );
    }

    #[test]
    fn parse_bytes_ok() {
        let bytes = vec![b'4', b':', 0xff, 0xf8, 0xff, 0xee]; // bad UTF8 gives bytes
        assert_eq!(
            BencodeElem::parse(&mut ByteBuffer::new(&bytes)).unwrap(),
            bencode_elem!((0xff, 0xf8, 0xff, 0xee))
        );
    }

    #[test]
    fn parse_list_ok() {
        let bytes = "l4:spam4:eggse".as_bytes();
        assert_eq!(
            BencodeElem::parse(&mut ByteBuffer::new(bytes)).unwrap(),
            bencode_elem!(["spam", "eggs"])
        );
    }

    #[test]
    fn parse_dictionary_ok() {
        let bytes = "d3:cow3:moo4:spam4:eggse".as_bytes();
        assert_eq!(
            BencodeElem::parse(&mut ByteBuffer::new(bytes)).unwrap(),
            bencode_elem!({ ("cow", "moo"), ("spam", "eggs") })
        );
    }
}
