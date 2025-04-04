use crate::ast::indices::{AstIdx, NameIdx, ParamIdx};
use crate::ast::primitives::PrimitiveFunc;
use std::collections::{HashMap, HashSet};
use std::ops::{Index, IndexMut};
use std::path::Path;

use super::Ast;

#[derive(Debug)]
pub struct AstPool {
    pub nodes: Vec<Ast>,

    string_pool: Vec<String>,
    string_map: HashMap<String, NameIdx>,
    pub function_defs: HashMap<NameIdx, AstIdx>,
}

impl AstPool {
    pub fn find_dependencies(&self, function_name: &str) -> HashSet<NameIdx> {
        let mut dependencies = HashSet::new();
        let mut visited = HashSet::new();

        if let Some(name_idx) = self.get_name_idx_from_func(function_name) {
            if let Some(&ast_idx) = self.function_defs.get(&name_idx) {
                self.find_dependencies_recursive(ast_idx, &mut dependencies, &mut visited);
            }
        }

        dependencies
    }

    fn find_dependencies_recursive(
        &self,
        node_idx: AstIdx,
        dependencies: &mut HashSet<NameIdx>,
        visited: &mut HashSet<AstIdx>,
    ) {
        if visited.contains(&node_idx) {
            return;
        }
        visited.insert(node_idx);

        match self[node_idx] {
            Ast::UserFunc(name_idx) => {
                if let Some(&func_ast_idx) = self.function_defs.get(&name_idx) {
                    if dependencies.insert(name_idx) {
                        self.find_dependencies_recursive(func_ast_idx, dependencies, visited);
                    }
                }
            }
            Ast::Call { func_idx, .. } => {
                self.find_dependencies_recursive(func_idx, dependencies, visited);
            }
            _ => {}
        }

        if let Some(children) = self.children(node_idx) {
            for child_idx in children {
                self.find_dependencies_recursive(child_idx, dependencies, visited);
            }
        }
    }

    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            string_pool: Vec::new(),
            string_map: HashMap::new(),
            function_defs: HashMap::new(),
        }
    }

    pub fn intern_string(&mut self, s: &str) -> NameIdx {
        if let Some(&idx) = self.string_map.get(s) {
            return idx;
        }

        let idx = NameIdx(self.string_pool.len());
        self.string_pool.push(s.to_string());
        self.string_map.insert(s.to_string(), idx);
        idx
    }

    pub fn get_string(&self, idx: NameIdx) -> &str {
        unsafe { self.string_pool.get_unchecked(idx.0) }
    }

    pub fn add_integer(&mut self, value: i64) -> AstIdx {
        let node_idx = AstIdx(self.nodes.len());
        self.nodes.push(Ast::Integer(value));
        node_idx
    }

    pub fn add_param_ref(&mut self, name: NameIdx, level: usize, offset: usize) -> AstIdx {
        let node_idx = AstIdx(self.nodes.len());
        self.nodes.push(Ast::ParamRef {
            name,
            level,
            offset: ParamIdx(offset),
        });
        node_idx
    }
    pub fn add_lambda(&mut self, param_count: usize, body_idx: AstIdx) -> AstIdx {
        let node_idx = AstIdx(self.nodes.len());
        self.nodes.push(Ast::Lambda {
            param_count,
            body_idx,
        });
        node_idx
    }
    pub fn add_primitive_func(&mut self, func: PrimitiveFunc) -> AstIdx {
        let node_idx = AstIdx(self.nodes.len());
        self.nodes.push(Ast::PrimitiveFunc(func));
        node_idx
    }

    pub fn add_user_func(&mut self, name: &str) -> AstIdx {
        let name_idx = self.intern_string(name);
        let node_idx = AstIdx(self.nodes.len());
        self.nodes.push(Ast::UserFunc(name_idx));
        node_idx
    }

    pub fn add_call(
        &mut self,
        func_idx: AstIdx,
        child_start: AstIdx,
        child_count: usize,
        len: usize,
    ) -> AstIdx {
        let node_idx = AstIdx(self.nodes.len());
        self.nodes.push(Ast::Call {
            func_idx,
            child_count,
            child_start,
            len,
        });
        node_idx
    }

    pub fn add_add(&mut self, child_start: AstIdx, len: usize) -> AstIdx {
        let id = self.add_primitive_func(PrimitiveFunc::Add);
        self.add_call(id, child_start, 2, len)
    }

    pub fn add_multiply(&mut self, child_start: AstIdx, len: usize) -> AstIdx {
        let id = self.add_primitive_func(PrimitiveFunc::Multiply);
        self.add_call(id, child_start, 2, len)
    }

    pub fn add_function_def(&mut self, name: &str, param_count: usize, body_idx: AstIdx) -> AstIdx {
        let name_idx = self.intern_string(name);
        let node_idx = AstIdx(self.nodes.len());
        self.nodes.push(Ast::FunctionDef {
            name_idx,
            param_count,
            body_idx,
        });

        self.function_defs.insert(name_idx, node_idx);

        node_idx
    }

    pub fn get_primitive_func(&self, name: &str) -> Option<PrimitiveFunc> {
        match name {
            "add" => Some(PrimitiveFunc::Add),
            "multiply" => Some(PrimitiveFunc::Multiply),
            _ => None,
        }
    }

    pub fn len(&self, idx: AstIdx) -> usize {
        match self[idx] {
            Ast::PrimitiveFunc(_) | Ast::UserFunc(_) | Ast::Integer(_) | Ast::ParamRef { .. } => 1,
            Ast::Call { len, .. } => len + 1,
            Ast::Lambda { .. } | Ast::FunctionDef { .. } => {
                1 + if let Some(children) = self.children(idx) {
                    children.iter().map(|&child| self.len(child)).sum()
                } else {
                    0
                }
            }
        }
    }

    pub fn children(&self, idx: AstIdx) -> Option<Vec<AstIdx>> {
        //println!("\tChildren of {idx:?}");

        match self[idx] {
            Ast::UserFunc(_) | Ast::PrimitiveFunc(_) | Ast::Integer(_) | Ast::ParamRef { .. } => {
                None
            }

            Ast::Call {
                child_count,
                child_start,
                ..
            } => {
                if child_count > 0 {
                    let mut children = Vec::with_capacity(child_count);
                    let mut current_idx = child_start.0;

                    for _ in 0..child_count {
                        children.push(current_idx.into());
                        let my_len = self.len(current_idx.into());
                        if my_len > current_idx {
                            println!("Is this fishy? {my_len:?} , {current_idx}");
                            break;
                        }
                        current_idx -= my_len;
                    }
                    children.reverse();
                    Some(children)
                } else {
                    None
                }
            }

            Ast::FunctionDef { body_idx, .. } | Ast::Lambda { body_idx, .. } => {
                Some(vec![body_idx])
            }
        }
    }

    pub fn add_function_call(
        &mut self,
        name: &str,
        child_start: AstIdx,
        child_count: usize,
        len: usize,
    ) -> AstIdx {
        if let Some(prim_func) = self.get_primitive_func(name) {
            let func_idx = self.add_primitive_func(prim_func);
            self.add_call(func_idx, child_start, child_count, len)
        } else {
            let func_idx = self.add_user_func(name);
            self.add_call(func_idx, child_start, child_count, len)
        }
    }

    pub fn add_lambda_call(
        &mut self,
        name: AstIdx,
        child_start: AstIdx,
        child_count: usize,

        len: usize,
    ) -> AstIdx {
        self.add_call(name, child_start, child_count, len)
    }

    pub fn display(&self) {
        for (i, node) in self.nodes.iter().enumerate() {
            match node {
                Ast::Integer(val) => {
                    println!("{}: Integer({})", i, val)
                }
                Ast::ParamRef {
                    name,
                    level,
                    offset,
                } => {
                    let name = self.get_string(*name);
                    println!(
                        "{}: ParamRef{{name: {} level: {} offset: {}}}",
                        i, name, level, offset.0
                    )
                }
                Ast::PrimitiveFunc(func) => {
                    let func_name = match func {
                        PrimitiveFunc::Add => "add",
                        PrimitiveFunc::Multiply => "multiply",
                    };
                    println!(
                        "{}: PrimitiveFunction {{ func: {:?} ({}) }}",
                        i, func, func_name
                    )
                }
                Ast::UserFunc(name_idx) => {
                    let name = self.get_string(*name_idx);
                    println!(
                        "{}: UserFunctionCall {{ name_idx: {} ({}) }}",
                        i, name_idx.0, name
                    )
                }

                Ast::Call {
                    func_idx,
                    child_count,
                    child_start,
                    len,
                } => {
                    println!(
                        "{}: Call {{ func_idx: {}, child_count: {}, child_start: {}, len: {} }}",
                        i, func_idx.0, child_count, child_start.0, len
                    )
                }
                Ast::FunctionDef {
                    name_idx,
                    param_count,
                    body_idx,
                } => {
                    let name = self.get_string(*name_idx);
                    println!(
                        "{}: FunctionDef {{ name_idx: {} ({}), param_count: {}, body_idx: {} }}",
                        i, name_idx.0, name, param_count, body_idx.0
                    )
                }
                Ast::Lambda {
                    param_count,
                    body_idx,
                } => {
                    println!(
                        "{}: Lambda {{ param_count: {}, body_idx: {} }}",
                        i, param_count, body_idx.0
                    )
                }
            }
        }

        println!("\nString Pool:");
        for (i, s) in self.string_pool.iter().enumerate() {
            println!("{}: {}", i, s);
        }

        println!("\nFunction Definitions:");
        for (&name_idx, &node_idx) in &self.function_defs {
            let name = self.get_string(name_idx);
            println!("{} ({}) -> node {}", name, name_idx.0, node_idx.0);
        }
    }

    pub fn get_name_idx_from_func(&self, func_name: &str) -> Option<NameIdx> {
        self.string_map.get(func_name).copied()
    }

    pub fn import_file<P: AsRef<Path>>(&mut self, file_path: P) -> Result<(), String> {
        todo!("not impl yet")
    }
}

impl Index<AstIdx> for AstPool {
    type Output = Ast;

    fn index(&self, index: AstIdx) -> &Self::Output {
        unsafe { self.nodes.get_unchecked(index.0) }
    }
}

impl IndexMut<AstIdx> for AstPool {
    fn index_mut(&mut self, index: AstIdx) -> &mut Self::Output {
        unsafe { self.nodes.get_unchecked_mut(index.0) }
    }
}

// Implement Index for string pool access
impl Index<NameIdx> for AstPool {
    type Output = String;

    fn index(&self, index: NameIdx) -> &Self::Output {
        unsafe { self.string_pool.get_unchecked(index.0) }
    }
}
