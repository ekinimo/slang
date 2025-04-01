use crate::ast::indices::{AstIdx, NameIdx, ParamIdx};
use crate::ast::primitives::PrimitiveFunc;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, Write};
use std::ops::{Index, IndexMut};
use std::path::Path;

use super::Ast;

#[derive(Debug)]
pub struct AstPool {
    nodes: Vec<Ast>,
    string_pool: Vec<String>,
    string_map: HashMap<String, NameIdx>,
    pub function_defs: HashMap<NameIdx, AstIdx>,
    parameter_names: HashMap<(NameIdx, ParamIdx), NameIdx>,
    current_function: RefCell<Option<(NameIdx, usize)>>,
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

    pub fn register_param_name(
        &mut self,
        func_name_idx: NameIdx,
        param_idx: ParamIdx,
        param_name: &str,
    ) {
        let param_name_idx = self.intern_string(param_name);
        self.parameter_names
            .insert((func_name_idx, param_idx), param_name_idx);
    }

    pub fn get_param_name(&self, func_name_idx: NameIdx, param_idx: ParamIdx) -> Option<&str> {
        self.parameter_names
            .get(&(func_name_idx, param_idx))
            .map(|&name_idx| self.get_string(name_idx))
    }

    pub fn set_function_context(&self, func_name_idx: NameIdx) {
        if let Some(&ast_idx) = self.function_defs.get(&func_name_idx) {
            if let Ast::FunctionDef { param_count, .. } = self[ast_idx] {
                *self.current_function.borrow_mut() = Some((func_name_idx, param_count));
            }
        }
    }

    /// Clear the current function context
    pub fn clear_function_context(&self) {
        *self.current_function.borrow_mut() = None;
    }

    /// Get the current function context
    pub fn current_function_context(&self) -> Option<(NameIdx, usize)> {
        *self.current_function.borrow()
    }

    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            string_pool: Vec::new(),
            string_map: HashMap::new(),
            function_defs: HashMap::new(),
            parameter_names: HashMap::new(),
            current_function: RefCell::new(None),
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

    pub fn add_param_ref(&mut self, param_idx: usize) -> AstIdx {
        let node_idx = AstIdx(self.nodes.len());
        self.nodes.push(Ast::ParamRef(ParamIdx(param_idx)));
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

    pub fn add_call(&mut self, func_idx: AstIdx, child_count: usize) -> AstIdx {
        let node_idx = AstIdx(self.nodes.len());
        self.nodes.push(Ast::Call {
            func_idx,
            child_count,
        });
        node_idx
    }

    pub fn add_add(&mut self, child_count: usize) -> AstIdx {
        let id = self.add_primitive_func(PrimitiveFunc::Add);
        self.add_call(id, 2)
    }

    pub fn add_multiply(&mut self, child_count: usize) -> AstIdx {
        let id = self.add_primitive_func(PrimitiveFunc::Multiply);
        self.add_call(id, 2)
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
            Ast::PrimitiveFunc(_) | Ast::UserFunc(_) | Ast::Integer(_) | Ast::ParamRef(_) => 1,
            Ast::Call { func_idx, .. } => {
                let mut total = 1 + self.len(func_idx);
                if let Some(children) = self.children(idx) {
                    for child_idx in children {
                        total += self.len(child_idx);
                    }
                }

                total
            }
            Ast::FunctionDef { .. } => {
                1 + if let Some(children) = self.children(idx) {
                    children.iter().map(|&child| self.len(child)).sum()
                } else {
                    0
                }
            }
        }
    }

    pub fn children(&self, idx: AstIdx) -> Option<Vec<AstIdx>> {
        match self[idx] {
            Ast::UserFunc(_) | Ast::PrimitiveFunc(_) | Ast::Integer(_) | Ast::ParamRef(_) => None,

            Ast::Call {
                func_idx,
                child_count,
            } => {
                let mut children = Vec::with_capacity(child_count);
                let next_len = self.len(func_idx);
                let mut current_idx = idx.0 - next_len;

                for _ in 0..child_count {
                    let next_child = AstIdx(current_idx - 1);
                    children.push(next_child);
                    current_idx -= self.len(next_child);
                }
                children.reverse();
                Some(children)
            }

            Ast::FunctionDef { body_idx, .. } => Some(vec![body_idx]),
        }
    }

