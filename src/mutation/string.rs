//! Bounded interpreted/raw Go string edits; no source evaluation or type inference.
use tree_sitter::Node;

fn digits(bytes: &[u8], offset: &mut usize, count: usize, radix: u32) -> Option<u32> {
    let mut value = 0;
    for _ in 0..count {
        value = value * radix + (*bytes.get(*offset)? as char).to_digit(radix)?;
        *offset += 1;
    }
    Some(value)
}

fn recipe(original: &str) -> Option<&'static str> {
    let bytes = original.as_bytes();
    // Go's compiler rejects literal source NULs and interior BOMs. Escaped
    // representations of either value remain valid strings.
    if bytes.contains(&0) || original.contains('\u{feff}') {
        return None;
    }
    let nonempty = match bytes.first()? {
        b'`' => {
            let body = original.strip_prefix('`')?.strip_suffix('`')?;
            if body.contains('`') {
                return None;
            }
            // Unlike interpreted strings, raw literals discard every CR.
            body.bytes().any(|b| b != b'\r')
        }
        b'"' => {
            let mut i = 1;
            let mut nonempty = false;
            loop {
                let byte = *bytes.get(i)?;
                i += 1;
                match byte {
                    b'"' => {
                        if i != bytes.len() {
                            return None;
                        }
                        break;
                    }
                    b'\n' => return None,
                    b'\\' => {
                        let escape = *bytes.get(i)?;
                        i += 1;
                        match escape {
                            b'a' | b'b' | b'f' | b'n' | b'r' | b't' | b'v' | b'\\' | b'"' => {}
                            b'0'..=b'7' => {
                                i -= 1;
                                if digits(bytes, &mut i, 3, 8)? > 255 {
                                    return None;
                                }
                            }
                            b'x' => {
                                digits(bytes, &mut i, 2, 16)?;
                            }
                            b'u' | b'U' => {
                                let value =
                                    digits(bytes, &mut i, if escape == b'u' { 4 } else { 8 }, 16)?;
                                char::from_u32(value)?;
                            }
                            _ => return None,
                        }
                    }
                    _ => {}
                }
                // Every legal interpreted-string character/escape emits bytes.
                nonempty = true;
            }
            nonempty
        }
        _ => return None,
    };
    Some(if nonempty { "\"\"" } else { "\"x\"" })
}

pub(super) fn replacement(node: Node<'_>, source: &str) -> Option<&'static str> {
    if !matches!(
        node.kind(),
        "interpreted_string_literal" | "raw_string_literal"
    ) || !super::literal::value_context(node)
    {
        return None;
    }
    recipe(&source[node.byte_range()])
}

pub(crate) fn valid_replacement(original: &str, changed: &str) -> bool {
    recipe(original) == Some(changed)
}

#[cfg(test)]
#[path = "string_tests.rs"]
mod tests;
