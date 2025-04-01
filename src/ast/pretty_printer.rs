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
}

impl<'a> PrettyPrinter<'a> {
    pub fn new(pool: &'a AstPool) -> Self {
        Self {
            pool,
            config: PrintConfig::default(),
        }
    }

    pub fn with_config(pool: &'a AstPool, config: PrintConfig) -> Self {
        Self { pool, config }
    }

    pub fn print_node(&self, node_idx: AstIdx) -> String {
        let mut output = String::new();
        self.print_node_to_string(node_idx, 0, &mut output);
        output
    }

    pub fn print_all_functions(&self) -> String {
        let mut output = String::new();

        for (&_name_idx, &node_idx) in &self.pool.function_defs {
            self.print_node_to_string(node_idx, 0, &mut output);
            if self.config.newlines_after_functions {
                output.push_str("\n\n");
            } else {
                output.push_str("\n");
            }
        }

        output
    }

    fn get_param_name(&self, func_name_idx: NameIdx, param_idx: ParamIdx) -> String {
        if let Some(param_name) = self.pool.get_param_name(func_name_idx, param_idx) {
            param_name.to_string()
        } else {
            format!("p{}", param_idx.0)
        }
    }

    fn print_node_to_string(&self, node_idx: AstIdx, indent_level: usize, output: &mut String) {
        let indent = if self.config.indent_is_tab {
            "\t".repeat(indent_level * self.config.indent_size)
        } else {
            " ".repeat(indent_level * self.config.indent_size)
        };

        match self.pool[node_idx] {
            Ast::Integer(val) => {
                output.push_str(&val.to_string());
            }

            Ast::ParamRef(param_idx) => {
                if let Some((func_name_idx, _)) = self.pool.current_function_context() {
                    let param_name = self.get_param_name(func_name_idx, param_idx);
                    output.push_str(&param_name);
                } else {
                    output.push_str(&format!("p{}", param_idx.0));
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
                // Set current function context for parameter name lookup
                self.pool.set_function_context(name_idx);

                // Function header
                output.push_str(&indent);
                output.push_str("fn ");
                output.push_str(self.pool.get_string(name_idx));
                output.push('(');

                // Parameters
                for i in 0..param_count {
                    if i > 0 {
                        output.push_str(", ");
                    }
                    let param_name = self.get_param_name(name_idx, ParamIdx(i));
                    output.push_str(&param_name);
                }

                output.push_str(") {\n");

                // Function body
                let body_indent = indent_level + 1;
                output.push_str(&" ".repeat(body_indent * self.config.indent_size));

                // Print the body expression
                self.print_node_to_string(body_idx, body_indent, output);

                // Close the function
                output.push('\n');
                output.push_str(&indent);
                output.push('}');

                // Clear function context
                self.pool.clear_function_context();
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