    pub fn add_function_call(&mut self, name: &str, child_count: usize) -> AstIdx {
        if let Some(prim_func) = self.get_primitive_func(name) {
            let func_idx = self.add_primitive_func(prim_func);
            self.add_call(func_idx, child_count)
        } else {
            let func_idx = self.add_user_func(name);
            self.add_call(func_idx, child_count)
        }
    }

    pub fn display(&self) {
        for (i, node) in self.nodes.iter().enumerate() {
            match node {
                Ast::Integer(val) => {
                    println!("{}: Integer({})", i, val)
                }
                Ast::ParamRef(param_idx) => {
                    println!("{}: ParamRef({})", i, param_idx.0)
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
                } => {
                    println!(
                        "{}: Call {{ func_idx: {}, child_count: {} }}",
                        i, func_idx.0, child_count
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
        let path_str = file_path.as_ref().to_string_lossy().to_string();
        let file_stem = file_path
            .as_ref()
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("Invalid file path: {}", path_str))?;

        let content = std::fs::read_to_string(file_path.as_ref())
            .map_err(|e| format!("Failed to read file '{}': {}", path_str, e))?;

        let mut temp_pool = AstPool::new();
        let parse_result = crate::parser::parser::parse_program(&content, &mut temp_pool);

        match parse_result {
            Ok(_) => {
                let base_offset = self.nodes.len();
                let mut idx_mapping = HashMap::new();
                for i in 0..temp_pool.nodes.len() {
                    idx_mapping.insert(AstIdx(i), AstIdx(base_offset + i));
                }
                let mut name_idx_mapping = HashMap::new();
                for (i, name) in temp_pool.string_pool.iter().enumerate() {
                    let old_name_idx = NameIdx(i);
                    let is_function_name = temp_pool
                        .function_defs
                        .iter()
                        .any(|(&name_idx, _)| name_idx == old_name_idx);
                    let new_name = if is_function_name {
                        format!("{}::{}", file_stem, name)
                    } else {
                        name.clone()
                    };

                    let new_name_idx = self.intern_string(&new_name);
                    name_idx_mapping.insert(old_name_idx, new_name_idx);
                }
                self.nodes
                    .extend(temp_pool.nodes.iter().map(|node| match node {
                        Ast::FunctionDef {
                            name_idx,
                            param_count,
                            body_idx,
                        } => {
                            let new_body_idx = idx_mapping.get(body_idx).unwrap_or(body_idx);
                            let new_name_idx = name_idx_mapping.get(name_idx).unwrap_or(name_idx);
                            Ast::FunctionDef {
                                name_idx: *new_name_idx,
                                param_count: *param_count,
                                body_idx: *new_body_idx,
                            }
                        }
                        Ast::UserFunc(name_idx) => {
                            let func_name = temp_pool.get_string(*name_idx);
                            let is_local_function =
                                temp_pool.function_defs.iter().any(|(&fn_name_idx, _)| {
                                    temp_pool.get_string(fn_name_idx) == func_name
                                });
                            if is_local_function {
                                let new_name_idx =
                                    name_idx_mapping.get(name_idx).unwrap_or(name_idx);
                                Ast::UserFunc(*new_name_idx)
                            } else {
                                Ast::UserFunc(*name_idx)
                            }
                        }
                        _ => node.clone(),
                    }));

                for (&old_name_idx, &old_ast_idx) in &temp_pool.function_defs {
                    let new_name_idx = name_idx_mapping[&old_name_idx];
                    let new_ast_idx = idx_mapping[&old_ast_idx];
                    self.function_defs.insert(new_name_idx, new_ast_idx);

                    if let Ast::FunctionDef { param_count, .. } = temp_pool[old_ast_idx] {
                        for param_idx in 0..param_count {
                            let key = (old_name_idx, ParamIdx(param_idx));
                            if let Some(&param_name_idx) = temp_pool.parameter_names.get(&key) {
                                let new_param_name_idx =
                                    *name_idx_mapping.entry(param_name_idx).or_insert_with(|| {
                                        self.intern_string(temp_pool.get_string(param_name_idx))
                                    });

                                self.parameter_names.insert(
                                    (new_name_idx, ParamIdx(param_idx)),
                                    new_param_name_idx,
                                );
                            }
                        }
                    }
                }

                Ok(())
            }
            Err(e) => Err(format!("Parse error: {}", e)),
        }
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
