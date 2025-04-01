use std::collections::{HashMap, HashSet};

use crate::ast::indices::{AstIdx, NameIdx};
use crate::ast::pool::AstPool;
use crate::ast::primitives::PrimitiveFunc;
use crate::ast::Ast;
use crate::checker::error::{CheckerError, Result};

pub struct TypeChecker<'a> {
    ast_pool: &'a AstPool,
    function_param_counts: HashMap<NameIdx, usize>,
}

impl<'a> TypeChecker<'a> {
    pub fn new(ast_pool: &'a AstPool) -> Self {
        let mut function_param_counts = HashMap::new();

        for (&name_idx, &ast_idx) in &ast_pool.function_defs {
            if let Ast::FunctionDef { param_count, .. } = ast_pool[ast_idx] {
                function_param_counts.insert(name_idx, param_count);
            }
        }

        Self {
            ast_pool,
            function_param_counts,
        }
    }

    pub fn check_program(&mut self) -> Result<()> {
        for (&name_idx, &ast_idx) in &self.ast_pool.function_defs {
            self.check_function_def(name_idx, ast_idx)?;
        }

        Ok(())
    }

    fn check_function_def(&mut self, _name_idx: NameIdx, ast_idx: AstIdx) -> Result<()> {
        if let Ast::FunctionDef { body_idx, .. } = self.ast_pool[ast_idx] {
            self.check_expression(body_idx)?;

            Ok(())
        } else {
            Err(CheckerError::InternalError(format!(
                "Expected FunctionDef but got {:?}",
                self.ast_pool[ast_idx]
            )))
        }
    }

    fn check_expression(&mut self, expr_idx: AstIdx) -> Result<()> {
        match self.ast_pool[expr_idx] {
            Ast::Integer(_) => Ok(()),
            Ast::ParamRef(_) => Ok(()),
            Ast::PrimitiveFunc(_) => Ok(()),
            Ast::UserFunc(name_idx) => {
                let func_name = self.ast_pool.get_string(name_idx).to_string();

                if !self.function_param_counts.contains_key(&name_idx) {
                    return Err(CheckerError::UndefinedFunction(func_name));
                }

                Ok(())
            }

            Ast::Call {
                func_idx,
                child_count,
            } => {
                self.check_expression(func_idx)?;

                match self.ast_pool[func_idx] {
                    Ast::PrimitiveFunc(func) => match func {
                        PrimitiveFunc::Add | PrimitiveFunc::Multiply => {
                            if child_count != 2 {
                                let func_name = match func {
                                    PrimitiveFunc::Add => "add",
                                    PrimitiveFunc::Multiply => "multiply",
                                };

                                return Err(CheckerError::InvalidPrimitiveArgCount(
                                    func_name.to_string(),
                                    child_count,
                                ));
                            }
                        }
                    },

                    Ast::UserFunc(name_idx) => {
                        let func_name = self.ast_pool.get_string(name_idx).to_string();

                        if let Some(&expected_count) = self.function_param_counts.get(&name_idx) {
                            if expected_count != child_count {
                                return Err(CheckerError::ArgumentCountMismatch {
                                    name: func_name,
                                    expected: expected_count,
                                    actual: child_count,
                                });
                            }
                        } else {
                            return Err(CheckerError::UndefinedFunction(func_name));
                        }
                    }

                    _ => {
                        return Err(CheckerError::InternalError(
                            "Cannot call non-function expression".to_string(),
                        ));
                    }
                }

                // Check all the children (arguments)
                if let Some(children) = self.ast_pool.children(expr_idx) {
                    for child_idx in children {
                        self.check_expression(child_idx)?;
                    }
                }

                Ok(())
            }

            Ast::FunctionDef { body_idx, .. } => self.check_expression(body_idx),
        }
    }
}
