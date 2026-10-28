#[test]
fn base_2() {
    assert_eq!(ron::from_str("-0b101010i32"), Ok(-0b101010i32));
    assert_eq!(ron::from_str("0b101010u32"), Ok(0b101010u32));
    assert_eq!(ron::from_str("-0b10_10_10i32"), Ok(-0b101010i32));
    assert_eq!(ron::from_str("0b101_010_u32"), Ok(0b101010u32));

    assert_eq!(
        ron::from_str::<i32>("-0b101010i31"),
        Err(ron::error::SpannedError {
            code: ron::error::Error::TrailingCharacters,
            span: ron::error::Span {
                start: ron::error::Position { line: 1, col: 10 },
                end: ron::error::Position { line: 1, col: 10 }
            }
        })
    );
}

#[test]
fn base_8() {
    assert_eq!(ron::from_str("-0o52i32"), Ok(-0o52i32));
    assert_eq!(ron::from_str("0o52u32"), Ok(0o52u32));
    assert_eq!(ron::from_str("-0o52_i32"), Ok(-0o52i32));
    assert_eq!(ron::from_str("0o5_2u32"), Ok(0o52u32));

    assert_eq!(
        ron::from_str::<i32>("0o_52_i32"),
        Err(ron::error::SpannedError {
            code: ron::error::Error::UnderscoreAtBeginning,
            span: ron::error::Span {
                start: ron::error::Position { line: 1, col: 7 },
                end: ron::error::Position {
                    line: 1,
                    col: 3, // FIXME
                }
            }
        })
    );
}

#[test]
fn base_10() {
    assert_eq!(ron::from_str("-42i32"), Ok(-42i32));
    assert_eq!(ron::from_str("42u32"), Ok(42u32));
    assert_eq!(ron::from_str("-42_i32"), Ok(-42i32));
    assert_eq!(ron::from_str("4_2u32"), Ok(42u32));

    assert_eq!(ron::from_str("00i32"), Ok(0i32));
}

#[test]
fn base_16() {
    assert_eq!(ron::from_str("-0x2Ai32"), Ok(-0x2Ai32));
    assert_eq!(ron::from_str("0x2Au32"), Ok(0x2Au32));
    assert_eq!(ron::from_str("-0x2_Ai32"), Ok(-0x2Ai32));
    assert_eq!(ron::from_str("0x2A_u32"), Ok(0x2Au32));

    assert_eq!(
        ron::from_str::<i32>("0x2Aj32"),
        Err(ron::error::SpannedError {
            code: ron::error::Error::TrailingCharacters,
            span: ron::error::Span {
                start: ron::error::Position { line: 1, col: 5 },
                end: ron::error::Position { line: 1, col: 5 }
            }
        })
    );
}
