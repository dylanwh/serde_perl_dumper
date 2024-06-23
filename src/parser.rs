#[allow(unused_imports)]
use nom::{
    branch::alt,
    bytes::complete::{escaped, tag},
    character::complete::{char, digit1, multispace0, multispace1},
    combinator::{map, map_res, opt},
    multi::{many0, separated_list0},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};
use nom::{
    bytes::complete::{escaped_transform, take_while1},
    character::complete::{none_of, one_of},
    combinator::value,
    error::ErrorKind,
    multi::many1,
    AsChar, InputTakeAtPosition,
};
use std::collections::HashMap;

/// These are all the characters that can be used as delimiters in Perl's `q` operator, I think.
/// There might be more, and possibly unicode characters, but I don't need those for now.
const PUNCTUATION: &str = r##"!"#$%&'(*+,-/:;<=?@[\^`{|~"##;

pub fn parse(input: &str) -> crate::error::Result<Scalar> {
    let (_, scalar) =
        parse_scalar(input).map_err(|e| crate::error::Error::Nom(format!("{e}")))?;
    Ok(scalar)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Scalar {
    Undefined,
    Int(i64),
    Float(f64),
    String(String),
    Reference(Box<Reference>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hash(pub HashMap<String, Scalar>);

#[derive(Debug, Clone, PartialEq)]
pub struct Array(pub Vec<Scalar>);

#[derive(Debug, Clone, PartialEq)]
pub enum Reference {
    Hash(Box<Hash>),
    Array(Box<Array>),
    Scalar(Box<Scalar>),
}

fn parse_scalar(input: &str) -> IResult<&str, Scalar> {
    alt((parse_literal_scalar, parse_reference))(input)
}

fn parse_reference(input: &str) -> IResult<&str, Scalar> {
    let (input, reference) = alt((parse_hashref, parse_arrayref, parse_scalarref))(input)?;

    Ok((input, Scalar::Reference(Box::new(reference))))
}

fn parse_scalarref(input: &str) -> IResult<&str, Reference> {
    let (input, _) = char('\\')(input)?;
    let (input, scalar) = parse_scalar(input)?;

    Ok((input, Reference::Scalar(Box::new(scalar))))
}

fn parse_hashref(input: &str) -> IResult<&str, Reference> {
    let (input, _) = char('{')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, pairs) = separated_list0(comma, parse_pair)(input)?;
    let (input, _) = opt(comma)(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('}')(input)?;

    let mut hash = HashMap::new();
    for (key, value) in pairs {
        if let Scalar::String(key) = key {
            hash.insert(key, value);
        } else {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                ErrorKind::Char,
            )));
        }
    }

    Ok((input, Reference::Hash(Box::new(Hash(hash)))))
}

fn parse_pair(input: &str) -> IResult<&str, (Scalar, Scalar)> {
    alt((parse_fatcomma_pair, parse_comma_pair))(input)
}

fn parse_comma_pair(input: &str) -> IResult<&str, (Scalar, Scalar)> {
    let (input, _) = multispace0(input)?;
    let (input, key) = parse_literal_scalar(input)?;
    let (input, _) = comma(input)?;
    let (input, value) = parse_scalar(input)?;

    Ok((input, (key, value)))
}

