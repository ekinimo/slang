use std::collections::{HashMap, HashSet};
use std::ops::Sub;
use std::rc::Rc;

use crate::ast::indices::{AstIdx, FunIdx, NameIdx};
use crate::ast::pool::AstPool;
use crate::ast::Ast;
use crate::compiler::function::CompiledFunction;
use crate::value::Value;

use super::function::ErrTrace;
pub struct CompiledFunctions {
    functions: Vec<CompiledFunction>,
    function_defs: HashMap<NameIdx, FunIdx>,
}

#[derive(Debug, Clone)]
pub struct CompilationContext {
    stack_size: usize,
    frames: Vec<usize>,
    arg_count: Vec<usize>,
}

impl CompilationContext {
    pub fn new() -> Self {
        Self {
            stack_size: 0,
            frames: vec![0],
            arg_count: vec![0],
        }
    }

    pub fn alloc(&mut self, n: usize) {
        self.stack_size += n
    }

    pub fn dealloc(&mut self, n: usize) {
        self.stack_size.sub(n);
    }

    fn stack_depth(&self) -> usize {
        self.stack_size
    }

    fn calculate_param_offset(&self, level: usize, offset: usize) -> usize {
        let ret = if level == self.frames.len() {
            self.stack_size - (self.frames[level - 1] + offset)
        } else {
            self.stack_size
                - (self.frames.last().unwrap() + self.arg_count.last().unwrap() + offset)
        };
        ret
    }

    fn enter_scope(&mut self, param_count: usize) {
        let s = self.stack_size;
        self.frames.push(s);
        self.arg_count.push(param_count);
        self.stack_size += param_count;
    }

