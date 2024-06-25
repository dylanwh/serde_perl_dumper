use serde::{
    de::{self, IntoDeserializer, Visitor},
    forward_to_deserialize_any, Deserialize,
};
use std::borrow::Cow;

use crate::error::{Error, Result};
use crate::parser::{self, Array, Hash, Reference, Scalar};

pub struct Deserializer<'de> {
    scalar: Cow<'de, Scalar>,
}

impl<'de> Deserializer<'de> {
    fn new(scalar: Cow<'de, Scalar>) -> Self {
        Deserializer { scalar }
    }
}

pub fn from_perl<'de, T>(scalar: &'de Scalar) -> Result<T>
where
    T: Deserialize<'de>,
{
    let deserializer = Deserializer::new(Cow::Borrowed(scalar));
    T::deserialize(deserializer)
}

pub fn from_str<'de, T>(scalar: &'de str) -> Result<T>
where
    T: Deserialize<'de>,
{
    let scalar = parser::parse(scalar)?;
    let deserializer = Deserializer::new(Cow::Owned(scalar));
    T::deserialize(deserializer)
}

impl<'de> de::Deserializer<'de> for Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let scalar = self.scalar.into_owned();
        match scalar {
            Scalar::Undefined => visitor.visit_unit(),
            Scalar::Int(i) => visitor.visit_i64(i),
            Scalar::Float(f) => visitor.visit_f64(f),
            Scalar::String(s) => visitor.visit_string(s),
            Scalar::Reference(r) => match *r {
                Reference::Hash(h) => {
                    let Hash(h) = *h;
                    let mut map = serde::de::value::MapDeserializer::new(h.into_iter());
                    visitor.visit_map(&mut map)
                }
                Reference::Array(a) => {
                    let Array(a) = *a;
                    let mut seq = serde::de::value::SeqDeserializer::new(a.into_iter());
                    visitor.visit_seq(&mut seq)
                }
                Reference::Scalar(s) => {
                    let deserializer = s.into_deserializer();
                    deserializer.deserialize_any(visitor)
                }
            },
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}

impl<'de> IntoDeserializer<'de, Error> for &'de Scalar {
    type Deserializer = Deserializer<'de>;

    fn into_deserializer(self) -> Self::Deserializer {
        Deserializer::new(Cow::Borrowed(self))
    }
}

impl<'de> IntoDeserializer<'de, Error> for Scalar {
    type Deserializer = Deserializer<'de>;

    fn into_deserializer(self) -> Self::Deserializer {
        Deserializer::new(Cow::Owned(self))
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{Hash, Reference};

    use super::*;
    use serde::Deserialize;

    #[test]
    fn test_deserialize() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Test {
            a: i32,
            b: String,
        }

        let scalar = Scalar::Reference(Box::new(Reference::Hash(Box::new(Hash(
            vec![
                (String::from("a"), Scalar::Int(42)),
                (String::from("b"), Scalar::String(String::from("hello"))),
            ]
            .into_iter()
            .collect::<std::collections::HashMap<String, Scalar>>(),
        )))));

        let test: Test = from_perl(&scalar).unwrap();
        assert_eq!(
            test,
            Test {
                a: 42,
                b: "hello".to_string()
            }
        );
    }

    // let's parse some perl
    #[test]
    fn test_deserialize_perl() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Test {
            a: i32,
            b: String,
            topic: String,
            c: Vec<i32>,
        }

        let scalar =
            parser::parse(r#"{a => 42, 'b' => 'hello', "topic" => "\nworld", 'c' => [1, 2, 3]}"#)
                .unwrap();
        let test: Test = from_perl(&scalar).unwrap();
        assert_eq!(
            test,
            Test {
                a: 42,
                b: "hello".to_string(),
                topic: "\nworld".to_string(),
                c: vec![1, 2, 3]
            }
        );
    }
}
