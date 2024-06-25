pub fn single_quote(output: &mut String, value: &str) {
    // grow the buffer to hold the string and some extra characters
    output.reserve(value.len() + 2);
    output.push('\'');
    for c in value.chars() {
        match c {
            '\'' => {
                output.push('\\');
                output.push('\'');
            }
            _ => output.push(c),
        }
    }
    output.push('\'');
}

/// quote a string if it contains any special characters
/// This is ideal for keys on the left side of the fat-comma => operator
pub fn bare_quote(output: &mut String, value: &str) {
    // if [-+]?a-zA-Z0-9_+ then no need to quote
    if is_bareword(value) {
        output.push_str(value);
    } else {
        single_quote(output, value);
    }
}

pub fn int_quote<I>(output: &mut String, value: I)
where
    I: itoa::Integer,
{
    let mut buffer = itoa::Buffer::new();
    output.push_str(buffer.format(value));
}

pub fn float_quote<F>(output: &mut String, value: F)
where
    F: ryu::Float,
{
    let mut buffer = ryu::Buffer::new();
    output.push_str(buffer.format(value));
}

pub fn is_bareword(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_single_quote() {
        let mut output = String::new();
        super::single_quote(&mut output, "hello");
        assert_eq!(output, "'hello'");

        let mut output = String::new();
        super::single_quote(&mut output, "hello 'world'");
        assert_eq!(output, "'hello \\'world\\''");
    }
}
