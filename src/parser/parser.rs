use pest::iterators::{Pair, Pairs};
use pest::Parser;
use pest_derive::Parser;
use std::collections::HashMap;

use super::error::{error_with_location, ParserError, Result};
use crate::ast::indices::AstIdx;
use crate::ast::pool::AstPool;
use crate::{NameIdx, ParamIdx};

#[derive(Parser)]
#[grammar = "grammar.pest"] // Update path to grammar file
pub struct LanguageParser;

#[derive(Clone)]
struct Scope {
    // Maps parameter names to their (level, offset) data
    variables: HashMap<String, (usize, (usize, NameIdx))>,
    parent: Option<Box<Scope>>,
    level: usize,
}

impl Scope {
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
            parent: None,
            level: 0,
        }
    }

    fn create_child(&self) -> Self {
        Self {
            variables: HashMap::new(),
            parent: Some(Box::new(self.clone())),
            level: self.level + 1,
        }
    }

    fn add_param(&mut self, name: &str, offset: usize, name_idx: NameIdx) {
        self.variables
            .insert(name.to_string(), (offset, (self.level, name_idx)));
    }

    fn lookup(&self, name: &str) -> Option<(usize, (usize, NameIdx))> {
        if let Some((offset, level_info)) = self.variables.get(name) {
            return Some((*offset, *level_info));
        }

        if let Some(parent) = &self.parent {
            return parent.lookup(name);
        }

        None
    }
}

pub fn parse_program(input: &str, pool: &mut AstPool) -> Result<Vec<AstIdx>> {
    let pairs = LanguageParser::parse(Rule::program, input)?;
    let mut top_level_nodes = Vec::new();
    let mut scope = Scope::new();

    for pair in pairs {
        match pair.as_rule() {
            Rule::program => {
                for inner_pair in pair.into_inner() {
                    match inner_pair.as_rule() {
                        Rule::function_def => {
                            let function_def = parse_function_def(inner_pair, pool, &mut scope)?;
                            top_level_nodes.push(function_def);
                        }
                        Rule::EOI => {}
                        _ => return Err(ParserError::UnexpectedRule(inner_pair.as_rule())),
                    }
                }
            }
            Rule::EOI => {}
            _ => return Err(ParserError::UnexpectedRule(pair.as_rule())),
        }
    }

    Ok(top_level_nodes)
}

fn parse_function_def(
    pair: Pair<Rule>,
    pool: &mut AstPool,
    parent_scope: &mut Scope,
) -> Result<AstIdx> {
    let input = pair.as_str();
    let span = pair.as_span();
    let mut inner_pairs = pair.into_inner();

    let identifier = inner_pairs.next().ok_or_else(|| {
        error_with_location(input, span, "Function definition is missing function name")
    })?;

    if identifier.as_rule() != Rule::identifier {
        return Err(error_with_location(
            input,
            identifier.as_span(),
            &format!(
                "Expected function name but found {:?}",
                identifier.as_rule()
            ),
        ));
    }

    let func_name = identifier.as_str();
    let func_name_idx = pool.intern_string(func_name);

    // Get parameter list
    let param_list = inner_pairs.next().ok_or_else(|| error_with_location(
        input, span, "Function definition is missing parameter list - use empty parentheses '()' for functions with no parameters"
    ))?;

    if param_list.as_rule() != Rule::param_list {
        return Err(error_with_location(
            input,
            param_list.as_span(),
            &format!(
                "Expected parameter list but found {:?}",
                param_list.as_rule()
            ),
        ));
    }

    // Create a new scope for this function's parameters
    let mut function_scope = parent_scope.create_child();

    // Collect and validate parameters
    let param_pairs: Vec<Pair<Rule>> = param_list.into_inner().collect();
    let mut seen_params = HashMap::new();

    for (i, param_pair) in param_pairs.iter().enumerate() {
        if param_pair.as_rule() != Rule::identifier {
            return Err(error_with_location(
                input,
                param_pair.as_span(),
                &format!(
                    "Expected parameter name but found {:?}",
                    param_pair.as_rule()
                ),
            ));
        }

        let param_name = param_pair.as_str();

        if let Some(prev_idx) = seen_params.get(param_name) {
            return Err(error_with_location(
                input,
                span,
                &format!(
                    "Duplicate parameter name '{}' at positions {} and {}",
                    param_name,
                    prev_idx + 1,
                    i + 1
                ),
            ));
        }
        seen_params.insert(param_name, i);

        // Add parameter to function scope
        let param_name_idx = pool.intern_string(param_name);
        function_scope.add_param(param_name, i, param_name_idx);
    }

    // Get function body
    let expr_pair = inner_pairs.next().ok_or_else(|| error_with_location(
        input, span, "Function definition is missing body - function must contain an expression between curly braces"
    ))?;

    if expr_pair.as_rule() != Rule::expr {
        return Err(error_with_location(
            input,
            expr_pair.as_span(),
            &format!(
                "Expected expression in function body but found {:?}",
                expr_pair.as_rule()
            ),
        ));
    }

    // Parse the function body using the function scope
    let body_idx = parse_expr(expr_pair, pool, &function_scope)?;

    Ok(pool.add_function_def(func_name, param_pairs.len(), body_idx))
}

