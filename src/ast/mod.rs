pub mod indices;
pub mod pool;
pub mod primitives;

pub mod pretty_printer;

// Re-export main types for convenient usage
pub use self::indices::{AstIdx, FunIdx, NameIdx, ParamIdx};
pub use self::pool::AstPool;
pub use self::primitives::PrimitiveFunc;

#[derive(Debug, Clone, Copy)]
pub enum Ast {
    Integer(i64),
    ParamRef {
        name: NameIdx,
        level: usize,
        offset: ParamIdx,
    },
    PrimitiveFunc(PrimitiveFunc),
    UserFunc(NameIdx),
    Lambda {
        param_count: usize,
        body_idx: AstIdx,
    },
    Call {
        func_idx: AstIdx,
        child_start: AstIdx,
        child_count: usize,
        len: usize,
    },
    FunctionDef {
        name_idx: NameIdx,
        param_count: usize,
        body_idx: AstIdx,
    },
}

#[cfg(test)]
mod tests {
    use crate::ast::pool::AstPool;
    use crate::ast::{Ast, AstIdx};
    use crate::parser::parse_program;

    fn parse_and_get_pool(input: &str) -> (Vec<AstIdx>, AstPool) {
        let mut pool = AstPool::new();
        let result = parse_program(input, &mut pool).expect("Failed to parse program");
        (result, pool)
    }

    fn traverse_and_print(pool: &AstPool, node_idx: AstIdx, depth: usize) -> Vec<String> {
        let mut result = Vec::new();
        let indent = "  ".repeat(depth);

        let node_desc = match pool[node_idx] {
            Ast::Integer(n) => format!("Integer({})", n),
            Ast::ParamRef {
                name,
                level,
                offset,
            } => format!(
                "ParamRef({}, level={}, offset={})",
                pool.get_string(name),
                level,
                offset.0
            ),
            Ast::PrimitiveFunc(f) => format!("PrimitiveFunc({:?})", f),
            Ast::UserFunc(name) => format!("UserFunc({})", pool.get_string(name)),
            Ast::Lambda { param_count, .. } => format!("Lambda(params={})", param_count),
            Ast::Call {
                func_idx,
                child_count,
                ..
            } => {
                let func_desc = match pool[func_idx] {
                    Ast::PrimitiveFunc(f) => format!("{:?}", f),
                    Ast::UserFunc(name) => pool.get_string(name).to_string(),
                    _ => "anonymous".to_string(),
                };
                format!("Call({}), with {} args", func_desc, child_count)
            }
            Ast::FunctionDef {
                name_idx,
                param_count,
                ..
            } => format!(
                "FunctionDef({}, params={})",
                pool.get_string(name_idx),
                param_count
            ),
        };

        result.push(format!("{}{:?}: {}", indent, node_idx, node_desc));

        if let Some(children) = pool.children(node_idx) {
            result.push(format!("{}Children count: {}", indent, children.len()));
            for (i, &child_idx) in children.iter().enumerate() {
                result.push(format!("{}Child #{}: {:?}", indent, i, child_idx));
                result.extend(traverse_and_print(pool, child_idx, depth + 1));
            }
        } else {
            result.push(format!("{}No children", indent));
        }

        result
    }

    #[test]
    fn test_muladd_simple() {
        let input = r#"
            fn muladd(x, y, z) {
                x * y + z
            }
            fn test1() {
                muladd(1, 2, 3)
            }
        "#;

        let (ast_indices, pool) = parse_and_get_pool(input);
        println!("Parsed AST Nodes:");
        pool.display();

        let test_idx = ast_indices
            .iter()
            .position(|&idx| {
                if let Ast::FunctionDef { name_idx, .. } = pool[idx] {
                    pool.get_string(name_idx) == "test1"
                } else {
                    false
                }
            })
            .expect("Test function not found");

        let test_node = ast_indices[test_idx];

        println!("\nTraversing test1:");
        let traversal = traverse_and_print(&pool, test_node, 0);
        for line in &traversal {
            println!("{}", line);
        }

        if let Ast::FunctionDef { body_idx, .. } = pool[test_node] {
            assert!(matches!(pool[body_idx], Ast::Call { .. }));

            let children = pool.children(body_idx).expect("Call should have children");
            assert_eq!(children.len(), 3);

            let expected_values = [1, 2, 3];
            for (i, &child) in children.iter().enumerate() {
                match pool[child] {
                    Ast::Integer(value) => {
                        assert_eq!(
                            value, expected_values[i],
                            "Argument {} has incorrect value",
                            i
                        );
                    }
                    _ => panic!("Expected Integer for argument {}", i),
                }
            }
        }
    }

