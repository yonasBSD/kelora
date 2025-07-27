#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    StringProcessing,
    DataParsing,
    ColumnExtraction,
    TimeFunctions,
    MetricsTracking,
    WindowAnalysis,
    SafetyFunctions,
    ArrayFunctions,
    Utility,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Category::StringProcessing => "STRING PROCESSING",
            Category::DataParsing => "DATA PARSING",
            Category::ColumnExtraction => "COLUMN EXTRACTION",
            Category::TimeFunctions => "TIME FUNCTIONS",
            Category::MetricsTracking => "METRICS TRACKING",
            Category::WindowAnalysis => "WINDOW ANALYSIS",
            Category::SafetyFunctions => "SAFETY FUNCTIONS",
            Category::ArrayFunctions => "ARRAY FUNCTIONS",
            Category::Utility => "UTILITY",
        }
    }
    
    pub fn sort_order(&self) -> u8 {
        match self {
            Category::StringProcessing => 1,
            Category::DataParsing => 2,
            Category::ColumnExtraction => 3,
            Category::TimeFunctions => 4,
            Category::MetricsTracking => 5,
            Category::WindowAnalysis => 6,
            Category::SafetyFunctions => 7,
            Category::ArrayFunctions => 8,
            Category::Utility => 9,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionDoc {
    pub name: &'static str,
    pub signature: &'static str,
    pub doc: &'static str,
    pub category: Category,
    pub method: bool,
}

/// Get all function documentation
pub fn get_all_function_docs() -> Vec<FunctionDoc> {
    let mut docs = Vec::new();
    
    // STRING PROCESSING
    docs.extend(vec![
        FunctionDoc { name: "after", signature: "text.after(delimiter)", doc: "Text after first delimiter", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "before", signature: "text.before(delimiter)", doc: "Text before first delimiter", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "between", signature: "text.between(start, end)", doc: "Text between start and end delimiters", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "contains", signature: "text.contains(pattern)", doc: "Check if text contains pattern", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "count", signature: "text.count(pattern)", doc: "Count occurrences of pattern in text", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "ending_with", signature: "text.ending_with(suffix)", doc: "Return text if it ends with suffix", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "extract_domain", signature: "text.extract_domain()", doc: "Extract domain from URL or email", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "extract_ip", signature: "text.extract_ip()", doc: "Extract first IP address from text", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "extract_ips", signature: "text.extract_ips()", doc: "Extract all IP addresses from text", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "extract_re", signature: "text.extract_re(pattern [, group])", doc: "Extract regex match/group from text", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "extract_all_re", signature: "text.extract_all_re(pattern [, group])", doc: "Extract all regex matches from text", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "extract_url", signature: "text.extract_url()", doc: "Extract first URL from text", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "is_digit", signature: "text.is_digit()", doc: "Check if text contains only digits", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "is_private_ip", signature: "text.is_private_ip()", doc: "Check if IP address is in private range", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "lower", signature: "text.lower()", doc: "Convert text to lowercase", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "mask_ip", signature: "text.mask_ip([octets])", doc: "Mask IP address (default: last octet)", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "matches", signature: "text.matches(pattern)", doc: "Check if text matches regex pattern", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "replace_re", signature: "text.replace_re(pattern, replacement)", doc: "Replace regex matches with replacement", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "slice", signature: "text.slice(spec)", doc: "Slice text using Python-style notation", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "split_re", signature: "text.split_re(pattern)", doc: "Split text by regex pattern", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "starting_with", signature: "text.starting_with(prefix)", doc: "Return text if it starts with prefix", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "strip", signature: "text.strip([chars])", doc: "Remove whitespace or specified chars", category: Category::StringProcessing, method: true },
        FunctionDoc { name: "upper", signature: "text.upper()", doc: "Convert text to uppercase", category: Category::StringProcessing, method: true },
    ]);
    
    // DATA PARSING
    docs.extend(vec![
        FunctionDoc { name: "parse_kv", signature: "parse_kv(text [, sep [, kv_sep]])", doc: "Parse key-value pairs from text", category: Category::DataParsing, method: false },
        FunctionDoc { name: "to_float", signature: "to_float(text)", doc: "Convert text to float (0 on error)", category: Category::DataParsing, method: false },
        FunctionDoc { name: "to_int", signature: "to_int(text)", doc: "Convert text to integer (0 on error)", category: Category::DataParsing, method: false },
        FunctionDoc { name: "unflatten", signature: "map.unflatten([separator])", doc: "Reconstruct nested structures from flat keys", category: Category::DataParsing, method: true },
    ]);
    
    // ARRAY FUNCTIONS
    docs.extend(vec![
        FunctionDoc { name: "contains_any", signature: "contains_any(array, search_array)", doc: "Check if array contains any search values", category: Category::ArrayFunctions, method: false },
        FunctionDoc { name: "flatten", signature: "array.flatten([style [, max_depth]])", doc: "Flatten nested arrays/objects", category: Category::ArrayFunctions, method: true },
        FunctionDoc { name: "join", signature: "array.join(separator)", doc: "Join array elements with separator", category: Category::ArrayFunctions, method: true },
        FunctionDoc { name: "reversed", signature: "reversed(array)", doc: "Return new array in reverse order", category: Category::ArrayFunctions, method: false },
        FunctionDoc { name: "sorted", signature: "sorted(array)", doc: "Return new sorted array", category: Category::ArrayFunctions, method: false },
        FunctionDoc { name: "sorted_by", signature: "sorted_by(array, field)", doc: "Sort array of objects by field", category: Category::ArrayFunctions, method: false },
        FunctionDoc { name: "starts_with_any", signature: "starts_with_any(array, search_array)", doc: "Check if array starts with any search values", category: Category::ArrayFunctions, method: false },
    ]);
    
    // SAFETY FUNCTIONS
    docs.extend(vec![
        FunctionDoc { name: "path_equals", signature: "path_equals(obj, \"field.path\", value)", doc: "Safe nested field comparison", category: Category::SafetyFunctions, method: false },
        FunctionDoc { name: "to_bool", signature: "to_bool(value, default)", doc: "Safe boolean conversion with fallback", category: Category::SafetyFunctions, method: false },
        FunctionDoc { name: "to_number", signature: "to_number(value, default)", doc: "Safe number conversion with fallback", category: Category::SafetyFunctions, method: false },
    ]);
    
    // UTILITY
    docs.extend(vec![
        FunctionDoc { name: "eprint", signature: "eprint(message)", doc: "Print to stderr", category: Category::Utility, method: false },
        FunctionDoc { name: "print", signature: "print(message)", doc: "Print to stdout", category: Category::Utility, method: false },
    ]);
    
    docs
}

/// Generate help text from function documentation
pub fn generate_help_text() -> String {
    let docs = get_all_function_docs();
    let mut output = String::from("\nAvailable Rhai Functions for Kelora:\n\n");
    
    // Group by category and sort
    let mut categories: std::collections::BTreeMap<u8, Vec<&FunctionDoc>> = std::collections::BTreeMap::new();
    
    for doc in &docs {
        categories.entry(doc.category.sort_order()).or_default().push(doc);
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