fn parse_expr(pair: Pair<Rule>, pool: &mut AstPool, scope: &Scope) -> Result<AstIdx> {
    let input = pair.as_str();
    let span = pair.as_span();

    match pair.as_rule() {
        Rule::expr => {
            // Unwrap the expr to get to the add_expr inside
            let inner = pair.into_inner().next().ok_or_else(|| {
                error_with_location(input, span, "Empty expression where a value was expected")
            })?;
            parse_expr(inner, pool, scope)
        }
        Rule::add_expr => parse_binary_expr(pair, pool, scope),
        Rule::mul_expr => parse_binary_expr(pair, pool, scope),
        Rule::primary => {
            let inner = pair.into_inner().next().ok_or_else(|| {
                error_with_location(input, span, "Empty expression where a value was expected")
            })?;

            match inner.as_rule() {
                Rule::integer => {
                    let int_span = inner.as_span();
                    let value = inner.as_str().parse::<i64>().map_err(|_| {
                        error_with_location(
                            input,
                            int_span,
                            &format!("Invalid integer literal: '{}'", inner.as_str()),
                        )
                    })?;
                    Ok(pool.add_integer(value))
                }
                Rule::identifier => {
                    let name = inner.as_str();
                    let id_span = inner.as_span();

                    if let Some((offset, (level, name_idx))) = scope.lookup(name) {
                        // Using updated ParamRef with name, level, and offset
                        Ok(pool.add_param_ref(name_idx, level, offset))
                    } else {
                        Err(error_with_location(
                            input,
                            id_span,
                            &format!("Undefined variable '{}' - all variables must be function parameters", name)
                        ))
                    }
                }
                Rule::function_call => parse_function_call(inner, pool, scope),
                Rule::lambda => parse_lambda(inner, pool, scope),
                Rule::expr => parse_expr(inner, pool, scope),
                _ => Err(error_with_location(
                    input,
                    inner.as_span(),
                    &format!("Unexpected syntax element: {:?}", inner.as_rule()),
                )),
            }
        }
        _ => Err(error_with_location(
            input,
            span,
            &format!("Unexpected syntax element: {:?}", pair.as_rule()),
        )),
    }
}

