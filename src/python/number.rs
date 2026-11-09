//! Exact numeric wire recipes without importing or evaluating Python source.

fn after_sign(mut text: &str) -> Option<&str> {
    loop {
        text = text.trim_start_matches([' ', '\t', '\x0c', '\r', '\n']);
        if let Some(rest) = text
            .strip_prefix("\\\r\n")
            .or_else(|| text.strip_prefix("\\\n"))
        {
            text = rest;
        } else if text.starts_with('#') {
            text = &text[text.find('\n')? + 1..];
        } else {
            return Some(text);
        }
    }
}

fn digits(bytes: &[u8], offset: &mut usize, radix: u32) -> bool {
    let start = *offset;
    while let Some(&byte) = bytes.get(*offset) {
        if (byte as char).is_digit(radix)
            || (byte == b'_'
                && *offset > start
                && bytes
                    .get(*offset + 1)
                    .is_some_and(|b| (*b as char).is_digit(radix)))
        {
            *offset += 1;
        } else {
            break;
        }
    }
    *offset > start
}

pub(super) fn replacement(original: &str) -> Option<&'static str> {
    let text = if let Some(rest) = original.strip_prefix(['+', '-']) {
        after_sign(rest)?
    } else {
        original
    };
    let radix = match text.get(..2) {
        Some("0x" | "0X") => Some(16),
        Some("0o" | "0O") => Some(8),
        Some("0b" | "0B") => Some(2),
        _ => None,
    };
    let (float, zero) = if let Some(radix) = radix {
        let body = text[2..].strip_prefix('_').unwrap_or(&text[2..]);
        let mut offset = 0;
        if !digits(body.as_bytes(), &mut offset, radix) || offset != body.len() {
            return None;
        }
        (false, body.bytes().all(|b| matches!(b, b'0' | b'_')))
    } else {
        let bytes = text.as_bytes();
        let mut offset = 0;
        let integer = digits(bytes, &mut offset, 10);
        let mut float = false;
        if bytes.get(offset) == Some(&b'.') {
            float = true;
            offset += 1;
            if !digits(bytes, &mut offset, 10) && !integer {
                return None;
            }
        } else if !integer {
            return None;
        }
        if matches!(bytes.get(offset), Some(b'e' | b'E')) {
            float = true;
            offset += 1;
            if matches!(bytes.get(offset), Some(b'+' | b'-')) {
                offset += 1;
            }
            if !digits(bytes, &mut offset, 10) {
                return None;
            }
        }
        if offset != bytes.len() {
            return None;
        }
        let zero = if float {
            text.replace('_', "").parse::<f64>().ok()? == 0.0
        } else {
            let zero = bytes.iter().all(|b| matches!(b, b'0' | b'_'));
            if bytes.first() == Some(&b'0') && !zero {
                return None;
            }
            zero
        };
        (float, zero)
    };
    Some(match (float, zero) {
        (false, false) => "(0)",
        (false, true) => "(1)",
        (true, false) => "(0.0)",
        (true, true) => "(1.0)",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{mutation::Operator, python::project::mutation_operator};

    #[test]
    fn exact_numeric_recipes_preserve_types_and_reject_expressions() {
        for (source, expected) in [
            ("0", "(1)"),
            ("00_0", "(1)"),
            ("-0", "(1)"),
            ("+ 7", "(0)"),
            ("0x_0", "(1)"),
            ("-0Xf_f", "(0)"),
            ("0b1_0", "(0)"),
            ("0o00", "(1)"),
            ("999999999999999999999999999999999999999", "(0)"),
            (".5", "(0.0)"),
            ("1.", "(0.0)"),
            ("000.0", "(1.0)"),
            ("-0.0", "(1.0)"),
            ("1e-999", "(1.0)"),
            ("1e999", "(0.0)"),
            ("2.4703282292062327e-324", "(1.0)"),
            ("2.4703282292062328e-324", "(0.0)"),
            ("1_0.2_0e+0_2", "(0.0)"),
            ("- # sign π\r\n 0.0", "(1.0)"),
            ("+\\\n 7", "(0)"),
        ] {
            assert_eq!(replacement(source), Some(expected), "{source}");
            assert_eq!(
                mutation_operator(source, expected),
                Some(Operator::NumberReplace)
            );
            for wrong in ["0", "1", "(2)", "(2.0)", "(0)", "(1)", "(0.0)", "(1.0)"] {
                if wrong != expected {
                    assert_eq!(
                        mutation_operator(source, wrong),
                        None,
                        "{source} -> {wrong}"
                    );
                }
            }
        }
        for source in [
            "",
            "True",
            "False",
            "1j",
            "-2J",
            "NaN",
            "inf",
            "01",
            "0_1",
            "_1",
            "1_",
            "1__2",
            "0x",
            "0x__1",
            "0x1g",
            "0b2",
            "0o8",
            "1._0",
            "1e",
            "1e_2",
            "1e+_2",
            ".",
            " 1",
            "1 ",
            "1 # end",
            "-(1)",
            "(1)",
            "1 + 2",
            "1; f()",
            "--1",
            "-# no newline",
        ] {
            assert_eq!(replacement(source), None, "{source}");
        }
    }
}
