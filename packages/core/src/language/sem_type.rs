use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SemType {
    Map,
    Seq,
    Str,
    Int,
    Float,
    Boolean,
    Nil,
}

impl SemType {
    pub fn tag(self) -> &'static str {
        match self {
            SemType::Map => "!!map",
            SemType::Seq => "!!seq",
            SemType::Str => "!!str",
            SemType::Int => "!!int",
            SemType::Float => "!!float",
            SemType::Boolean => "!!bool",
            SemType::Nil => "!!null",
        }
    }

    pub fn from_string(value: &str) -> Option<Self> {
        const PAIRS: &[(&str, SemType)] = &[
            ("!!map", SemType::Map),
            ("!!seq", SemType::Seq),
            ("!!str", SemType::Str),
            ("!!int", SemType::Int),
            ("!!float", SemType::Float),
            ("!!bool", SemType::Boolean),
            ("!!null", SemType::Nil),
        ];

        PAIRS
            .iter()
            .find(|(candidate, _)| *candidate == value)
            .map(|(_, sem_type)| *sem_type)
    }

    pub fn has_tag_prefix(value: &str) -> bool {
        value.starts_with("!!")
    }

    pub fn to_value_type(self) -> &'static str {
        match self {
            SemType::Map => "object",
            SemType::Seq => "array",
            SemType::Str => "string",
            SemType::Int | SemType::Float => "number",
            SemType::Boolean => "boolean",
            SemType::Nil => "null",
        }
    }
}

impl fmt::Display for SemType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.tag())
    }
}
