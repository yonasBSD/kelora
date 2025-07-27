#[allow(dead_code)] // Variants used by generate_help_text function called from main.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    StringProcessing,
    DataParsing,
    SafetyFunctions,
    ArrayFunctions,
    Utility,
}

impl Category {
    #[allow(dead_code)] // Used by generate_help_text function called from main.rs
    pub fn as_str(&self) -> &'static str {
        match self {
            Category::StringProcessing => "STRING PROCESSING",
            Category::DataParsing => "DATA PARSING",
            Category::SafetyFunctions => "SAFETY FUNCTIONS",
            Category::ArrayFunctions => "ARRAY FUNCTIONS",
            Category::Utility => "UTILITY",
        }
    }

    #[allow(dead_code)] // Used by generate_help_text function called from main.rs
    pub fn sort_order(&self) -> u8 {
        match self {
            Category::StringProcessing => 1,
            Category::DataParsing => 2,
            Category::SafetyFunctions => 3,
            Category::ArrayFunctions => 4,
            Category::Utility => 5,
        }
    }
}

#[allow(dead_code)] // Used by generate_help_text function called from main.rs
#[derive(Debug, Clone)]
pub struct FunctionDoc {
    pub signature: &'static str,
    pub doc: &'static str,
    pub category: Category,
}

/// Get all function documentation
#[allow(dead_code)] // Used by generate_help_text function called from main.rs
pub fn get_all_function_docs() -> Vec<FunctionDoc> {
    let mut docs = Vec::new();

    // STRING PROCESSING
    docs.extend(vec![
        FunctionDoc {
            signature: "text.after(delimiter)",
            doc: "Text after first delimiter",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.before(delimiter)",
            doc: "Text before first delimiter",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.between(start, end)",
            doc: "Text between start and end delimiters",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.contains(pattern)",
            doc: "Check if text contains pattern",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.count(pattern)",
            doc: "Count occurrences of pattern in text",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.ending_with(suffix)",
            doc: "Return text if it ends with suffix",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.extract_domain()",
            doc: "Extract domain from URL or email",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.extract_ip()",
            doc: "Extract first IP address from text",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.extract_ips()",
            doc: "Extract all IP addresses from text",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.extract_re(pattern [, group])",
            doc: "Extract regex match/group from text",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.extract_all_re(pattern [, group])",
            doc: "Extract all regex matches from text",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.extract_url()",
            doc: "Extract first URL from text",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.is_digit()",
            doc: "Check if text contains only digits",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.is_private_ip()",
            doc: "Check if IP address is in private range",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.lower()",
            doc: "Convert text to lowercase",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.mask_ip([octets])",
            doc: "Mask IP address (default: last octet)",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.matches(pattern)",
            doc: "Check if text matches regex pattern",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.replace_re(pattern, replacement)",
            doc: "Replace regex matches with replacement",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.slice(spec)",
            doc: "Slice text using Python-style notation",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.split_re(pattern)",
            doc: "Split text by regex pattern",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.starting_with(prefix)",
            doc: "Return text if it starts with prefix",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.strip([chars])",
            doc: "Remove whitespace or specified chars",
            category: Category::StringProcessing,
        },
        FunctionDoc {
            signature: "text.upper()",
            doc: "Convert text to uppercase",
            category: Category::StringProcessing,
        },
    ]);

    // DATA PARSING
    docs.extend(vec![
        FunctionDoc {
            signature: "parse_kv(text [, sep [, kv_sep]])",
            doc: "Parse key-value pairs from text",
            category: Category::DataParsing,
        },
        FunctionDoc {
            signature: "to_float(text)",
            doc: "Convert text to float (0 on error)",
            category: Category::DataParsing,
        },
        FunctionDoc {
            signature: "to_int(text)",
            doc: "Convert text to integer (0 on error)",
            category: Category::DataParsing,
        },
        FunctionDoc {
            signature: "map.unflatten([separator])",
            doc: "Reconstruct nested structures from flat keys",
            category: Category::DataParsing,
        },
    ]);

    // ARRAY FUNCTIONS
    docs.extend(vec![
        FunctionDoc {
            signature: "contains_any(array, search_array)",
            doc: "Check if array contains any search values",
            category: Category::ArrayFunctions,
        },
        FunctionDoc {
            signature: "array.flatten([style [, max_depth]])",
            doc: "Flatten nested arrays/objects",
            category: Category::ArrayFunctions,
        },
        FunctionDoc {
            signature: "array.join(separator)",
            doc: "Join array elements with separator",
            category: Category::ArrayFunctions,
        },
        FunctionDoc {
            signature: "reversed(array)",
            doc: "Return new array in reverse order",
            category: Category::ArrayFunctions,
        },
        FunctionDoc {
            signature: "sorted(array)",
            doc: "Return new sorted array",
            category: Category::ArrayFunctions,
        },
        FunctionDoc {
            signature: "sorted_by(array, field)",
            doc: "Sort array of objects by field",
            category: Category::ArrayFunctions,
        },
        FunctionDoc {
            signature: "starts_with_any(array, search_array)",
            doc: "Check if array starts with any search values",
            category: Category::ArrayFunctions,
        },
    ]);

    // SAFETY FUNCTIONS
    docs.extend(vec![
        FunctionDoc {
            signature: "path_equals(obj, \"field.path\", value)",
            doc: "Safe nested field comparison",
            category: Category::SafetyFunctions,
        },
        FunctionDoc {
            signature: "to_bool(value, default)",
            doc: "Safe boolean conversion with fallback",
            category: Category::SafetyFunctions,
        },
        FunctionDoc {
            signature: "to_number(value, default)",
            doc: "Safe number conversion with fallback",
            category: Category::SafetyFunctions,
        },
    ]);

    // UTILITY
    docs.extend(vec![
        FunctionDoc {
            signature: "eprint(message)",
            doc: "Print to stderr",
            category: Category::Utility,
        },
        FunctionDoc {
            signature: "print(message)",
            doc: "Print to stdout",
            category: Category::Utility,
        },
    ]);

    docs
}

/// Generate help text from function documentation
#[allow(dead_code)] // Used by main.rs for --help-functions flag
pub fn generate_help_text() -> String {
    let docs = get_all_function_docs();
    let mut output = String::from("\nAvailable Rhai Functions for Kelora:\n\n");

    // Group by category and sort
    let mut categories: std::collections::BTreeMap<u8, Vec<&FunctionDoc>> =
        std::collections::BTreeMap::new();

    for doc in &docs {
        categories
            .entry(doc.category.sort_order())
            .or_default()
            .push(doc);
    }

    for (_, category_docs) in categories {
        if category_docs.is_empty() {
            continue;
        }

        let category_name = category_docs[0].category.as_str();
        output.push_str(&format!("{}:\n", category_name));

        // Sort functions within category by signature
        let mut sorted_docs = category_docs;
        sorted_docs.sort_by(|a, b| a.signature.cmp(b.signature));

        for doc in sorted_docs {
            // Format: "  signature                             doc string"
            output.push_str(&format!("  {:<35} {}\n", doc.signature, doc.doc));
        }
        output.push('\n');
    }

    output.push_str("Note: These functions are available in --filter and --exec expressions.\n");
    output.push_str("Use 'e' to access the current event (e.g., e.message.lower()).\n");
    output.push_str("For more examples, see the documentation or use --help for general usage.\n");

    output
}
