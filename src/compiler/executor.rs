use std::collections::HashMap;

use crate::ast::indices::{AstIdx, FunIdx, NameIdx};
use crate::ast::pool::AstPool;
use crate::ast::Ast;
use crate::compiler::function::CompiledFunction;
use crate::value::Value;
/// CompiledFunctions manages the compilation and execution of AST into executable code
pub struct CompiledFunctions {
    /// Vector of compiled functions
    functions: Vec<CompiledFunction>,

    /// Mapping from function names (as NameIdx) to compiled function indices
    function_defs: HashMap<NameIdx, FunIdx>,
}

impl CompiledFunctions {
    /// Create a new compiler instance
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            function_defs: HashMap::new(),
        }
    }

    /// Compile an AST expression into an executable function
    pub fn compile_expr<'a, 'b>(
        &'a self,
        node: AstIdx,
        pool: &'b AstPool,
    ) -> Option<CompiledFunction>
    where
        'b: 'a,
    {
        let expr = &pool[node];
        match *expr {
            Ast::Integer(i) => Some(CompiledFunction::new(
                move |mem: &mut Vec<Value>, _param_base: usize| {
                    mem.push(Value::Int(i));
                },
                0,
            )),

            Ast::ParamRef(i) => Some(CompiledFunction::new(
                move |mem: &mut Vec<Value>, param_base: usize| {
                    let val = mem[param_base + i.0];
                    mem.push(val);
                },
                0,
            )),

            Ast::PrimitiveFunctionCall { func, .. } => {
                let children = pool.children(node)?;
                let mut child_lambdas = Vec::with_capacity(children.len());

                for &child_idx in &children {
                    let child_lambda = self.compile_expr(child_idx, pool)?;
                    child_lambdas.push(child_lambda);
                }

                match func {
                    crate::ast::primitives::PrimitiveFunc::Add => Some(CompiledFunction::new(
                        move |mem: &mut Vec<Value>, param_base: usize| {
                            for lambda in &child_lambdas {
                                lambda.call(mem, param_base);
                            }

                            let b = mem.pop().unwrap();
                            let a = mem.pop().unwrap();

                            if let (Value::Int(a_val), Value::Int(b_val)) = (a, b) {
                                mem.push(Value::Int(a_val + b_val));
                            }
                        },
                        0,
                    )),

                    crate::ast::primitives::PrimitiveFunc::Multiply => Some(CompiledFunction::new(
                        move |mem: &mut Vec<Value>, param_base: usize| {
                            for lambda in &child_lambdas {
                                lambda.call(mem, param_base);
                            }

                            let b = mem.pop().unwrap();
                            let a = mem.pop().unwrap();

                            if let (Value::Int(a_val), Value::Int(b_val)) = (a, b) {
                                mem.push(Value::Int(a_val * b_val));
                            }
                        },
                        0,
                    )),
                }
            }

            Ast::UserFunctionCall {
                name_idx,
                child_count,
                ..
            } => {
                if let Some(&lambda_idx) = self.function_defs.get(&name_idx) {
                    let children = pool.children(node)?;
                    let mut child_lambdas = Vec::with_capacity(children.len());

                    for &child_idx in &children {
                        let child_lambda = self.compile_expr(child_idx, pool)?;
                        child_lambdas.push(child_lambda);
                    }

                    let func_to_call = self.functions[lambda_idx.0].clone();

                    Some(CompiledFunction::new(
                        move |mem: &mut Vec<Value>, parent_param_base: usize| {
                            let base_len = mem.len();
                            let param_base = base_len;

                            for lambda in child_lambdas.iter() {
                                lambda.call(mem, parent_param_base);
                            }
                            func_to_call.call(mem, param_base);
                            let result = mem.pop().unwrap();
                            while mem.len() > base_len {
                                mem.pop();
                            }
                            mem.push(result);
                        },
                        0,
                    ))
                } else {
                    None
                }
            }

            Ast::FunctionDef {
                body_idx,
                param_count,
                ..
            } => {
                let body_lambda = self.compile_expr(body_idx, pool)?;

                Some(CompiledFunction::new(
                    move |mem: &mut Vec<Value>, param_base: usize| {
                        body_lambda.call(mem, param_base);
                    },
                    param_count,
                ))
            }
        }
    }

    /// Compile all functions in the AST pool
    pub fn compile(&mut self, pool: &AstPool) {
        // First pass - create function placeholders
        for (&name_idx, &ast_idx) in &pool.function_defs {
            let lambda_idx = FunIdx(self.functions.len());
            self.function_defs.insert(name_idx, lambda_idx);

            // Get param count from the function definition
            let param_count = if let Ast::FunctionDef { param_count, .. } = pool[ast_idx] {
                param_count
            } else {
                0
            };

            // Add a placeholder function
            self.functions.push(CompiledFunction::new(
                |mem: &mut Vec<Value>, _param_base: usize| {
                    panic!("Function is not implemented yet");
                },
                param_count,
            ));
        }

        // Second pass - compile all function bodies
        for (&name_idx, &ast_idx) in &pool.function_defs {
            let lambda_idx = self.function_defs[&name_idx];

            if let Ast::FunctionDef {
                body_idx,
                param_count,
                ..
            } = pool[ast_idx]
            {
                if let Some(compiled_body) = self.compile_expr(body_idx, pool) {
                    // We need to clone the function first, then update the inner implementation
                    let func_idx = lambda_idx.0;

                    // Now replace the placeholder function with our new implementation
                    *self.functions[func_idx].inner.borrow_mut() =
                        Box::new(move |mem: &mut Vec<Value>, param_base: usize| {
                            compiled_body.call(mem, param_base);
                        });
                } else {
                    eprintln!(
                        "Failed to compile function body for {}",
                        pool.get_string(name_idx)
                    );
                }
            }
        }
    }

    /// Execute a compiled expression and return its result
    pub fn execute(&self, expr_idx: AstIdx, pool: &AstPool) -> Option<Value> {
        if let Some(compiled_expr) = self.compile_expr(expr_idx, pool) {
            let mut memory = Vec::new();
            compiled_expr.call(&mut memory, 0);
            memory.pop()
        } else {
            None
        }
    }
}
