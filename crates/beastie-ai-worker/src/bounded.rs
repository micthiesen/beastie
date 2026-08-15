use std::io::{self, BufRead};

pub(crate) enum BoundedLine {
    Line(String),
    Invalid,
}

pub(crate) fn read_bounded_line(
    reader: &mut impl BufRead,
    maximum: usize,
) -> io::Result<Option<BoundedLine>> {
    let mut bytes = Vec::new();
    let mut overflow = false;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if bytes.is_empty() && !overflow {
                return Ok(None);
            }
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |index| index + 1);
        let content = newline.map_or(consumed, |index| index);
        if !overflow {
            if bytes.len().saturating_add(content) > maximum {
                overflow = true;
                bytes.clear();
            } else {
                bytes.extend_from_slice(&available[..content]);
            }
        }
        reader.consume(consumed);
        if newline.is_some() {
            break;
        }
    }
    if overflow {
        return Ok(Some(BoundedLine::Invalid));
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    Ok(Some(match String::from_utf8(bytes) {
        Ok(line) => BoundedLine::Line(line),
        Err(_) => BoundedLine::Invalid,
    }))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn oversized_line_is_drained_without_consuming_the_next_line() {
        let mut input = Cursor::new(b"123456789\nok\n");
        assert!(matches!(
            read_bounded_line(&mut input, 4).unwrap(),
            Some(BoundedLine::Invalid)
        ));
        assert!(matches!(
            read_bounded_line(&mut input, 4).unwrap(),
            Some(BoundedLine::Line(line)) if line == "ok"
        ));
    }

    #[test]
    fn bounded_unterminated_line_is_returned_at_eof() {
        let mut input = Cursor::new(b"hello");
        assert!(matches!(
            read_bounded_line(&mut input, 5).unwrap(),
            Some(BoundedLine::Line(line)) if line == "hello"
        ));
        assert!(read_bounded_line(&mut input, 5).unwrap().is_none());
    }
}
