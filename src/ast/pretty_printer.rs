use std::collections::HashMap;
use std::fmt;

use crate::ast::indices::{AstIdx, NameIdx, ParamIdx};
use crate::ast::pool::AstPool;
use crate::ast::primitives::PrimitiveFunc;

use super::Ast;

#[derive(Debug, Clone)]
pub struct PrintConfig {
    pub indent_is_tab: bool,
    pub indent_size: usize,
    pub spaces_around_operators: bool,
    pub newlines_after_functions: bool,
    pub max_line_length: usize,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            indent_is_tab: true,
            indent_size: 2,
            spaces_around_operators: true,
            newlines_after_functions: true,
            max_line_length: 80,
        }
    }
}

pub struct PrettyPrinter<'a> {
    pool: &'a AstPool,
    config: PrintConfig,
    // Track parameter names by (function_idx, level, offset)
    param_names: HashMap<(NameIdx, usize, usize), String>,
    // Track the current function context during traversal
    current_function: Option<NameIdx>,
    current_level: usize,
}

impl<'a> PrettyPrinter<'a> {
    pub fn new(pool: &'a AstPool) -> Self {
        Self {
            pool,
            config: PrintConfig::default(),
            param_names: HashMap::new(),
            current_function: None,
            current_level: 0,
        }
    }

    pub fn with_config(pool: &'a AstPool, config: PrintConfig) -> Self {
        Self {
            pool,
            config,
            param_names: HashMap::new(),
            current_function: None,
            current_level: 0,
        }
    }

    pub fn print_node(&self, node_idx: AstIdx) -> String {
        let mut output = String::new();
        let mut printer = Self {
            pool: self.pool,
            config: self.config.clone(),
            param_names: HashMap::new(),
            current_function: None,
            current_level: 0,
        };

        // First pass - collect parameter names
        printer.collect_param_names(node_idx);

        // Second pass - print with parameter names
        printer.print_node_to_string(node_idx, 0, &mut output);
        output
    }

    pub fn print_all_functions(&self) -> String {
        let mut output = String::new();
        let mut printer = Self {
            pool: self.pool,
            config: self.config.clone(),
            param_names: HashMap::new(),
            current_function: None,
            current_level: 0,
        };

        // First pass - collect parameter names from all functions
        for (_, &node_idx) in &self.pool.function_defs {
            printer.collect_param_names(node_idx);
        }

        // Second pass - print all functions with collected parameter names
        for (&_name_idx, &node_idx) in &self.pool.function_defs {
            printer.print_node_to_string(node_idx, 0, &mut output);
            if self.config.newlines_after_functions {
                output.push_str("\n\n");
            } else {
                output.push_str("\n");
            }
        }

        output
    }

    // Collect parameter names from function and lambda definitions
    fn collect_param_names(&mut self, node_idx: AstIdx) {
        match self.pool[node_idx] {
            Ast::FunctionDef {
                name_idx,
                param_count,
                body_idx,
            } => {
                let prev_function = self.current_function;
                let prev_level = self.current_level;

                self.current_function = Some(name_idx);
                self.current_level = 0;

                // Add default parameter names for this function
                for i in 0..param_count {
                    let param_key = (name_idx, 0, i);
                    let param_name = format!("p{}", i);
                    self.param_names.insert(param_key, param_name);
                }

                // Continue traversing to collect parameters in nested lambdas
                self.collect_param_names(body_idx);

                // Restore previous context
                self.current_function = prev_function;
                self.current_level = prev_level;
            }
            Ast::Lambda {
                param_count,
                body_idx,
            } => {
                if let Some(func_idx) = self.current_function {
                    let new_level = self.current_level + 1;

                    // Add default parameter names for this lambda
                    for i in 0..param_count {
                        let param_key = (func_idx, new_level, i);
                        let param_name = format!("l{}p{}", new_level, i); // Lambda level + param index
                        self.param_names.insert(param_key, param_name);
                    }

                    // Traverse lambda body with increased level
                    let prev_level = self.current_level;
                    self.current_level = new_level;
                    self.collect_param_names(body_idx);
                    self.current_level = prev_level;
                }
            }
            Ast::Call { func_idx, .. } => {
                // Traverse function and arguments
                self.collect_param_names(func_idx);

                if let Some(children) = self.pool.children(node_idx) {
                    for child_idx in children {
                        self.collect_param_names(child_idx);
                    }
                }
            }
            // No need to handle other cases as they don't define parameters
            _ => {}
        }
    }

    fn get_param_name(&self, name_idx: NameIdx, level: usize, offset: usize) -> String {
        // Try to find the parameter name in our mapping
        if let Some(current_func) = self.current_function {
            let param_key = (current_func, level, offset);
            if let Some(name) = self.param_names.get(&param_key) {
                return name.clone();
            }
        }

        // Fallback to a generic parameter name based on level and offset
        if level == 0 {
            format!("p{}", offset)
        } else {
            format!("l{}p{}", level, offset)
        }
    }

