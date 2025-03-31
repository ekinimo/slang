use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::ast::pool::AstPool;
use crate::ast::pretty_printer::PrettyPrinter;
use crate::ast::Ast;
use crate::checker::type_check::TypeChecker;
use crate::compiler::executor::CompiledFunctions;
use crate::parser::parser::parse_program;
use crate::value::Value;

pub struct Interpreter {
    pool: AstPool,
    compiled_functions: CompiledFunctions,
    repl_environment: HashMap<String, Value>,
    loaded_files: Vec<String>,
    debug_mode: bool,
}

impl Interpreter {
    pub fn save_functions_with_deps<P: AsRef<Path>>(
        &self,
        path: P,
        function_names: &[&str],
    ) -> std::result::Result<(), String> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        let mut missing_functions = Vec::new();
        for &func_name in function_names {
            if self.pool.get_name_idx_from_func(func_name).is_none() {
                missing_functions.push(func_name);
            }
        }

        if !missing_functions.is_empty() {
            return Err(format!(
                "The following functions were not found: {}",
                missing_functions.join(", ")
            ));
        }
        let mut ret = String::new();
        for name in function_names {
            let deps = self.pool.find_dependencies(name);
            for dep in deps {
                let dep_name = self.pool.get_string(dep);
                let code = self.pretty_print_function(dep_name)?;
                ret.extend(code.chars());
            }
            let code = self.pretty_print_function(name)?;
            ret.extend(code.chars());
        }
        match fs::write(path.as_ref(), ret) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to write to file '{}': {}", path_str, e)),
        }
    }

    pub fn new() -> Self {
        Self {
            pool: AstPool::new(),
            compiled_functions: CompiledFunctions::new(),
            repl_environment: HashMap::new(),
            loaded_files: Vec::new(),
            debug_mode: false,
        }
    }

    pub fn set_debug_mode(&mut self, enabled: bool) {
        self.debug_mode = enabled;
    }

    pub fn get_deps(&self, name: &str) -> impl Iterator<Item = &str> {
        let ret = self.pool.find_dependencies(&name);
        ret.into_iter().map(|x| self.pool.get_string(x))
    }

    pub fn load_file<P: AsRef<Path>>(&mut self, path: P) -> std::result::Result<(), String> {
        self.pool.import_file(path)?;
        let mut checker = TypeChecker::new(&self.pool);
        if let Err(err) = checker.check_program() {
            return Err(format!("Type check error: {}", err));
        }
        self.compiled_functions.compile(&self.pool);
        Ok(())
    }

    pub fn save_file<P: AsRef<Path>>(&self, path: P) -> std::result::Result<(), String> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        let printer = crate::ast::pretty_printer::PrettyPrinter::new(&self.pool);
        let code = printer.print_all_functions();

        match fs::write(path.as_ref(), code) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to write to file '{}': {}", path_str, e)),
        }
    }

    pub fn eval_expression(&mut self, expr: &str) -> std::result::Result<Value, String> {
        if expr.trim().starts_with("fn ") {
            match parse_program(expr, &mut self.pool) {
                Ok(_) => {
                    let mut checker = TypeChecker::new(&self.pool);
                    if let Err(err) = checker.check_program() {
                        return Err(format!("Type check error: {}", err));
                    }

                    self.compiled_functions = CompiledFunctions::new();
                    self.compiled_functions.compile(&self.pool);

                    Ok(Value::Int(0)) // Return a dummy value
                }
                Err(e) => Err(format!("Parse error: {}", e)),
            }
        } else {
            let synthetic_fn = format!("fn __eval__() {{ {} }}", expr);

            match parse_program(&synthetic_fn, &mut self.pool) {
                Ok(_) => {
                    let mut checker = TypeChecker::new(&self.pool);
                    if let Err(err) = checker.check_program() {
                        return Err(format!("Type check error: {}", err));
                    }

                    self.compiled_functions = CompiledFunctions::new();
                    self.compiled_functions.compile(&self.pool);

                    let name_idx = self.pool.intern_string("__eval__");
                    if let Some(&ast_idx) = self.pool.function_defs.get(&name_idx) {
                        if let Ast::FunctionDef { body_idx, .. } = self.pool[ast_idx] {
                            match self.compiled_functions.execute(body_idx, &self.pool) {
                                Some(value) => Ok(value),
                                None => Err("Failed to execute expression".to_string()),
                            }
                        } else {
                            Err("Internal error: __eval__ is not a function definition".to_string())
                        }
                    } else {
                        Err("Internal error: __eval__ function not found".to_string())
                    }
                }
                Err(e) => Err(format!("Parse error: {}", e)),
            }
        }
    }

    pub fn pretty_print_function(&self, func_name: &str) -> std::result::Result<String, String> {
        let name_idx = self
            .pool
            .get_name_idx_from_func(func_name)
            .ok_or("No such function")?;

        // Find the function definition
        if let Some(&ast_idx) = self.pool.function_defs.get(&name_idx) {
            let printer = PrettyPrinter::new(&self.pool);
            Ok(printer.print_node(ast_idx))
        } else {
            Err(format!("Function '{}' not found", func_name))
        }
    }

    pub fn run_repl(&mut self) {
        println!("Simple Language Interpreter REPL");
        println!("Type expressions to evaluate them, 'help' for commands, or 'exit' to quit");

        let stdin = io::stdin();
        let mut stdout = io::stdout();

        loop {
            print!("> ");
            stdout.flush().unwrap();

            let mut line = String::new();
            stdin.lock().read_line(&mut line).unwrap();

            let input = line.trim();
            if input.is_empty() {
                continue;
            }

            match input {
                "exit" | "quit" => break,

                "help" => {
                    println!("Available commands:");
                    println!("  help                          - Display this help message");
                    println!("  exit                          - Exit the REPL");
                    println!("  quit                          - Same as exit");
                    println!(
                        "  depends     <func>            - Pretty print all defined functions"
                    );
                    println!("  load        <file>            - Load and parse a source file");
                    println!("  save        <file>            - Saved current functions to file");
                    println!("  save-funs   <file> <funcs>+   - Saved current functions to file");
                    println!("  funcs                         - List all defined functions");
                    println!(
                        "  pretty                        - Pretty print all defined functions"
                    );
                    println!("  pretty       <func>?          - Pretty print <func>");
                    println!("  ast                           - Display the current AST");
                    println!("  reset                         - Reset the interpreter state");
                    println!("  <expr>                        - Evaluate an expression");
                }

                "debug" => {
                    self.debug_mode = !self.debug_mode;
                    println!(
                        "Debug mode {}",
                        if self.debug_mode {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    );
                }
                "pretty" => {
                    println!("Pretty printed code:");
                    let printer = PrettyPrinter::new(&self.pool);
                    println!("{}", printer);
                }
                "funcs" => {
                    println!("Defined functions:");
                    for (&name_idx, _) in &self.pool.function_defs {
                        println!("  {}", self.pool.get_string(name_idx));
                    }
                }

                "ast" => {
                    println!("Current AST:");
                    self.pool.display();
                }

                "reset" => {
                    self.pool = AstPool::new();
                    self.compiled_functions = CompiledFunctions::new();
                    self.repl_environment.clear();
                    self.loaded_files.clear();
                    println!("Interpreter state reset");
                }

                _ if input.starts_with("load ") => {
                    let file_path = input[5..].trim();
                    match self.load_file(file_path) {
                        Ok(_) => println!("Successfully loaded file: {}", file_path),
                        Err(e) => println!("Error: {}", e),
                    }
                }

                _ if input.starts_with("depends ") => {
                    let func_name = input[7..].trim();
                    println!("Dependencies of {}", func_name);
                    for deps in self.get_deps(func_name) {
                        println!(" - {}", deps);
                    }
                    println!(" ");
                }
                _ if input.starts_with("pretty ") => {
                    let func_name = input[7..].trim();
                    match self.pretty_print_function(func_name) {
                        Ok(code) => println!("{}", code),
                        Err(e) => println!("Error: {}", e),
                    }
                }
                _ if input.starts_with("save ") => {
                    let file_path = input[5..].trim();
                    if file_path.is_empty() {
                        println!("Error: Missing file path");
                        println!("Usage: save <file_path>");
                    } else {
                        match self.save_file(file_path) {
                            Ok(_) => {
                                println!("Successfully saved all functions to file: {}", file_path)
                            }
                            Err(e) => println!("Error: {}", e),
                        }
                    }
                }

                _ if input.starts_with("save-funs ") => {
                    let args: Vec<&str> = input[10..].trim().split_whitespace().collect();

                    if args.len() < 2 {
                        println!("Error: Missing file path or function names");
                        println!(
                            "Usage: save-deps <file_path> <function_name> [<function_name> ...]"
                        );
                    } else {
                        let file_path = args[0];
                        let function_names = &args[1..];

                        match self.save_functions_with_deps(file_path, function_names) {
                            Ok(()) => {
                                println!("Successfully saved to file: {}", file_path);
                            }
                            Err(e) => println!("Error: {}", e),
                        }
                    }
                }
                _ => {
                    // Treat as an expression to evaluate
                    match self.eval_expression(input) {
                        Ok(result) => println!("=> {:?}", result),
                        Err(e) => println!("Error: {}", e),
                    }
                }
            }
        }
    }

    /// Call a function by name with the provided arguments
    pub fn call_function(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> std::result::Result<Value, String> {
        let name_idx = self.pool.intern_string(name);

        // Find the function definition
        if let Some(&ast_idx) = self.pool.function_defs.get(&name_idx) {
            if let Ast::FunctionDef {
                param_count,
                body_idx,
                ..
            } = self.pool[ast_idx]
            {
                // Check argument count
                if args.len() != param_count {
                    return Err(format!(
                        "Function '{}' expects {} arguments but got {}",
                        name,
                        param_count,
                        args.len()
                    ));
                }

                // Prepare a custom function that sets up the arguments and calls the body
                let arg_values = args.to_vec();
                let call_func = crate::compiler::function::CompiledFunction::new(
                    move |mem: &mut Vec<Value>, _: usize| {
                        // Push arguments to memory
                        for arg in &arg_values {
                            mem.push(*arg);
                        }
                    },
                    0,
                );

                // Execute the prepared function
                let mut memory = Vec::new();
                call_func.call(&mut memory, 0);

                // Now execute the function body with the prepared arguments
                if let Some(compiled_body) =
                    self.compiled_functions.compile_expr(body_idx, &self.pool)
                {
                    compiled_body.call(&mut memory, 0);
                    match memory.pop() {
                        Some(result) => Ok(result),
                        None => Err(format!("Function '{}' did not return a value", name)),
                    }
                } else {
                    Err(format!("Failed to compile function '{}'", name))
                }
            } else {
                Err(format!("'{}' is not a function definition", name))
            }
        } else {
            Err(format!("Function '{}' not found", name))
        }
    }

    /// Dump the current state of the interpreter (for debugging)
    pub fn dump_state(&self) {
        println!("=== Interpreter State ===");
        println!("Debug mode: {}", self.debug_mode);
        println!("Loaded files: {:?}", self.loaded_files);

        println!("\nAST Pool:");
        self.pool.display();

        println!("\nREPL Environment:");
        for (name, value) in &self.repl_environment {
            println!("  {} = {:?}", name, value);
        }
    }
}

/// Run the interpreter with command line arguments
pub fn run_interpreter() -> std::result::Result<(), String> {
    let mut interpreter = Interpreter::new();

    // Process command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        // If given a file, load and execute it
        interpreter.load_file(&args[1])?;

        // If there are additional arguments, try to call a main function
        if args.len() > 2 && args[2] == "run" {
            let main_args = args[3..]
                .iter()
                .map(|s| {
                    // Try to parse as integer first
                    if let Ok(n) = s.parse::<i64>() {
                        Value::Int(n)
                    } else {
                        // Could extend this to support other types if needed
                        Value::Int(0) // Fallback
                    }
                })
                .collect::<Vec<_>>();

            match interpreter.call_function("main", &main_args) {
                Ok(result) => {
                    println!("Program returned: {:?}", result);
                    Ok(())
                }
                Err(e) => Err(format!("Error running main: {}", e)),
            }
        } else {
            // Just load the file without executing anything
            Ok(())
        }
    } else {
        // No arguments, start REPL
        interpreter.run_repl();
        Ok(())
    }
}
