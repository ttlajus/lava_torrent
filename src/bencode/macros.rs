use super::*;

// a macro to be used in tests to reduce boilerplate code
// informal syntax:
// -integer: as-is, works for (u8, u16, u32, i8, i16, i32, i64), conversion should be lossless
// -string: as-is, works for both owned and borrowed strings
// -bytes: (b1, b2, ...), support trailing comma
// -list: [e1, e2, ...], support trailing comma
// -dictionary: { (k1, v1), (k2, v2), ... }, support trailing comma (no trailing comma in K-V pair)
#[macro_export]
macro_rules! bencode_elem {
    ([ $( $element:tt ),* ]) => {
        $crate::bencode::BencodeElem::List(vec![ $( bencode_elem!($element) ),* ])
    };
    ([ $( $element:tt ),+ ,]) => {
        bencode_elem!([ $( $element ),* ])
    };
    (( $( $element:tt ),* )) => {
        $crate::bencode::BencodeElem::Bytes(vec![ $( $element ),* ])
    };
    (( $( $element:tt ),+ ,)) => {
        bencode_elem!(( $( $element ),* ))
    };
    ({ $( ($key:tt, $val:tt) ),* }) => {
        $crate::bencode::BencodeElem::Dictionary(
            ::std::collections::HashMap::from_iter(
                vec![ $( ($key.to_owned(), bencode_elem!($val)) ),* ].into_iter()
            )
        )
    };
    ({ $( ($key:tt, $val:tt) ),+ ,}) => {
        bencode_elem!({ $( ($key, $val) ),* })
    };
    ($other:expr) => {
        $crate::bencode::BencodeElem::from($other)
    }
}

#[cfg(test)]
mod bencode_elem_macro_tests {
    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn u8_to_integer_ok() {
        assert_eq!(bencode_elem!(0_u8), BencodeElem::Integer(0))
    }

    #[test]
    fn u16_to_integer_ok() {
        assert_eq!(bencode_elem!(0_u16), BencodeElem::Integer(0))
    }

    #[test]
    fn u32_to_integer_ok() {
        assert_eq!(bencode_elem!(0_u32), BencodeElem::Integer(0))
    }

    #[test]
    fn i8_to_integer_ok() {
        assert_eq!(bencode_elem!(0_i8), BencodeElem::Integer(0))
    }

    #[test]
    fn i16_to_integer_ok() {
        assert_eq!(bencode_elem!(0_i16), BencodeElem::Integer(0))
    }

    #[test]
    fn i32_to_integer_ok() {
        assert_eq!(bencode_elem!(0_i32), BencodeElem::Integer(0))
    }

    #[test]
    fn i64_to_integer_ok() {
        assert_eq!(bencode_elem!(0_i64), BencodeElem::Integer(0))
    }

    #[test]
    fn str_ref_to_integer_ok() {
        assert_eq!(bencode_elem!(""), BencodeElem::String("".to_owned()))
    }

    #[test]
    fn string_to_integer_ok() {
        let string = "".to_owned();
        assert_eq!(bencode_elem!(string), BencodeElem::String("".to_owned()))
    }

    #[test]
    fn bytes_ok() {
        assert_eq!(
            bencode_elem!((0x01, 0x02)),
            BencodeElem::Bytes(vec![0x01, 0x02])
        )
    }

    #[test]
    fn bytes_empty() {
        assert_eq!(bencode_elem!(()), BencodeElem::Bytes(vec![]))
    }

    #[test]
    fn list_ok() {
        assert_eq!(
            bencode_elem!([0x01, "0x02", [0x03]]),
            BencodeElem::List(vec![
                BencodeElem::Integer(0x01),
                BencodeElem::String("0x02".to_owned()),
                BencodeElem::List(vec![BencodeElem::Integer(0x03)]),
            ])
        )
    }

    #[test]
    fn list_empty() {
        assert_eq!(bencode_elem!([]), BencodeElem::List(vec![]))
    }

    #[test]
    fn dict_ok() {
        assert_eq!(
            bencode_elem!({ ("cow", { ("moo", 4) }), ("spam", "eggs") }),
            BencodeElem::Dictionary(HashMap::from_iter(
                vec![
                    (
                        "cow".to_owned(),
                        BencodeElem::Dictionary(HashMap::from_iter(
                            vec![("moo".to_owned(), BencodeElem::Integer(4_i64))].into_iter(),
                        )),
                    ),
                    ("spam".to_owned(), BencodeElem::String("eggs".to_owned())),
                ].into_iter()
            ))
        )
    }

    #[test]
    fn dict_empty() {
        assert_eq!(bencode_elem!({}), BencodeElem::Dictionary(HashMap::new()))
    }
}