fn parse_fatcomma_pair(input: &str) -> IResult<&str, (Scalar, Scalar)> {
    let (input, _) = multispace0(input)?;
    let (input, key) = parse_bareword_or_literal(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("=>")(input)?;
    let (input, _) = multispace0(input)?;
    let (input, value) = parse_scalar(input)?;

    Ok((input, (key, value)))
}

fn parse_bareword_or_literal(input: &str) -> IResult<&str, Scalar> {
    alt((parse_bareword, parse_literal_scalar))(input)
}

fn parse_bareword(input: &str) -> IResult<&str, Scalar> {
    let (input, s) = take_while1(|c: char| c.is_ascii_alphanumeric() || c == '_')(input)?;

    Ok((input, Scalar::String(s.to_string())))
}

fn comma(input: &str) -> IResult<&str, char> {
    delimited(multispace0, char(','), multispace0)(input)
}

/* [ "foo", 1.0, 2, undef, ] */
fn parse_arrayref(input: &str) -> IResult<&str, Reference> {
    let (input, _) = char('[')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, scalars) = separated_list0(comma, parse_scalar)(input)?;
    let (input, _) = opt(comma)(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char(']')(input)?;

    Ok((input, Reference::Array(Box::new(Array(scalars)))))
}

fn parse_literal_scalar(input: &str) -> IResult<&str, Scalar> {
    let (input, _) = multispace0(input)?;
    alt((parse_undef, parse_number, parse_string))(input)
}

fn parse_undef(input: &str) -> IResult<&str, Scalar> {
    let (input, _) = tag("undef")(input)?;

    Ok((input, Scalar::Undefined))
}

fn parse_string(input: &str) -> IResult<&str, Scalar> {
    alt((
        parse_single_quoted_string,
        parse_double_quoted_string,
        parse_q_string,
    ))(input)
}

fn perl_digit1(input: &str) -> IResult<&str, &str> {
    input.split_at_position1_complete(|item| !is_perl_digit(item), ErrorKind::Digit)
}

fn is_perl_digit(c: char) -> bool {
    c.is_dec_digit() || c == '_'
}

fn parse_number(input: &str) -> IResult<&str, Scalar> {
    let (input, parts) = tuple((
        opt(char('-')),
        perl_digit1,
        opt(tuple((char('.'), many1(perl_digit1)))),
        opt(preceded(one_of("eE"), digit1)),
    ))(input)?;

    match parts {
        (sign, int, None, e) => {
            let sign = sign.map(|c| c.to_string()).unwrap_or_default();
            let s = format!("{}{}{}", sign, int, e.unwrap_or(""));
            let s = s.replace('_', "");
            let i = s.parse::<i64>().unwrap();
            Ok((input, Scalar::Int(i)))
        }
        (sign, int, Some((_, frac)), e) => {
            let sign = sign.map(|c| c.to_string()).unwrap_or_default();
            let s = format!("{}{}.{}{}", sign, int, frac.join(""), e.unwrap_or(""));
            let s = s.replace('_', "");
            let f = s.parse::<f64>().unwrap();
            Ok((input, Scalar::Float(f)))
        }
    }
}

fn parse_single_quoted_string(input: &str) -> IResult<&str, Scalar> {
    let (input, s) = delimited(
        char('\''),
        escaped(none_of("\\'"), '\\', one_of("'\\")),
        char('\''),
    )(input)?;

    Ok((input, Scalar::String(s.to_string())))
}

fn parse_double_quoted_string(input: &str) -> IResult<&str, Scalar> {
    let (input, s) = delimited(
        char('"'),
        escaped_transform(
            none_of("\\\""),
            '\\',
            alt((
                value("\\", tag("\\")),
                value("\"", tag("\"")),
                value("\n", tag("n")),
                value("\r", tag("r")),
                value("\t", tag("t")),
                value("\0", tag("0")),
                value("\x0B", tag("v")),
                value("\x08", tag("b")),
                value("\x07", tag("a")),
                value("\x1B", tag("e")),
                value("\x1F", tag("z")),
            )),
        ),
        char('"'),
    )(input)?;

    Ok((input, Scalar::String(s.to_string())))
}

/// this parses:
/// - q(foo)
/// - q{foo}
/// - q[foo]
/// - q<foo>
/// - q"foo"
/// - q'foo'
/// - q!foo!
/// - q@foo@
/// etc
fn parse_q_string(input: &str) -> IResult<&str, Scalar> {
    let (input, _) = char('q')(input)?;
    // delim is any char that is not a letter, digit, or underscore
    let (input, start_delim) = one_of(PUNCTUATION)(input)?;
    let end_delim = paired_quote_delimiter(start_delim);
    let esc = format!("{}\\", end_delim);
    let (input, s) = escaped(many0(none_of(esc.as_str())), '\\', one_of(esc.as_str()))(input)?;
    let (input, _) = char(end_delim)(input)?;

    Ok((input, Scalar::String(s.to_string())))
}

fn paired_quote_delimiter(c: char) -> char {
    match c {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        '<' => '>',
        c => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_undef() {
        let input = "undef";
        let expected = Scalar::Undefined;
        let actual = parse_undef(input).unwrap().1;
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_parse_string() {
        let input = "'hello'";
        let expected = Scalar::String("hello".to_string());
        let actual = parse_single_quoted_string(input).unwrap().1;
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_parse_q_string() {
        let input = "q{hello}";
        let expected = Scalar::String("hello".to_string());
        let actual = parse_q_string(input).unwrap().1;
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_parse_literal_scalar() {
        let input = "undef";
        let expected = Scalar::Undefined;
        let actual = parse_literal_scalar(input).unwrap().1;
        assert_eq!(expected, actual);

        let input = "123";
        let expected = Scalar::Int(123);
        let actual = parse_literal_scalar(input).unwrap().1;
        assert_eq!(expected, actual);

        let input = "123.456";
        let expected = Scalar::Float(123.456);
        let actual = parse_literal_scalar(input).unwrap().1;
        assert_eq!(expected, actual);

        let input = "'hello'";
        let expected = Scalar::String("hello".to_string());
        let actual = parse_literal_scalar(input).unwrap().1;
        assert_eq!(expected, actual);

        let input = "q{hello}";
        let expected = Scalar::String("hello".to_string());
        let actual = parse_literal_scalar(input).unwrap().1;
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_parse_pair() {
        let input = "'foo'=>123";
        let expected = (Scalar::String("foo".to_string()), Scalar::Int(123));
        let actual = parse_pair(input).unwrap().1;
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_hashref() {
        let input = "{ 'foo' => 'bar' }";
        let expected = Reference::Hash(Box::new(Hash(
            vec![("foo".to_string(), Scalar::String("bar".to_string()))]
                .into_iter()
                .collect(),
        )));
        let actual = parse_hashref(input).unwrap().1;
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_arrayref() {
        let input = "[ 'foo', 'bar' ]";
        let expected = Reference::Array(Box::new(Array(vec![
            Scalar::String("foo".to_string()),
            Scalar::String("bar".to_string()),
        ])));
        let actual = parse_arrayref(input).unwrap().1;
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_scalarref() {
        let input = "\\123";
        let expected = Reference::Scalar(Box::new(Scalar::Int(123)));
        let actual = parse_scalarref(input).unwrap().1;
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_array_trailing_comma() {
        let input = "[ 'foo', 'bar', ]";
        let expected = Reference::Array(Box::new(Array(vec![
            Scalar::String("foo".to_string()),
            Scalar::String("bar".to_string()),
        ])));
        let actual = parse_arrayref(input).unwrap().1;
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_hash_trailing_comma() {
        let input = "{ 'foo' => 'bar', }";
        let expected = Reference::Hash(Box::new(Hash(
            vec![("foo".to_string(), Scalar::String("bar".to_string()))]
                .into_iter()
                .collect(),
        )));
        let actual = parse_hashref(input).unwrap().1;
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_array_of_hash() {
        let input = "[ { 'foo' => 'bar' }, { 'baz' => 'qux' } ]";
        parse_arrayref(input).unwrap();
    }

    #[test]
    fn test_deeply_nested() {
        let input = "{ 'foo' => [ 'bar', { 'baz' => 'qux' } ] }";

        let actual = parse_hashref(input).unwrap().1;
        let foo = "foo".to_string();
        let bar = "bar".to_string();
        let baz = "baz".to_string();
        let qux = Scalar::String("qux".to_string());
        let bazqux = Hash(vec![(baz, qux)].into_iter().collect());
        let barbazqux = Array(vec![
            Scalar::String(bar),
            Scalar::Reference(Box::new(Reference::Hash(Box::new(bazqux)))),
        ]);
        let expected = Reference::Hash(Box::new(Hash(
            vec![(
                foo,
                Scalar::Reference(Box::new(Reference::Array(Box::new(barbazqux)))),
            )]
            .into_iter()
            .collect(),
        )));
        assert_eq!(expected, actual);
    }
}
