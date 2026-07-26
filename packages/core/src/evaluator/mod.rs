use std::collections::BTreeMap;

use crate::errors::CoreError;

pub mod all_at_once_evaluator;
pub mod stream_evaluator;

pub use all_at_once_evaluator::{AllAtOnceEvaluator, Input};
pub use stream_evaluator::{ReaderInput, StreamEvaluator};

/// A numeric value whose source type is preserved through evaluation.
///
/// A floating-point value that happens to be whole (such as `12.0`) must not
/// be collapsed into an integer: callers use this distinction when writing a
/// result back to JSON or a tree node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Numeric {
    Int(i64),
    Float(f64),
}

impl std::fmt::Display for Numeric {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(value) => value.fmt(formatter),
            Self::Float(value) => value.fmt(formatter),
        }
    }
}

impl Numeric {
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Int(value) => value as f64,
            Self::Float(value) => value,
        }
    }

    pub fn is_zero(self) -> bool {
        match self {
            Self::Int(value) => value == 0,
            Self::Float(value) => value == 0.0,
        }
    }

    pub fn display(self) -> String {
        match self {
            Self::Int(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(Numeric),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(value) => *value,
            Value::Number(value) => !value.is_zero(),
            Value::String(value) => !value.is_empty(),
            Value::Array(value) => !value.is_empty(),
            Value::Object(value) => !value.is_empty(),
        }
    }

    pub fn from_literal(literal: &str) -> Result<Self, EvaluationError> {
        let trimmed = literal.trim();
        if literal != trimmed && !literal.is_empty() {
            return Ok(Value::String(literal.to_string()));
        }
        if trimmed.eq_ignore_ascii_case("null") {
            return Ok(Value::Null);
        }
        if trimmed.eq_ignore_ascii_case("true") {
            return Ok(Value::Bool(true));
        }
        if trimmed.eq_ignore_ascii_case("false") {
            return Ok(Value::Bool(false));
        }
        if ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
            && trimmed.len() >= 2
        {
            return Ok(Value::String(trimmed[1..trimmed.len() - 1].to_string()));
        }
        if let Ok(value) = trimmed.parse::<i64>() {
            return Ok(Value::Number(Numeric::Int(value)));
        }
        if let Ok(value) = trimmed.parse::<f64>() {
            return Ok(Value::Number(Numeric::Float(value)));
        }
        Ok(Value::String(trimmed.to_string()))
    }

    pub fn as_numeric(&self) -> Result<Numeric, EvaluationError> {
        match self {
            Value::Number(value) => Ok(*value),
            Value::String(value) => Value::from_literal(value).and_then(|value| match value {
                Value::Number(value) => Ok(value),
                _ => Err(EvaluationError::TypeMismatch {
                    expected: "number",
                    actual: "string".to_string(),
                }),
            }),
            Value::Bool(_) => Err(EvaluationError::TypeMismatch {
                expected: "number",
                actual: "bool".to_string(),
            }),
            Value::Null => Err(EvaluationError::TypeMismatch {
                expected: "number",
                actual: "null".to_string(),
            }),
            Value::Array(_) => Err(EvaluationError::TypeMismatch {
                expected: "number",
                actual: "array".to_string(),
            }),
            Value::Object(_) => Err(EvaluationError::TypeMismatch {
                expected: "number",
                actual: "object".to_string(),
            }),
        }
    }

    pub fn as_number(&self) -> Result<f64, EvaluationError> {
        Ok(self.as_numeric()?.as_f64())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluationError {
    MissingOperand(&'static str),
    DivisionByZero,
    TypeMismatch {
        expected: &'static str,
        actual: String,
    },
    UnsupportedOperation(String),
    Core(CoreError),
}

impl From<CoreError> for EvaluationError {
    fn from(value: CoreError) -> Self {
        Self::Core(value)
    }
}
