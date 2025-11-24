/// Prototype test for AST-based field access detection
///
/// Goal: Validate that we can reliably extract field names from Rhai ASTs,
/// including chained method calls (e.g., e.status.to_upper()).
///
/// Scope:
/// - Top-level fields: e.field ✓
/// - Chained methods: e.field.to_upper() ✓
/// - Static field names only ✓
/// - Nested fields: e.user.name (future)
/// - Dynamic access: e[variable] (future)
use rhai::{Engine, AST};

#[test]
fn test_ast_walking_basic() {
    let engine = Engine::new();

    let test_cases = vec![
        ("e.status > 400", vec!["status"]),
        ("e.level == \"error\"", vec!["level"]),
        ("e.count + 1", vec!["count"]),
        ("e.message.to_upper()", vec!["message"]),
        ("e.field1 > 10 && e.field2 < 20", vec!["field1", "field2"]),
        ("e.has(\"test\") || e.value > 0", vec!["value"]), // e.has is method, not field access
    ];

    for (expr, expected_fields) in test_cases {
        println!("\n=== Testing expression: {} ===", expr);
        println!("Expected fields: {:?}", expected_fields);

        match engine.compile_expression(expr) {
            Ok(ast) => {
                let mut extracted = extract_field_accesses_from_ast(&ast);
                extracted.sort();
                let mut expected = expected_fields.clone();
                expected.sort();

                println!("Extracted fields: {:?}", extracted);

                // Validate extraction
                assert_eq!(
                    extracted, expected,
                    "Field extraction mismatch for expression '{}'\n  Expected: {:?}\n  Got: {:?}",
                    expr, expected, extracted
                );
                println!("✓ Validation passed!");
            }
            Err(e) => {
                panic!("Failed to compile expression '{}': {}", expr, e);
            }
        }
    }
}

fn extract_field_accesses_from_ast(ast: &AST) -> Vec<String> {
    use std::collections::HashSet;

    let mut fields = HashSet::new();
    let mut node_count = 0;

    ast.walk(&mut |path| {
        node_count += 1;

        // Print first few nodes for debugging
        if node_count <= 20 {
            if let Some(node) = path.first() {
                let node_str = format!("{:?}", node);
                println!(
                    "  Node {}: {}",
                    node_count,
                    node_str.chars().take(150).collect::<String>()
                );

                // Extract field names from this node
                extract_fields_from_node_string(&node_str, &mut fields);
            }
        } else if let Some(node) = path.first() {
            // Still extract fields even if we're not printing
            let node_str = format!("{:?}", node);
            extract_fields_from_node_string(&node_str, &mut fields);
        }

        true // Continue walking
    });

    println!("  Total nodes: {}", node_count);

    fields.into_iter().collect()
}

fn extract_fields_from_node_string(node_str: &str, fields: &mut std::collections::HashSet<String>) {
    // Pattern 1: Direct property access - "lhs: Variable(e)" followed by "rhs: Property(field_name)"
    // Look for: Variable(e) ... Property(field_name)

    // First check if this node involves Variable(e)
    if !node_str.contains("Variable(e)") {
        return;
    }

    // Pattern: Dot { lhs: Variable(e) ... rhs: Property(field_name)
    // or nested: rhs: Dot { lhs: Property(field_name)

    // Use simple regex to extract Property(name) that comes after Variable(e)
    let re = regex::Regex::new(r"Variable\(e\)[^}]*Property\((\w+)\)").unwrap();
    for cap in re.captures_iter(node_str) {
        if let Some(field_name) = cap.get(1) {
            fields.insert(field_name.as_str().to_string());
        }
    }

    // Also handle nested case: rhs: Dot { lhs: Property(field)
    let nested_re = regex::Regex::new(r"rhs: Dot \{ lhs: Property\((\w+)\)").unwrap();
    // But only if we also have Variable(e) in the node
    if node_str.contains("lhs: Variable(e)") {
        for cap in nested_re.captures_iter(node_str) {
            if let Some(field_name) = cap.get(1) {
                fields.insert(field_name.as_str().to_string());
            }
        }
    }
}

#[test]
fn test_ast_structure_exploration() {
    println!("\n\n╔══════════════════════════════════════════════════════╗");
    println!("║  AST STRUCTURE EXPLORATION FOR FIELD DETECTION     ║");
    println!("╚══════════════════════════════════════════════════════╝\n");

    let engine = Engine::new();

    // Start with simplest case
    let simple_expr = "e.status";
    println!("=== SIMPLEST: {} ===", simple_expr);
    if let Ok(ast) = engine.compile_expression(simple_expr) {
        walk_and_print_ast(&ast);
    }

    println!("\n");
}

fn walk_and_print_ast(ast: &AST) {
    use std::collections::HashMap;

    let mut node_types: HashMap<String, usize> = HashMap::new();

    ast.walk(&mut |path| {
        if let Some(node) = path.first() {
            let node_debug = format!("{:?}", node);
            let node_type = node_debug
                .split('(')
                .next()
                .unwrap_or("Unknown")
                .to_string();
            *node_types.entry(node_type.clone()).or_insert(0) += 1;

            // Print full node for first occurrence of each type
            if node_types[&node_type] == 1 {
                println!(
                    "  [{}] {}",
                    node_type,
                    node_debug.chars().take(200).collect::<String>()
                );
            }
        }
        true
    });

    println!("\n  Node type summary:");
    for (node_type, count) in node_types.iter() {
        println!("    {}: {}", node_type, count);
    }
}
