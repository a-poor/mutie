//! String wire recipes without evaluating source. Native AST admission resolves
//! Unicode names and source context; this reader independently checks token forms
//! and whether the resulting concatenated string can be empty.

fn newline(bytes: &[u8], offset: &mut usize) -> bool {
    match bytes.get(*offset) {
        Some(b'\n') => *offset += 1,
        Some(b'\r') => {
            *offset += 1;
            if bytes.get(*offset) == Some(&b'\n') {
                *offset += 1;
            }
        }
        _ => return false,
    }
    true
}

fn hex(bytes: &[u8], offset: &mut usize, count: usize) -> Option<u32> {
    let mut value = 0;
    for _ in 0..count {
        value = value * 16 + (*bytes.get(*offset)? as char).to_digit(16)?;
        *offset += 1;
    }
    Some(value)
}

fn literal(bytes: &[u8], offset: &mut usize) -> Option<bool> {
    let raw = matches!(bytes.get(*offset), Some(b'r' | b'R'));
    if raw || matches!(bytes.get(*offset), Some(b'u' | b'U')) {
        *offset += 1;
    }
    let quote = *bytes.get(*offset)?;
    if !matches!(quote, b'\'' | b'"') {
        return None;
    }
    let triple = bytes.get(*offset..*offset + 3) == Some(&[quote; 3]);
    let width = if triple { 3 } else { 1 };
    *offset += width;
    let mut nonempty = false;
    loop {
        if bytes
            .get(*offset..*offset + width)
            .is_some_and(|s| s.iter().all(|b| *b == quote))
        {
            *offset += width;
            return Some(nonempty);
        }
        let c = *bytes.get(*offset)?;
        *offset += 1;
        if c == b'\\' {
            if newline(bytes, offset) {
                // Raw strings retain both the slash and the normalized newline.
                nonempty |= raw;
                continue;
            }
            let escape = *bytes.get(*offset)?;
            *offset += 1;
            if !raw {
                match escape {
                    b'x' => {
                        hex(bytes, offset, 2)?;
                    }
                    b'u' => {
                        hex(bytes, offset, 4)?;
                    }
                    b'U' => {
                        if hex(bytes, offset, 8)? > 0x10ffff {
                            return None;
                        }
                    }
                    b'N' => {
                        if bytes.get(*offset) != Some(&b'{') {
                            return None;
                        }
                        *offset += 1;
                        let start = *offset;
                        while let Some(c) = bytes.get(*offset) {
                            if !(c.is_ascii_alphanumeric() || matches!(c, b' ' | b'-')) {
                                break;
                            }
                            *offset += 1;
                        }
                        if *offset == start || bytes.get(*offset) != Some(&b'}') {
                            return None;
                        }
                        *offset += 1;
                    }
                    b'0'..=b'7' => {
                        for _ in 0..2 {
                            if matches!(bytes.get(*offset), Some(b'0'..=b'7')) {
                                *offset += 1;
                            } else {
                                break;
                            }
                        }
                    }
                    // Other escapes emit a character or preserve the backslash.
                    // Native parsing remains responsible for any source warnings.
                    _ => {}
                }
            }
        } else if !triple && matches!(c, b'\r' | b'\n') {
            return None;
        }
        nonempty = true;
    }
}

pub(super) fn replacement(original: &str) -> Option<&'static str> {
    let bytes = original.as_bytes();
    if bytes.contains(&0) {
        return None;
    }
    let mut offset = 0;
    let mut nonempty = false;
    loop {
        nonempty |= literal(bytes, &mut offset)?;
        if offset == bytes.len() {
            break;
        }
        // Only literal tokens and their intervening trivia can belong to the
        // AST's combined Constant span. Never admit operators or outer grouping.
        loop {
            match bytes.get(offset) {
                Some(b' ' | b'\t' | b'\x0c' | b'\r' | b'\n') => offset += 1,
                Some(b'#') => {
                    while bytes
                        .get(offset)
                        .is_some_and(|b| !matches!(b, b'\r' | b'\n'))
                    {
                        offset += 1;
                    }
                    if !newline(bytes, &mut offset) {
                        return None;
                    }
                }
                Some(b'\\') => {
                    offset += 1;
                    if !newline(bytes, &mut offset) {
                        return None;
                    }
                }
                _ => break,
            }
        }
        // A real AST span ends exactly at the final quote, without trailing trivia.
        if offset == bytes.len() {
            return None;
        }
    }
    Some(if nonempty { "(\"\")" } else { "(\"x\")" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{mutation::Operator, python::project::mutation_operator};

    #[test]
    fn string_wire_recipes_preserve_values_and_exact_spans() {
        for source in [
            "''",
            "\"\"",
            "r''",
            "R\"\"",
            "u''",
            "U\"\"",
            "''''''",
            "\"\"\"\"\"\"",
            "'\\\n'",
            "'\\\r'",
            "'\\\r\n'",
            "'' ''",
            "r''u''",
            "'' # π\r\n r''",
            "''\\\n''",
        ] {
            assert_eq!(replacement(source), Some("(\"x\")"), "{source:?}");
            assert_eq!(
                mutation_operator(source, "(\"x\")"),
                Some(Operator::StringReplace)
            );
        }
        for source in [
            "'text'",
            "'🦀'",
            "'\u{2028}'",
            "'''\n'''",
            "\"\"\"\r\n\"\"\"",
            "'a' 'b'",
            "'' # comment\n 'value'",
            "r'\\\n'",
            r"'\0'",
            r"'\377'",
            r"'\x00'",
            r"'\u0000'",
            r"'\ud800'",
            r"'\U0010FFFF'",
            r"'\N{NULL}'",
            r"'\N{LATIN SMALL LETTER A}'",
            r"r'\x'",
            r"'\q'",
            r"r'\''",
            r#"'\"'"#,
            r#"'"'"#,
        ] {
            assert_eq!(replacement(source), Some("(\"\")"), "{source:?}");
            assert_eq!(
                mutation_operator(source, "(\"\")"),
                Some(Operator::StringReplace)
            );
        }
        for source in [
            "",
            " 'a'",
            "'a' ",
            "'a' # tail",
            "'a' + 'b'",
            "('a')",
            "'a';f()",
            "'a'.upper()",
            "b''",
            "br''",
            "rb''",
            "f''",
            "t''",
            "ur''",
            "rr''",
            "'\0'",
            "'\n'",
            "'\r'",
            "'",
            "'''",
            r"'\x0'",
            r"'\u123'",
            r"'\U00110000'",
            r"'\N{}'",
            r"'\N{x'",
            r"'\N{bad_name}'",
            "''\\x''",
            "'' # no newline",
        ] {
            assert_eq!(replacement(source), None, "{source:?}");
        }
        for wrong in ["''", "\"\"", "('')", "(\"x\")", "(0)"] {
            assert_eq!(mutation_operator("'value'", wrong), None, "{wrong}");
        }
    }
}
