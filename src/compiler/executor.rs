use std::collections::HashMap;

use crate::ast::indices::{AstIdx, FunIdx, NameIdx};
use crate::ast::pool::AstPool;
use crate::ast::Ast;
use crate::compiler::function::CompiledFunction;
use crate::value::Value;

use super::function;
pub struct CompiledFunctions {
    functions: Vec<CompiledFunction>,
    function_defs: HashMap<NameIdx, FunIdx>,
}

impl CompiledFunctions {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            function_defs: HashMap::new(),
        }
    }

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
                    let val = mem[param_base + i.0].clone();
                    mem.push(val);
                },
                0,
            )),
            Ast::PrimitiveFunc(primitive_func) => match primitive_func {
                crate::ast::PrimitiveFunc::Add => Some(CompiledFunction::new(
                    move |mem: &mut Vec<Value>, _param_base: usize| {
                        let fun = CompiledFunction::new(
                            move |mem: &mut Vec<Value>, _param_base: usize| {
                                let b = mem.pop().unwrap();
                                let a = mem.pop().unwrap();
                                if let (Value::Int(a_val), Value::Int(b_val)) = (a, b) {
                                    mem.push(Value::Int(a_val + b_val));
                                }
                            },
                            0,
                        );

                        mem.push(Value::Fun(fun));
                    },
                    0,
                )),
                crate::ast::PrimitiveFunc::Multiply => Some(CompiledFunction::new(
                    move |mem: &mut Vec<Value>, _param_base: usize| {
                        let fun = CompiledFunction::new(
                            move |mem: &mut Vec<Value>, _param_base: usize| {
                                let b = mem.pop().unwrap();
                                let a = mem.pop().unwrap();
                                if let (Value::Int(a_val), Value::Int(b_val)) = (a, b) {
                                    mem.push(Value::Int(a_val * b_val));
                                }
                            },
                            0,
                        );

                        mem.push(Value::Fun(fun));
                    },
                    0,
                )),
            },
            Ast::UserFunc(name_idx) => {
                if let Some(&lambda_idx) = self.function_defs.get(&name_idx) {
                    let func_to_call = self.functions[lambda_idx.0].clone();
                    Some(CompiledFunction::new(
                        move |mem: &mut Vec<Value>, _parent_param_base: usize| {
                            mem.push(Value::Fun(func_to_call.clone()));
                        },
                        0,
                    ))
                } else {
                    None
                }
            }
            Ast::Call {
                func_idx,
                child_count,
            } => {
                let children = pool.children(node)?;
                let mut child_lambdas = Vec::with_capacity(children.len());

                for &child_idx in &children {
                    let child_lambda = self.compile_expr(child_idx, pool)?;
                    child_lambdas.push(child_lambda);
                }
                let func = self.compile_expr(func_idx, pool)?;
                Some(CompiledFunction::new(
                    move |mem: &mut Vec<Value>, parent_param_base: usize| {
                        let base_len = mem.len();
                        let param_base = base_len;

                        for lambda in child_lambdas.iter() {
                            lambda.call(mem, parent_param_base);
                        }
                        func.call(mem, param_base);
                        let result = mem.pop().unwrap();

                        match result {
                            Value::Fun(fun) => fun.call(mem, param_base),
                            _ => panic!("This shouldnt happen"),
                        };
                        let result = mem.pop().unwrap();

                        while mem.len() > base_len {
                            mem.pop();
                        }
                        mem.push(result);
                    },
                    0,
                ))
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

    pub fn compile(&mut self, pool: &AstPool) {
        for (&name_idx, &ast_idx) in &pool.function_defs {
            let lambda_idx = FunIdx(self.functions.len());
            self.function_defs.insert(name_idx, lambda_idx);
            let param_count = if let Ast::FunctionDef { param_count, .. } = pool[ast_idx] {
                param_count
            } else {
                0
            };
            self.functions.push(CompiledFunction::new(
                |mem: &mut Vec<Value>, _param_base: usize| {
                    panic!("Function is not implemented yet");
                },
                param_count,
            ));
        }

        for (&name_idx, &ast_idx) in &pool.function_defs {
            let lambda_idx = self.function_defs[&name_idx];

            if let Ast::FunctionDef {
                body_idx,
                param_count,
                ..
            } = pool[ast_idx]
            {
                if let Some(compiled_body) = self.compile_expr(body_idx, pool) {
                    let func_idx = lambda_idx.0;
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
