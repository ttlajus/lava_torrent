use std::fmt;
use std::convert::From;
use std::collections::HashMap;
use itertools;
use itertools::Itertools;

#[cfg(test)]
#[macro_use]
mod macros;
mod read;
pub mod write;

const DICTIONARY_PREFIX: u8 = b'd';
const DICTIONARY_POSTFIX: u8 = b'e';
const LIST_PREFIX: u8 = b'l';
const LIST_POSTFIX: u8 = b'e';
const INTEGER_PREFIX: u8 = b'i';
const INTEGER_POSTFIX: u8 = b'e';
const STRING_DELIMITER: u8 = b':';

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BencodeElem {
    String(String),
    Bytes(Vec<u8>),
    Integer(i64),
    List(Vec<BencodeElem>),
    Dictionary(HashMap<String, BencodeElem>),
}

impl From<u8> for BencodeElem {
    fn from(val: u8) -> BencodeElem {
        BencodeElem::Integer(i64::from(val))
    }
}

impl From<u16> for BencodeElem {
    fn from(val: u16) -> BencodeElem {
        BencodeElem::Integer(i64::from(val))
    }
}

impl From<u32> for BencodeElem {
    fn from(val: u32) -> BencodeElem {
        BencodeElem::Integer(i64::from(val))
    }
}

impl From<i8> for BencodeElem {
    fn from(val: i8) -> BencodeElem {
        BencodeElem::Integer(i64::from(val))
    }
}

impl From<i16> for BencodeElem {
    fn from(val: i16) -> BencodeElem {
        BencodeElem::Integer(i64::from(val))
    }
}

impl From<i32> for BencodeElem {
    fn from(val: i32) -> BencodeElem {
        BencodeElem::Integer(i64::from(val))
    }
}

impl From<i64> for BencodeElem {
    fn from(val: i64) -> BencodeElem {
        BencodeElem::Integer(val)
    }
}

impl<'a> From<&'a str> for BencodeElem {
    fn from(val: &'a str) -> BencodeElem {
        BencodeElem::String(val.to_string())
    }
}

impl From<String> for BencodeElem {
    fn from(val: String) -> BencodeElem {
        BencodeElem::String(val)
    }
}

impl<'a> From<&'a [u8]> for BencodeElem {
    fn from(val: &'a [u8]) -> BencodeElem {
        BencodeElem::Bytes(val.to_vec())
    }
}

impl From<Vec<u8>> for BencodeElem {
    fn from(val: Vec<u8>) -> BencodeElem {
        BencodeElem::Bytes(val)
    }
}

impl fmt::Display for BencodeElem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            BencodeElem::String(ref string) => write!(f, "\"{}\"", string),
            BencodeElem::Bytes(ref bytes) => write!(f, "[{:#02x}]", bytes.iter().format(", ")),
            BencodeElem::Integer(ref int) => write!(f, "{}", int),
            BencodeElem::List(ref list) => write!(f, "[{}]", itertools::join(list, ", ")),
            BencodeElem::Dictionary(ref dict) => write!(
                f,
                "{{ {} }}",
                dict.iter()
                    .sorted_by(|&(k1, _), &(k2, _)| k1.as_bytes().cmp(k2.as_bytes()))
                    .iter()
                    .format_with(", ", |&(k, v), f| f(&format_args!("(\"{}\", {})", k, v)))
            ),
        }
    }
}

#[cfg(test)]
mod bencode_elem_display_tests {
    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn display_test_string() {
        assert_eq!(bencode_elem!("").to_string(), "\"\"");
    }

    #[test]
    fn display_test_bytes() {
        assert_eq!(
            bencode_elem!((0xff, 0xf8, 0xff, 0xee)).to_string(),
            "[0xff, 0xf8, 0xff, 0xee]"
        );
    }

    #[test]
    fn display_test_integer() {
        assert_eq!(bencode_elem!(0).to_string(), "0");
    }

    #[test]
    fn display_test_list() {
        assert_eq!(bencode_elem!([0, "spam"]).to_string(), "[0, \"spam\"]");
    }

    #[test]
    fn display_test_dictionary() {
        assert_eq!(
            bencode_elem!({ ("cow", { ("moo", 4) }), ("spam", "eggs") }).to_string(),
            "{ (\"cow\", { (\"moo\", 4) }), (\"spam\", \"eggs\") }",
        )
    }
}