    fn exit_scope(&mut self) {
        let last = self.frames.pop().unwrap();
        self.stack_size = last;
    }
}
impl CompiledFunctions {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            function_defs: HashMap::new(),
        }
    }

    fn find_captured_vars(
        &self,
        node: AstIdx,
        pool: &AstPool,
        lambda_level: usize,
    ) -> Vec<(usize, usize)> {
        let mut captured = HashSet::new();
        self.collect_captured_vars(node, pool, lambda_level, &mut captured);

        // Convert to a Vec and sort by level and offset for consistent ordering
        let mut captured_vars: Vec<_> = captured.into_iter().collect();
        captured_vars.sort_by(|a, b| {
            // Sort by level first (ascending)
            match a.0.cmp(&b.0) {
                std::cmp::Ordering::Equal => a.1.cmp(&b.1), // If levels equal, sort by offset
                other => other,
            }
        });

        captured_vars
    }

    fn collect_captured_vars(
        &self,
        node: AstIdx,
        pool: &AstPool,
        lambda_level: usize,
        captured: &mut HashSet<(usize, usize)>,
    ) {
        match &pool[node] {
            Ast::ParamRef { offset, level, .. } => {
                if *level < lambda_level && *level > 0 {
                    captured.insert((*level, offset.0));
                }
            }

            Ast::Lambda { body_idx, .. } => {
                self.collect_captured_vars(*body_idx, pool, lambda_level + 1, captured);
            }

            _ => {
                if let Some(children) = pool.children(node) {
                    for &child in &children {
                        self.collect_captured_vars(child, pool, lambda_level, captured);
                    }
                }
            }
        }
    }

    pub fn debug_captured_vars(&self, node: AstIdx, pool: &AstPool) -> String {
        let captures = self.find_captured_vars(node, pool, 1);
        let mut result = String::new();

        result.push_str(&format!("Found {} captured variables:\n", captures.len()));

        for (i, (level, offset)) in captures.iter().enumerate() {
            let name = if let Ast::Lambda { body_idx, .. } = pool[node] {
                if let Ast::ParamRef { name, .. } = pool[body_idx] {
                    pool.get_string(name).to_string()
                } else {
                    format!("var_{}", i)
                }
            } else {
                format!("var_{}", i)
            };

            result.push_str(&format!(
                "  {}: level={}, offset={} ({})\n",
                i, level, offset, name
            ));
        }

        result
    }

    pub fn compile_expr<'a, 'b>(
        &'a self,
        node: AstIdx,
        pool: &'b AstPool,
        context: &mut CompilationContext,
    ) -> Option<CompiledFunction>
    where
        'b: 'a,
    {
        let expr = &pool[node];
        match *expr {
            Ast::Integer(i) => compile_integer(context, i),
            Ast::ParamRef { offset, level, .. } => compile_param(context, offset, level),
            Ast::PrimitiveFunc(primitive_func) => compile_primitive_func(context, primitive_func),
            Ast::UserFunc(name_idx) => self.compile_user_func(context, name_idx),
            Ast::Lambda {
                param_count,
                body_idx,
            } => self.compile_lambda(node, pool, context, param_count, body_idx),

            Ast::Call {
                func_idx,
                child_count,
                child_start,
                len,
            } => self.compile_call(node, pool, context, func_idx),
            Ast::FunctionDef {
                body_idx,
                param_count,
                ..
            } => self.compile_fun_def(pool, context, body_idx, param_count),
        }
    }

    fn compile_call<'a>(
        &'a self,
        node: AstIdx,
        pool: &AstPool,
        context: &mut CompilationContext,
        func_idx: AstIdx,
    ) -> Option<CompiledFunction> {
        let children = pool.children(node)?;
        let mut child_lambdas = Vec::with_capacity(children.len());
        // context.alloc(1);
        let func = self.compile_expr(func_idx, pool, context)?;

        for &child_idx in children.iter() {
            //context.alloc(1);
            let child_lambda = self.compile_expr(child_idx, pool, context)?;
            child_lambdas.push(child_lambda);
        }
        context.dealloc(children.len() + 1);

        Some(CompiledFunction::new(
            move |mem: &mut Vec<Value>| {
                let start_len = mem.len();
                for (i, lambda) in child_lambdas.iter().rev().enumerate() {
                    lambda.call(mem)?;
                }
                func.call(mem)?;
                let func_val = mem.pop().ok_or(ErrTrace::new("stack underflow"))?;

                match func_val {
                    Value::Fun(fun) => {
                        let expected_args = fun.param_count;
                        if mem.len() < expected_args {
                            return Err(ErrTrace::new(format!(
                                "not enough arguments for function call: expected {}, got {}",
                                expected_args,
                                mem.len()
                            )));
                        }

                        fun.call(mem)?;
                    }
                    _ => {
                        return Err(ErrTrace::new(
                            "expected function or lambda but got something else",
                        ));
                    }
                };

                let result = mem.pop().ok_or(ErrTrace::new("stack underflow"))?;
                mem.truncate(start_len);
                mem.push(result);
                Ok(())
            },
            0,
        ))
    }

    fn compile_lambda<'a>(
        &'a self,
        node: AstIdx,
        pool: &AstPool,
        context: &mut CompilationContext,
        param_count: usize,
        body_idx: AstIdx,
    ) -> Option<CompiledFunction> {
        //let initial_stack_depth = context.stack_depth();

        context.enter_scope(param_count);
        let curr_lev = context.frames.len();
        let captured_vars: Rc<[_]> = self
            .find_captured_vars(node, pool, curr_lev)
            .into_iter()
            .map(|(level, offset)| {
                let id = context.calculate_param_offset(level, offset);
                id
            })
            .collect();

        let body_func = self.compile_expr(body_idx, pool, context)?;
        context.exit_scope();

        context.alloc(1);

        Some(CompiledFunction::new(
            move |mem: &mut Vec<Value>| {
                let len = mem.len() - 1;
                let captures: Box<[_]> = captured_vars
                    .into_iter()
                    .map(|i| mem[len - *i].clone())
                    .collect();

                println!("Lambda @ CAPTURES: {:?}", &captures);

                let body_func = body_func.clone();
                let clos = CompiledFunction::new(
                    move |mem: &mut Vec<Value>| {
                        let start_len = mem.len();
                        mem.extend(captures.clone());
                        body_func.clone().call(mem)?;
                        let result = mem.pop().ok_or(ErrTrace::new("stack underflow"))?;
                        mem.truncate(start_len);
                        mem.push(result);

                        println!("Lambda @ END: {mem:?}");
                        Ok(())
                    },
                    param_count,
                );
                mem.push(Value::Fun(clos));
                Ok(())
            },
            0,
        ))
    }

    fn compile_fun_def<'a>(
        &'a self,
        pool: &AstPool,
        context: &mut CompilationContext,
        body_idx: AstIdx,
        param_count: usize,
    ) -> Option<CompiledFunction> {
        context.enter_scope(param_count);

        let body_lambda = self.compile_expr(body_idx, pool, context)?;
        context.exit_scope();

        Some(CompiledFunction::new(
            move |mem: &mut Vec<Value>| body_lambda.call(mem),
            param_count,
        ))
    }

    fn compile_user_func<'a>(
        &'a self,
        context: &mut CompilationContext,
        name_idx: NameIdx,
    ) -> Option<CompiledFunction> {
        if let Some(&lambda_idx) = self.function_defs.get(&name_idx) {
            let func_to_call = self.functions[lambda_idx.0].clone();
            context.alloc(1);
            Some(CompiledFunction::new(
                move |mem: &mut Vec<Value>| {
                    mem.push(Value::Fun(func_to_call.clone()));
                    Ok(())
                },
                0,
            ))
        } else {
            None
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
                |mem: &mut Vec<Value>| {
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
                let mut ctx = CompilationContext::new();
                if let Some(compiled_body) = self.compile_expr(body_idx, pool, &mut ctx) {
                    let func_idx = lambda_idx.0;
                    *self.functions[func_idx].inner.borrow_mut() =
                        Box::new(move |mem: &mut Vec<Value>| compiled_body.call(mem));
                } else {
                    eprintln!(
                        "Failed to compile function body for {}",
                        pool.get_string(name_idx)
                    );
                }
            }
        }
    }

    fn debug_print_ast(&self, node_idx: AstIdx, pool: &AstPool, indent: usize) {
        let indent_str = " ".repeat(indent * 2);
        println!("{}Node {:?}: {:?}", indent_str, node_idx, &pool[node_idx]);
        if let Some(children) = pool.children(node_idx) {
            for &child_idx in children.iter() {
                self.debug_print_ast(child_idx, pool, indent + 1);
            }
        }
    }
    pub fn execute(&self, expr_idx: AstIdx, pool: &AstPool) -> Option<Value> {
        println!("Full AST structure:");
        self.debug_print_ast(expr_idx, pool, 0);

        let mut ctx = CompilationContext::new();
        if let Some(compiled_expr) = self.compile_expr(expr_idx, pool, &mut ctx) {
            let mut memory = Vec::new();
            if let Err(e) = compiled_expr.call(&mut memory) {
                eprintln!("Error during execution: {:?}", e);
                return None;
            }
            memory.pop()
        } else {
            None
        }
    }
}

