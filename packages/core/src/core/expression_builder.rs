use crate::core::expression::{ExpressionNode, Operation, OperationId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionBuildError {
    InsufficientOperands {
        operation: String,
        expected: usize,
        actual: usize,
    },
    UnsupportedArity {
        operation: String,
        arity: u32,
    },
    InvalidStackState {
        remaining: usize,
    },
}

pub fn build_expression_tree_from_postfix_ops(
    postfix_ops: &[Operation],
) -> Result<Option<Box<ExpressionNode>>, ExpressionBuildError> {
    if postfix_ops.is_empty() {
        return Ok(None);
    }

    let mut stack: Vec<Box<ExpressionNode>> = Vec::with_capacity(postfix_ops.len());

    for operation in postfix_ops {
        let mut node = Box::new(ExpressionNode::leaf(operation.clone()));

        match operation.operation_type.num_args {
            0 => {}
            1 => {
                if let Some(rhs) = stack.pop() {
                    node.rhs = Some(rhs);
                } else if operation.operation_type.id != OperationId::First {
                    return Err(ExpressionBuildError::InsufficientOperands {
                        operation: operation.to_string(),
                        expected: 1,
                        actual: 0,
                    });
                }
            }
            2 => {
                if stack.len() < 2 {
                    return Err(ExpressionBuildError::InsufficientOperands {
                        operation: operation.to_string(),
                        expected: 2,
                        actual: stack.len(),
                    });
                }
                let rhs = stack.pop().expect("rhs should exist when stack len >= 2");
                let lhs = stack.pop().expect("lhs should exist when stack len >= 2");
                node.lhs = Some(lhs);
                node.rhs = Some(rhs);
            }
            arity => {
                return Err(ExpressionBuildError::UnsupportedArity {
                    operation: operation.to_string(),
                    arity,
                });
            }
        }

        stack.push(node);
    }

    match stack.len() {
        0 => Ok(None),
        1 => Ok(stack.pop()),
        remaining => Err(ExpressionBuildError::InvalidStackState { remaining }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::expression::{Operation, OperationId};

    #[test]
    fn builds_binary_tree_from_postfix_sequence() {
        let postfix = vec![
            Operation::value("1"),
            Operation::value("2"),
            Operation::binary(OperationId::Add, "+", 10),
        ];

        let tree = build_expression_tree_from_postfix_ops(&postfix)
            .expect("tree should build")
            .expect("tree should exist");

        assert_eq!(tree.operation.operation_type.id, OperationId::Add);
        assert_eq!(tree.lhs.as_ref().unwrap().operation.string_value, "1");
        assert_eq!(tree.rhs.as_ref().unwrap().operation.string_value, "2");
    }

    #[test]
    fn allows_first_without_rhs_when_stack_is_empty() {
        let postfix = vec![Operation::unary(OperationId::First, "first", 10)];

        let tree = build_expression_tree_from_postfix_ops(&postfix)
            .expect("tree should build")
            .expect("tree should exist");

        assert_eq!(tree.operation.operation_type.id, OperationId::First);
        assert!(tree.lhs.is_none());
        assert!(tree.rhs.is_none());
    }

    #[test]
    fn reports_invalid_stack_state_when_multiple_entries_remain() {
        let postfix = vec![Operation::value("1"), Operation::value("2")];

        let error = build_expression_tree_from_postfix_ops(&postfix).expect_err("should fail");

        assert_eq!(
            error,
            ExpressionBuildError::InvalidStackState { remaining: 2 }
        );
    }
}
