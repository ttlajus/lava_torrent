use std::path::Path;
use std::fs::File;
use std::io::{BufReader, Read};
use std::collections::HashMap;
use std::iter::FromIterator;
use std::borrow::Cow;
use conv::ValueFrom;
use unicode_normalization::UnicodeNormalization;
use {Error, ErrorKind, Result};
use super::*;

struct ByteBuffer<'a> {
    bytes: &'a [u8],
    position: usize, // current cursor position
    length: usize,   // total buffer length
}

impl<'a> ByteBuffer<'a> {
    fn new(bytes: &[u8]) -> ByteBuffer {
        ByteBuffer {
            bytes,
            position: 0,
            length: bytes.len(),
        }
    }

    fn peek(&self) -> Option<&'a u8> {
        if self.is_empty() {
            None
        } else {
            Some(&self.bytes[self.position])
        }
    }

    fn advance(&mut self, step: usize) {
        self.position += step;
        if self.position > self.length {
            self.position = self.length;
        }
    }

    fn pos(&self) -> usize {
        self.position
    }

    fn is_empty(&self) -> bool {
        self.position >= self.length
    }
}

impl<'a> Iterator for ByteBuffer<'a> {
    type Item = &'a u8;

    fn next(&mut self) -> Option<&'a u8> {
        if self.is_empty() {
            None
        } else {
            self.position += 1;
            Some(&self.bytes[self.position - 1])
        }
    }
}

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
        if let Ok(file) = File::open(&path) {
            let mut bytes = Vec::new();

            match BufReader::new(file).read_to_end(&mut bytes) {
                Ok(_) => Self::from_bytes(bytes),
                Err(_) => Err(Error::new(
                    ErrorKind::IOError,
                    Cow::Owned(format!(
                        "IO error when reading [{}].",
                        path.as_ref().display()
                    )),
                )),
            }
        } else {
            Err(Error::new(
                ErrorKind::IOError,
                Cow::Owned(format!("Failed to open [{}].", path.as_ref().display())),
            ))
        }
    }

    fn peek_byte(bytes: &mut ByteBuffer) -> Result<u8> {
        match bytes.peek() {
            Some(&byte) => Ok(byte),
            None => Err(Error::new(
                ErrorKind::MalformedBencode,
                Cow::Borrowed("Malformed/incomplete input."),
            )), // expect more bytes, but none found
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
                Ok(BencodeElem::String(string)) => entries.push((string, Self::parse(bytes)?)),
                Ok(_) => {
                    return Err(Error::new(
                        ErrorKind::MalformedBencode,
                        Cow::Borrowed("Non-string dictionary key."),
                    ))
                }
                Err(e) => return Err(e),
            }
        }
        bytes.advance(1); // consume the postfix

        // check that the dictionary is sorted
        for (i, j) in (1..entries.len()).enumerate() {
            let (&(ref k1, _), &(ref k2, _)) = (&entries[i], &entries[j]);
            // "sorted as raw strings, not alphanumerics"
            if k1.as_bytes() > k2.as_bytes() {
                return Err(Error::new(
                    ErrorKind::MalformedBencode,
                    Cow::Borrowed("A dictionary is not properly sorted."),
                ));
            }
        }

        Ok(BencodeElem::Dictionary(HashMap::from_iter(
            entries.into_iter(),
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
            Err(Error::new(
                ErrorKind::MalformedBencode,
                Cow::Borrowed("Integer delimiter not found."),
            ))
        } else {
            match String::from_utf8(read) {
                Ok(int_string) => {
                    if int_string.starts_with("-0") {
                        Err(Error::new(
                            ErrorKind::MalformedBencode,
                            Cow::Borrowed("-0 found."),
                        ))
                    } else if (int_string.starts_with('0')) && (int_string.len() != 1) {
                        Err(Error::new(
                            ErrorKind::MalformedBencode,
                            Cow::Borrowed("Integer with leading zero(s) found."),
                        ))
                    } else {
                        match int_string.parse() {
                            Ok(int) => Ok(BencodeElem::Integer(int)),
                            Err(_) => Err(Error::new(
                                ErrorKind::MalformedBencode,
                                Cow::Owned(format!(
                                    "Input contains invalid integer: {}.",
                                    int_string
                                )),
                            )),
                        }
                    }
                }
                Err(_) => Err(Error::new(
                    ErrorKind::MalformedBencode,
                    Cow::Borrowed("Input contains invalid UTF-8."),
                )),
            }
        }
    }

    fn decode_string(bytes: &mut ByteBuffer) -> Result<BencodeElem> {
        match Self::decode_integer(bytes, STRING_DELIMITER) {
            Ok(BencodeElem::Integer(len)) => {
                // @todo: switch to `usize::try_from(len)` when it's stable
                if let Ok(len) = usize::value_from(len) {
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
                    Err(Error::new(
                        ErrorKind::MalformedBencode,
                        Cow::Borrowed("A string's length does not fit into `usize`."),
                    ))
                }
            }
            Ok(_) => panic!("decode_integer() did not return an integer."),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod byte_buffer_tests {
    use super::*;

    #[test]
    fn byte_buffer_sanity_test() {
        let bytes = vec![1, 2, 3];
        let mut buffer = ByteBuffer::new(&bytes);

        assert!(!buffer.is_empty());
        assert_eq!(buffer.peek(), Some(&1));
        assert_eq!(buffer.pos(), 0);
        buffer.advance(1);

        assert!(!buffer.is_empty());
        assert_eq!(buffer.peek(), Some(&2));
        assert_eq!(buffer.pos(), 1);
        buffer.advance(2);

        assert!(buffer.is_empty());
        assert_eq!(buffer.peek(), None);
        assert_eq!(buffer.pos(), 3);
        buffer.advance(1);

        assert!(buffer.is_empty());
        assert_eq!(buffer.peek(), None);
        assert_eq!(buffer.pos(), 3);
    }

    #[test]
    fn byte_buffer_iterator_test() {
        let bytes = vec![1, 2, 3];
        let mut buffer = ByteBuffer::new(&bytes);
        let mut output = Vec::new();

        for byte in &mut buffer {
            output.push(*byte);
        }

        assert!(buffer.is_empty());
        assert_eq!(buffer.peek(), None);
        assert_eq!(buffer.next(), None);
        assert_eq!(buffer.pos(), 3);
        assert_eq!(bytes, output);
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
        match BencodeElem::peek_byte(&mut ByteBuffer::new(bytes)) {
            Ok(c) => assert_eq!(c, b'a'),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn peek_byte_err() {
        let bytes = "".as_bytes();
        match BencodeElem::peek_byte(&mut ByteBuffer::new(bytes)) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_integer_ok() {
        let bytes = "0e".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Ok(i) => assert_eq!(i, bencode_elem!(0_i64)),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn decode_integer_ok_2() {
        let bytes = "-4e".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Ok(i) => assert_eq!(i, bencode_elem!(-4_i64)),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn decode_integer_invalid_int() {
        let bytes = "4ae".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_integer_invalid_int_2() {
        let bytes = "--1e".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_integer_invalid_int_3() {
        let bytes = "03e".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_integer_invalid_int_4() {
        let bytes = "-0e".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_integer_invalid_int_5() {
        let bytes = "-01e".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_integer_overflow() {
        let bytes = "9223372036854775808e".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_integer_no_delimiter() {
        let bytes = "9223372036854775807".as_bytes();
        match BencodeElem::decode_integer(&mut ByteBuffer::new(bytes), INTEGER_POSTFIX) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_integer_bad_utf8() {
        let bytes = vec![b'4', 0xff, 0xf8, INTEGER_POSTFIX];
        match BencodeElem::decode_integer(&mut ByteBuffer::new(&bytes), INTEGER_POSTFIX) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_string_ok() {
        let bytes = "4:spam".as_bytes();
        match BencodeElem::decode_string(&mut ByteBuffer::new(bytes)) {
            Ok(s) => assert_eq!(s, bencode_elem!("spam")),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn decode_string_invalid_len() {
        let bytes = "a:spam".as_bytes();
        match BencodeElem::decode_string(&mut ByteBuffer::new(bytes)) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_string_no_len() {
        let bytes = ":spam".as_bytes();
        match BencodeElem::decode_string(&mut ByteBuffer::new(bytes)) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_string_negative_len() {
        let bytes = "-1:spam".as_bytes();
        match BencodeElem::decode_string(&mut ByteBuffer::new(bytes)) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_string_no_delimiter() {
        let bytes = "4spam".as_bytes();
        match BencodeElem::decode_string(&mut ByteBuffer::new(bytes)) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_string_no_delimiter_2() {
        let bytes = "456".as_bytes();
        match BencodeElem::decode_string(&mut ByteBuffer::new(bytes)) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_string_as_bytes() {
        let bytes = vec![b'4', b':', 0xff, 0xf8, 0xff, 0xee]; // bad UTF8 gives bytes
        match BencodeElem::decode_string(&mut ByteBuffer::new(&bytes)) {
            Ok(bytes) => assert_eq!(bytes, bencode_elem!((0xff, 0xf8, 0xff, 0xee))),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn decode_list_ok() {
        let bytes = "4:spam4:eggse".as_bytes();
        match BencodeElem::decode_list(&mut ByteBuffer::new(bytes)) {
            Ok(l) => assert_eq!(l, bencode_elem!(["spam", "eggs"])),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn decode_list_nested() {
        let bytes = "4:spaml6:cheesee4:eggse".as_bytes();
        match BencodeElem::decode_list(&mut ByteBuffer::new(bytes)) {
            Ok(l) => assert_eq!(l, bencode_elem!(["spam", ["cheese"], "eggs"])),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn decode_list_empty() {
        let bytes = "e".as_bytes();
        match BencodeElem::decode_list(&mut ByteBuffer::new(bytes)) {
            Ok(l) => assert_eq!(l, bencode_elem!([])),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn decode_list_bad_structure() {
        let bytes = "4:spaml6:cheese4:eggse".as_bytes();
        match BencodeElem::decode_list(&mut ByteBuffer::new(bytes)) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_dictionary_ok() {
        let bytes = "3:cow3:moo4:spam4:eggse".as_bytes();
        match BencodeElem::decode_dictionary(&mut ByteBuffer::new(bytes)) {
            Ok(d) => assert_eq!(d, bencode_elem!({ ("cow", "moo"), ("spam", "eggs") })),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn decode_dictionary_nested() {
        let bytes = "3:cowd3:mooi4ee4:spam4:eggse".as_bytes();
        match BencodeElem::decode_dictionary(&mut ByteBuffer::new(bytes)) {
            Ok(d) => assert_eq!(
                d,
                bencode_elem!({ ("cow", { ("moo", 4_i64) }), ("spam", "eggs") })
            ),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn decode_dictionary_empty() {
        let bytes = "e".as_bytes();
        match BencodeElem::decode_dictionary(&mut ByteBuffer::new(bytes)) {
            Ok(d) => assert_eq!(d, bencode_elem!({})),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn decode_dictionary_bad_structure() {
        let bytes = "3:cow3:moo4:spame".as_bytes();
        match BencodeElem::decode_dictionary(&mut ByteBuffer::new(bytes)) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_dictionary_non_string_key() {
        let bytes = "i4e3:moo4:spam4:eggse".as_bytes();
        match BencodeElem::decode_dictionary(&mut ByteBuffer::new(bytes)) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    #[test]
    fn decode_dictionary_not_sorted() {
        let bytes = "3:zoo3:moo4:spam4:eggse".as_bytes();
        match BencodeElem::decode_dictionary(&mut ByteBuffer::new(bytes)) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(e.kind(), ErrorKind::MalformedBencode),
        }
    }

    // @note: `parse()` is called by other `decode_*()` methods, so
    // it is implicitly tested by other tests. Still, the following tests
    // are provided. Though these tests are not as comprehensive.
    #[test]
    fn parse_integer_ok() {
        let bytes = "i0e".as_bytes();
        match BencodeElem::parse(&mut ByteBuffer::new(bytes)) {
            Ok(i) => assert_eq!(i, bencode_elem!(0_i64)),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn parse_string_ok() {
        let bytes = "4:spam".as_bytes();
        match BencodeElem::parse(&mut ByteBuffer::new(bytes)) {
            Ok(s) => assert_eq!(s, bencode_elem!("spam")),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn parse_bytes_ok() {
        let bytes = vec![b'4', b':', 0xff, 0xf8, 0xff, 0xee]; // bad UTF8 gives bytes
        match BencodeElem::parse(&mut ByteBuffer::new(&bytes)) {
            Ok(bytes) => assert_eq!(bytes, bencode_elem!((0xff, 0xf8, 0xff, 0xee))),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn parse_list_ok() {
        let bytes = "l4:spam4:eggse".as_bytes();
        match BencodeElem::parse(&mut ByteBuffer::new(bytes)) {
            Ok(l) => assert_eq!(l, bencode_elem!(["spam", "eggs"])),
            Err(_) => assert!(false),
        }
    }

    #[test]
    fn parse_dictionary_ok() {
        let bytes = "d3:cow3:moo4:spam4:eggse".as_bytes();
        match BencodeElem::parse(&mut ByteBuffer::new(bytes)) {
            Ok(d) => assert_eq!(d, bencode_elem!({ ("cow", "moo"), ("spam", "eggs") })),
            Err(_) => assert!(false),
        }
    }
}
