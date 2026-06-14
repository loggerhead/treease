use std::collections::BTreeMap;

use crate::core::CoreError;

pub mod all_at_once_evaluator;
pub mod stream_evaluator;

pub use all_at_once_evaluator::{AllAtOnceEvaluator, Input};
pub use stream_evaluator::{ReaderInput, StreamEvaluator};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(value) => *value,
            Value::Number(value) => *value != 0.0,
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
        if let Ok(value) = trimmed.parse::<f64>() {
            return Ok(Value::Number(value));
        }
        Ok(Value::String(trimmed.to_string()))
    }

    pub fn as_number(&self) -> Result<f64, EvaluationError> {
        match self {
            Value::Number(value) => Ok(*value),
            Value::String(value) => {
                value
                    .parse::<f64>()
                    .map_err(|_| EvaluationError::TypeMismatch {
                        expected: "number",
                        actual: "string".to_string(),
                    })
            }
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