    #[test]
    fn test_muladd_nested_last() {
        let input = r#"
            fn muladd(x, y, z) {
                x * y + z
            }
            fn test1() {
                muladd(1, 2, muladd(3,4,5))
            }
        "#;

        let (ast_indices, pool) = parse_and_get_pool(input);
        println!("Parsed AST Nodes:");
        pool.display();

        let test_idx = ast_indices
            .iter()
            .position(|&idx| {
                if let Ast::FunctionDef { name_idx, .. } = pool[idx] {
                    pool.get_string(name_idx) == "test1"
                } else {
                    false
                }
            })
            .expect("Test function not found");

        let test_node = ast_indices[test_idx];

        println!("\nTraversing test1:");
        let traversal = traverse_and_print(&pool, test_node, 0);
        for line in &traversal {
            println!("{}", line);
        }

        if let Ast::FunctionDef { body_idx, .. } = pool[test_node] {
            assert!(matches!(pool[body_idx], Ast::Call { .. }));

            let children = pool.children(body_idx).expect("Call should have children");
            assert_eq!(children.len(), 3);
            println!("Children Ids : {children:?}");

            let in_expected_values = [5, 4, 3];
            let expected_values = [1, 2, 3];
            for (i, &child) in children.iter().enumerate() {
                match pool[child] {
                    Ast::Integer(value) => {
                        assert_eq!(
                            value, expected_values[i],
                            "Argument {} has incorrect value",
                            i
                        );
                    }
                    Ast::Call {
                        func_idx,
                        child_start,
                        child_count,
                        len,
                    } => {
                        println!(
                            "Inner Call :
\t func_idx     : {func_idx:?}
\t child_start  : {child_start:?}
\t child_count  : {child_count}
\t len          : {len}

Ast              : {}
",
                            pool.nodes
                                .iter()
                                .enumerate()
                                .map(|(x, y)| format!("{x} : {y:?}\n"))
                                .collect::<String>()
                        );

                        assert_eq!(child_count, 3);
                        for i in 0..child_count {
                            match pool[AstIdx(child_start.0 - i)] {
                                Ast::Integer(value) => {
                                    assert_eq!(
                                        value, in_expected_values[i],
                                        "Argument {} has incorrect value",
                                        i
                                    );
                                }
                                _ => panic!("Expected Integer for argument {}", i),
                            }
                        }
                    }
                    _ => panic!("Expected Integer for argument {}", i),
                }
            }
        }
    }

    #[test]
    fn test_muladd_nested_first_arg() {
        let input = r#"
        fn muladd(x, y, z) {
            x * y + z
        }
        fn test_nested() {
            muladd(muladd(1, 2, 3), 4, 5)
        }
    "#;

        let (ast_indices, pool) = parse_and_get_pool(input);
        println!("Parsed AST Nodes:");
        pool.display();

        let test_idx = ast_indices
            .iter()
            .position(|&idx| {
                if let Ast::FunctionDef { name_idx, .. } = pool[idx] {
                    pool.get_string(name_idx) == "test_nested"
                } else {
                    false
                }
            })
            .expect("Test function not found");

        let test_node = ast_indices[test_idx];

        println!("\nTraversing test_nested:");
        let traversal = traverse_and_print(&pool, test_node, 0);
        for line in &traversal {
            println!("{}", line);
        }

        if let Ast::FunctionDef { body_idx, .. } = pool[test_node] {
            assert!(matches!(pool[body_idx], Ast::Call { .. }));
            let children = pool.children(body_idx).expect("Call should have children");
            assert_eq!(children.len(), 3);
            println!("Children Ids: {children:?}");

            assert!(matches!(pool[children[1]], Ast::Integer(4)));
            assert!(matches!(pool[children[2]], Ast::Integer(5)));

            match pool[children[0]] {
                Ast::Call {
                    func_idx,
                    child_start,
                    child_count,
                    len,
                } => {
                    println!(
                    "Inner Call:\n\tfunc_idx: {func_idx:?}\n\tchild_start: {child_start:?}\n\tchild_count: {child_count}\n\tlen: {len}"
                );

                    match pool[func_idx] {
                        Ast::UserFunc(name_idx) => {
                            assert_eq!(pool.get_string(name_idx), "muladd");
                        }
                        _ => panic!("Expected UserFunc for inner call"),
                    }

                    let inner_children = pool
                        .children(children[0])
                        .expect("Inner call should have children");
                    assert_eq!(inner_children.len(), 3);

                    let inner_expected = [1, 2, 3];
                    for (i, &child) in inner_children.iter().enumerate() {
                        match pool[child] {
                            Ast::Integer(value) => {
                                assert_eq!(
                                    value, inner_expected[i],
                                    "Inner argument {} has incorrect value",
                                    i
                                );
                            }
                            _ => panic!("Expected Integer for inner argument {}", i),
                        }
                    }
                }
                _ => panic!("Expected Call for first argument"),
            }
        }
    }