fn compile_integer(context: &mut CompilationContext, i: i64) -> Option<CompiledFunction> {
    context.alloc(1);

    Some(CompiledFunction::new(
        move |mem: &mut Vec<Value>| {
            mem.push(Value::Int(i));
            Ok(())
        },
        0,
    ))
}

fn compile_param(
    context: &mut CompilationContext,
    offset: crate::ParamIdx,
    level: usize,
) -> Option<CompiledFunction> {
    let param_index = context.calculate_param_offset(level, offset.0);
    context.alloc(1);

    Some(CompiledFunction::new(
        move |mem: &mut Vec<Value>| {
            println!("Param acces");
            println!("            level  : {level}");
            println!("            offset : {})", offset.0);
            println!("            imdex  : {param_index}");
            println!("            mem    : ");
            println!("            {mem:?}");

            let l = mem.len() - 1;
            if param_index >= mem.len() {
                return Err(ErrTrace::new(format!(
                    "parameter access out of bounds: index {} but memory size {}",
                    param_index,
                    mem.len()
                )));
            }

            let val = mem[l - param_index].clone();
            mem.push(val);
            Ok(())
        },
        0,
    ))
}

fn compile_primitive_func(
    context: &mut CompilationContext,
    primitive_func: crate::ast::PrimitiveFunc,
) -> Option<CompiledFunction> {
    context.alloc(1);
    match primitive_func {
        crate::ast::PrimitiveFunc::Add => Some(CompiledFunction::new(
            move |mem: &mut Vec<Value>| {
                let fun = CompiledFunction::new(
                    move |mem: &mut Vec<Value>| {
                        let b = mem.pop().unwrap();
                        let a = mem.pop().unwrap();
                        if let (Value::Int(a_val), Value::Int(b_val)) = (a, b) {
                            mem.push(Value::Int(a_val + b_val));
                            Ok(())
                        } else {
                            Err(ErrTrace::new("Wrong Argument type for `add` "))
                        }
                    },
                    2,
                );

                mem.push(Value::Fun(fun));
                Ok(())
            },
            2,
        )),
        crate::ast::PrimitiveFunc::Multiply => Some(CompiledFunction::new(
            move |mem: &mut Vec<Value>| {
                let fun = CompiledFunction::new(
                    move |mem: &mut Vec<Value>| {
                        let b = mem.pop().unwrap();
                        let a = mem.pop().unwrap();
                        if let (Value::Int(a_val), Value::Int(b_val)) = (a, b) {
                            mem.push(Value::Int(a_val * b_val));
                            Ok(())
                        } else {
                            Err(ErrTrace::new("Wrong Argument type for `mul` "))
                        }
                    },
                    2,
                );

                mem.push(Value::Fun(fun));
                Ok(())
            },
            2,
        )),
    }
}