    fn print_node_to_string(&mut self, node_idx: AstIdx, indent_level: usize, output: &mut String) {
        let indent = if self.config.indent_is_tab {
            "\t".repeat(indent_level)
        } else {
            " ".repeat(indent_level * self.config.indent_size)
        };

        match self.pool[node_idx] {
            Ast::Integer(val) => {
                output.push_str(&val.to_string());
            }

            Ast::ParamRef {
                name: _,
                level,
                offset,
            } => {
                if let Some(func_idx) = self.current_function {
                    // Use our parameter naming scheme based on function, level, and offset
                    let param_name = self.get_param_name(func_idx, level, offset.0);
                    output.push_str(&param_name);
                } else {
                    // Fallback if we don't have a function context
                    output.push_str(&format!("p{}", offset.0));
                }
            }

            Ast::PrimitiveFunc(func) => {
                output.push_str(match func {
                    PrimitiveFunc::Add => "add",
                    PrimitiveFunc::Multiply => "multiply",
                });
            }

            Ast::UserFunc(name_idx) => {
                let func_name = self.pool.get_string(name_idx);
                output.push_str(func_name);
            }

            Ast::Lambda {
                param_count,
                body_idx,
            } => {
                output.push_str("lambda ");

                // Add parameter names based on current context and level
                let new_level = self.current_level + 1;
                if let Some(func_idx) = self.current_function {
                    for i in 0..param_count {
                        if i > 0 {
                            output.push(' ');
                        }
                        let param_name = self.get_param_name(func_idx, new_level, i);
                        output.push_str(&param_name);
                    }
                } else {
                    // Fallback if we don't have a function context
                    for i in 0..param_count {
                        if i > 0 {
                            output.push(' ');
                        }
                        output.push_str(&format!("p{}", i));
                    }
                }

                output.push_str(" { ");

                // Update level for traversing the lambda body
                let prev_level = self.current_level;
                self.current_level = new_level;

                // Print the lambda body
                self.print_node_to_string(body_idx, indent_level + 1, output);

                // Restore previous level
                self.current_level = prev_level;

                output.push_str(" }");
            }

            Ast::Call { func_idx, .. } => {
                let children = self.pool.children(node_idx).unwrap_or_default();

                if let Ast::PrimitiveFunc(func) = self.pool[func_idx] {
                    {
                        if children.len() != 2 {
                            // For non-binary primitive calls, use function call syntax
                            output.push_str(match func {
                                PrimitiveFunc::Add => "add",
                                PrimitiveFunc::Multiply => "multiply",
                            });
                            output.push('(');

                            for (i, &child) in children.iter().enumerate() {
                                if i > 0 {
                                    output.push_str(", ");
                                }
                                self.print_node_to_string(child, indent_level, output);
                            }

                            output.push(')');
                        } else {
                            output.push('(');

                            self.print_node_to_string(children[0], indent_level, output);

                            output.push_str(match func {
                                PrimitiveFunc::Add => {
                                    if self.config.spaces_around_operators {
                                        " + "
                                    } else {
                                        "+"
                                    }
                                }
                                PrimitiveFunc::Multiply => {
                                    if self.config.spaces_around_operators {
                                        " * "
                                    } else {
                                        "*"
                                    }
                                }
                            });

                            self.print_node_to_string(children[1], indent_level, output);

                            output.push(')');
                        }
                    }
                } else {
                    self.print_node_to_string(func_idx, indent_level, output);
                    output.push('(');

                    for (i, &child) in children.iter().enumerate() {
                        if i > 0 {
                            output.push_str(", ");
                        }
                        self.print_node_to_string(child, indent_level, output);
                    }

                    output.push(')');
                }
            }

            Ast::FunctionDef {
                name_idx,
                param_count,
                body_idx,
            } => {
                // Store previous function context
                let prev_function = self.current_function;
                let prev_level = self.current_level;

                // Set current function context for parameter name lookup
                self.current_function = Some(name_idx);
                self.current_level = 0;

                // Function header
                output.push_str(&indent);
                output.push_str("fn ");
                output.push_str(self.pool.get_string(name_idx));
                output.push('(');

                // Parameters with meaningful names from our tracking
                for i in 0..param_count {
                    if i > 0 {
                        output.push_str(", ");
                    }
                    let param_name = self.get_param_name(name_idx, 0, i);
                    output.push_str(&param_name);
                }

                output.push_str(") {\n");

                // Function body
                let body_indent = indent_level + 1;
                output.push_str(&if self.config.indent_is_tab {
                    "\t".repeat(body_indent)
                } else {
                    " ".repeat(body_indent * self.config.indent_size)
                });

                // Print the body expression
                self.print_node_to_string(body_idx, body_indent, output);

                // Close the function
                output.push('\n');
                output.push_str(&indent);
                output.push('}');

                // Restore previous function context
                self.current_function = prev_function;
                self.current_level = prev_level;
            }
        }
    }
}

// Implement Display for convenience
impl<'a> fmt::Display for PrettyPrinter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.print_all_functions())
    }
}