fn parse_lambda(pair: Pair<Rule>, pool: &mut AstPool, parent_scope: &Scope) -> Result<AstIdx> {
    let input = pair.as_str();
    let span = pair.as_span();
    let mut pairs = pair.into_inner();

    // Create a new scope for lambda parameters
    let mut lambda_scope = parent_scope.create_child();

    // Collect lambda parameters
    let mut param_count = 0;
    while pairs
        .peek()
        .map_or(false, |p| p.as_rule() == Rule::identifier)
    {
        let param_pair = pairs.next().unwrap();
        let param_name = param_pair.as_str();
        let param_name_idx = pool.intern_string(param_name);

        // Add parameter to lambda scope
        lambda_scope.add_param(param_name, param_count, param_name_idx);
        param_count += 1;
    }

    // Parse lambda body
    let body_pair = pairs
        .next()
        .ok_or_else(|| error_with_location(input, span, "Lambda expression is missing a body"))?;

    if body_pair.as_rule() != Rule::expr {
        return Err(error_with_location(
            input,
            body_pair.as_span(),
            &format!(
                "Expected expression in lambda body but found {:?}",
                body_pair.as_rule()
            ),
        ));
    }

    let body_idx = parse_expr(body_pair, pool, &lambda_scope)?;

    // Create the lambda node
    Ok(pool.add_lambda(param_count, body_idx))
}

fn parse_binary_expr(pair: Pair<Rule>, pool: &mut AstPool, scope: &Scope) -> Result<AstIdx> {
    let input = pair.as_str();
    let span = pair.as_span();
    let mut pairs = pair.into_inner();

    // Parse the first operand
    let first = pairs.next().ok_or_else(|| {
        error_with_location(input, span, "Binary expression is missing its left operand")
    })?;

    let mut left = parse_expr(first, pool, scope)?;

    // If there are no operators, just return the first operand
    if pairs.peek().is_none() {
        return Ok(left);
    }

    // Process operators and right operands
    while let Some(op) = pairs.next() {
        let op_span = op.as_span();

        if op.as_rule() != Rule::add_op && op.as_rule() != Rule::mul_op {
            return Err(error_with_location(
                input,
                op_span,
                &format!("Expected operator but found {:?}", op.as_rule()),
            ));
        }

        let op_str = op.as_str();

        // Get the right operand
        let right_operand = pairs.next().ok_or_else(|| {
            error_with_location(
                input,
                op_span,
                &format!("Operator '{}' is missing its right operand", op_str),
            )
        })?;

        let right = parse_expr(right_operand, pool, scope)?;

        // Create the appropriate operation based on the operator
        match op_str {
            "+" => {
                // We need to create a new node for the addition
                // Note: in AstPool, we need to add children before the parent
                left = pool.add_add(2);
            }

            "*" => {
                left = pool.add_multiply(2);
            }

            _ => {
                return Err(error_with_location(
                    input,
                    op_span,
                    &format!(
                        "Unsupported operator: '{}' - only +, -, *, / are supported",
                        op_str
                    ),
                ))
            }
        }
    }

    Ok(left)
}

fn parse_function_call(pair: Pair<Rule>, pool: &mut AstPool, scope: &Scope) -> Result<AstIdx> {
    let input = pair.as_str();
    let span = pair.as_span();
    let mut pairs = pair.into_inner();

    // Get function name
    let identifier = pairs.next().ok_or_else(|| {
        error_with_location(input, span, "Function call is missing function name")
    })?;

    if identifier.as_rule() != Rule::identifier {
        return Err(error_with_location(
            input,
            identifier.as_span(),
            &format!(
                "Expected function name but found {:?}",
                identifier.as_rule()
            ),
        ));
    }

    let func_name = identifier.as_str();

    // Get argument list
    let args_pair = pairs.next().ok_or_else(|| {
        error_with_location(input, span, "Function call is missing argument list")
    })?;

    if args_pair.as_rule() != Rule::argument_list {
        return Err(error_with_location(
            input,
            args_pair.as_span(),
            &format!("Expected argument list but found {:?}", args_pair.as_rule()),
        ));
    }

    // Parse each argument
    let mut args = Vec::new();
    for arg_pair in args_pair.into_inner() {
        let arg_idx = parse_expr(arg_pair, pool, scope)?;
        args.push(arg_idx);
    }

    // Add the function call with the correct number of arguments
    Ok(pool.add_function_call(func_name, args.len()))
}