    #[test]
    fn test_muladd_nested_middle_arg() {
        let input = r#"
        fn muladd(x, y, z) {
            x * y + z
        }
        fn test_nested_middle() {
            muladd(1, muladd(2, 3, 4), 5)
        }
    "#;

        let (ast_indices, pool) = parse_and_get_pool(input);
        println!("Parsed AST Nodes:");
        pool.display();

        let test_idx = ast_indices
            .iter()
            .position(|&idx| {
                if let Ast::FunctionDef { name_idx, .. } = pool[idx] {
                    pool.get_string(name_idx) == "test_nested_middle"
                } else {
                    false
                }
            })
            .expect("Test function not found");

        let test_node = ast_indices[test_idx];

        println!("\nTraversing test_nested_middle:");
        let traversal = traverse_and_print(&pool, test_node, 0);
        for line in &traversal {
            println!("{}", line);
        }

        if let Ast::FunctionDef { body_idx, .. } = pool[test_node] {
            assert!(matches!(pool[body_idx], Ast::Call { .. }));
            let children = pool.children(body_idx).expect("Call should have children");
            assert_eq!(children.len(), 3);
            println!("Children Ids: {children:?}");

            assert!(matches!(pool[children[0]], Ast::Integer(1)));
            assert!(matches!(pool[children[2]], Ast::Integer(5)));

            match pool[children[1]] {
                Ast::Call {
                    func_idx,
                    child_start,
                    child_count,
                    len,
                } => {
                    println!(
                    "Inner Call:\n\tfunc_idx: {func_idx:?}\n\tchild_start: {child_start:?}\n\tchild_count: {child_count}\n\tlen: {len}"
                );

                    match pool[func_idx] {
                        Ast::UserFunc(name_idx) => {
                            assert_eq!(pool.get_string(name_idx), "muladd");
                        }
                        _ => panic!("Expected UserFunc for inner call"),
                    }

                    let inner_children = pool
                        .children(children[1])
                        .expect("Inner call should have children");
                    assert_eq!(inner_children.len(), 3);

                    let inner_expected = [2, 3, 4];
                    for (i, &child) in inner_children.iter().enumerate() {
                        match pool[child] {
                            Ast::Integer(value) => {
                                assert_eq!(
                                    value, inner_expected[i],
                                    "Inner argument {} has incorrect value",
                                    i
                                );
                            }
                            _ => panic!("Expected Integer for inner argument {}", i),
                        }
                    }
                }
                _ => panic!("Expected Call for middle argument"),
            }
        }
    }
    #[test]
    fn test_higher_order_multiple_args() {
        let input = r#"
        fn curry(a, b) {
            lambda c d e {
                lambda f {
                    a + b + c + d + e + f
                }

            }
        }

        fn test_curried() {
            curry(1, 2)(3, 4, 5)(6)
        }
    "#;

        let (ast_indices, pool) = parse_and_get_pool(input);
        println!("Parsed AST Nodes:");
        pool.display();

        let test_idx = ast_indices
            .iter()
            .position(|&idx| {
                if let Ast::FunctionDef { name_idx, .. } = pool[idx] {
                    pool.get_string(name_idx) == "test_curried"
                } else {
                    false
                }
            })
            .expect("Test function not found");

        let test_node = ast_indices[test_idx];

        println!("\nTraversing test_curried:");
        let traversal = traverse_and_print(&pool, test_node, 0);
        for line in &traversal {
            println!("{}", line);
        }

        if let Ast::FunctionDef { body_idx, .. } = pool[test_node] {
            assert!(matches!(pool[body_idx], Ast::Call { .. }));

            let outer_children = pool
                .children(body_idx)
                .expect("Outer call should have children");
            assert_eq!(outer_children.len(), 1);
            assert!(matches!(pool[outer_children[0]], Ast::Integer(6)));

            if let Ast::Call {
                func_idx: middle_call_idx,
                ..
            } = pool[body_idx]
            {
                assert!(matches!(pool[middle_call_idx], Ast::Call { .. }));

                let middle_children = pool
                    .children(middle_call_idx)
                    .expect("Middle call should have children");
                assert_eq!(middle_children.len(), 3);
                assert!(matches!(pool[middle_children[0]], Ast::Integer(3)));
                assert!(matches!(pool[middle_children[1]], Ast::Integer(4)));
                assert!(matches!(pool[middle_children[2]], Ast::Integer(5)));

                if let Ast::Call {
                    func_idx: inner_func_idx,
                    ..
                } = pool[middle_call_idx]
                {
                    assert!(matches!(pool[inner_func_idx], Ast::Call { .. }));

                    let inner_children = pool
                        .children(inner_func_idx)
                        .expect("Inner call should have children");
                    assert_eq!(inner_children.len(), 2);
                    assert!(matches!(pool[inner_children[0]], Ast::Integer(1)));
                    assert!(matches!(pool[inner_children[1]], Ast::Integer(2)));

                    if let Ast::Call {
                        func_idx: curry_func_idx,
                        ..
                    } = pool[inner_func_idx]
                    {
                        match pool[curry_func_idx] {
                            Ast::UserFunc(name_idx) => {
                                assert_eq!(pool.get_string(name_idx), "curry");
                            }
                            _ => panic!("Expected UserFunc for curry function"),
                        }
                    } else {
                        panic!("Expected Call for innermost call");
                    }
                } else {
                    panic!("Expected Call for middle call's function");
                }
            } else {
                panic!("Expected Call for outer call's function");
            }
        }
    }
}
