use rhai::Dynamic;
use std::collections::HashMap;

/// Supported type annotations for field conversion
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    Int,
    Float,
    Bool,
    String,
}

impl FieldType {
    /// Parse a type annotation string into a FieldType
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "int" | "i64" | "integer" => Some(FieldType::Int),
            "float" | "f64" | "double" => Some(FieldType::Float),
            "bool" | "boolean" => Some(FieldType::Bool),
            "string" | "str" => Some(FieldType::String),
            _ => None,
        }
    }
}

/// Type map for field name -> desired type
pub type TypeMap = HashMap<String, FieldType>;

/// Parse a field specification with optional type annotation
/// Returns (field_name, optional_type)
///
/// Examples:
/// - "status" -> ("status", None)
/// - "status:int" -> ("status", Some(FieldType::Int))
/// - "bytes:float" -> ("bytes", Some(FieldType::Float))
pub fn parse_field_with_type(spec: &str) -> Result<(String, Option<FieldType>), String> {
    let parts: Vec<&str> = spec.splitn(2, ':').collect();

    match parts.as_slice() {
        [field_name] => {
            // No type annotation
            Ok((field_name.trim().to_string(), None))
        }
        [field_name, type_str] => {
            // Type annotation present
            let field = field_name.trim().to_string();
            if field.is_empty() {
                return Err("Field name cannot be empty".to_string());
            }

            match FieldType::from_str(type_str.trim()) {
                Some(field_type) => Ok((field, Some(field_type))),
                None => Err(format!("Unknown type annotation: '{}'", type_str.trim())),
            }
        }
        _ => Err("Invalid field specification".to_string()),
    }
}

/// Convert a string value to a Dynamic with the specified type
///
/// # Arguments
/// * `value` - The string value to convert
/// * `field_type` - The target type
/// * `strict` - If true, return error on conversion failure; if false, return string
///
/// # Returns
/// * `Ok(Dynamic)` - Successfully converted value
/// * `Err(String)` - Conversion error (only in strict mode)
pub fn convert_value_to_type(
    value: &str,
    field_type: &FieldType,
    strict: bool,
) -> Result<Dynamic, String> {
    match field_type {
        FieldType::Int => {
            match value.parse::<i64>() {
                Ok(i) => Ok(Dynamic::from(i)),
                Err(_) => {
                    if strict {
                        Err(format!("Cannot convert '{}' to int", value))
                    } else {
                        // Resilient mode: fall back to string
                        Ok(Dynamic::from(value.to_string()))
                    }
                }
            }
        }
        FieldType::Float => {
            match value.parse::<f64>() {
                Ok(f) => Ok(Dynamic::from(f)),
                Err(_) => {
                    if strict {
                        Err(format!("Cannot convert '{}' to float", value))
                    } else {
                        // Resilient mode: fall back to string
                        Ok(Dynamic::from(value.to_string()))
                    }
                }
            }
        }
        FieldType::Bool => {
            match value.to_lowercase().as_str() {
                "true" | "t" | "yes" | "y" | "1" => Ok(Dynamic::from(true)),
                "false" | "f" | "no" | "n" | "0" => Ok(Dynamic::from(false)),
                _ => {
                    if strict {
                        Err(format!("Cannot convert '{}' to bool", value))
                    } else {
                        // Resilient mode: fall back to string
                        Ok(Dynamic::from(value.to_string()))
                    }
                }
            }
        }
        FieldType::String => {
            // Always succeeds
            Ok(Dynamic::from(value.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_type_from_str() {
        assert_eq!(FieldType::from_str("int"), Some(FieldType::Int));
        assert_eq!(FieldType::from_str("INT"), Some(FieldType::Int));
        assert_eq!(FieldType::from_str("i64"), Some(FieldType::Int));
        assert_eq!(FieldType::from_str("float"), Some(FieldType::Float));
        assert_eq!(FieldType::from_str("f64"), Some(FieldType::Float));
        assert_eq!(FieldType::from_str("bool"), Some(FieldType::Bool));
        assert_eq!(FieldType::from_str("string"), Some(FieldType::String));
        assert_eq!(FieldType::from_str("unknown"), None);
    }

    #[test]
    fn test_parse_field_with_type() {
        // Without type annotation
        let (name, type_opt) = parse_field_with_type("status").unwrap();
        assert_eq!(name, "status");
        assert_eq!(type_opt, None);

        // With type annotation
        let (name, type_opt) = parse_field_with_type("status:int").unwrap();
        assert_eq!(name, "status");
        assert_eq!(type_opt, Some(FieldType::Int));

        // With whitespace
        let (name, type_opt) = parse_field_with_type("  bytes : float  ").unwrap();
        assert_eq!(name, "bytes");
        assert_eq!(type_opt, Some(FieldType::Float));

        // Invalid type
        assert!(parse_field_with_type("field:unknown").is_err());

        // Empty field name
        assert!(parse_field_with_type(":int").is_err());
    }

    #[test]
    fn test_convert_value_to_type_int() {
        // Valid integer
        let result = convert_value_to_type("123", &FieldType::Int, true).unwrap();
        assert_eq!(result.as_int().unwrap(), 123);

        // Invalid integer - strict mode
        let result = convert_value_to_type("abc", &FieldType::Int, true);
        assert!(result.is_err());

        // Invalid integer - resilient mode
        let result = convert_value_to_type("abc", &FieldType::Int, false).unwrap();
        assert_eq!(result.clone().into_string().unwrap(), "abc");
    }

    #[test]
    fn test_convert_value_to_type_float() {
        // Valid float
        let result = convert_value_to_type("123.45", &FieldType::Float, true).unwrap();
        assert!((result.as_float().unwrap() - 123.45).abs() < 0.001);

        // Invalid float - strict mode
        let result = convert_value_to_type("abc", &FieldType::Float, true);
        assert!(result.is_err());

        // Invalid float - resilient mode
        let result = convert_value_to_type("abc", &FieldType::Float, false).unwrap();
        assert_eq!(result.clone().into_string().unwrap(), "abc");
    }

    #[test]
    fn test_convert_value_to_type_bool() {
        // Valid bool - true variants
        assert!(convert_value_to_type("true", &FieldType::Bool, true)
            .unwrap()
            .as_bool()
            .unwrap());
        assert!(convert_value_to_type("TRUE", &FieldType::Bool, true)
            .unwrap()
            .as_bool()
            .unwrap());
        assert!(convert_value_to_type("yes", &FieldType::Bool, true)
            .unwrap()
            .as_bool()
            .unwrap());
        assert!(convert_value_to_type("1", &FieldType::Bool, true)
            .unwrap()
            .as_bool()
            .unwrap());

        // Valid bool - false variants
        assert!(!convert_value_to_type("false", &FieldType::Bool, true)
            .unwrap()
            .as_bool()
            .unwrap());
        assert!(!convert_value_to_type("no", &FieldType::Bool, true)
            .unwrap()
            .as_bool()
            .unwrap());
        assert!(!convert_value_to_type("0", &FieldType::Bool, true)
            .unwrap()
            .as_bool()
            .unwrap());

        // Invalid bool - strict mode
        let result = convert_value_to_type("maybe", &FieldType::Bool, true);
        assert!(result.is_err());

        // Invalid bool - resilient mode
        let result = convert_value_to_type("maybe", &FieldType::Bool, false).unwrap();
        assert_eq!(result.clone().into_string().unwrap(), "maybe");
    }

    #[test]
    fn test_convert_value_to_type_string() {
        // String type always succeeds
        let result = convert_value_to_type("anything", &FieldType::String, true).unwrap();
        assert_eq!(result.clone().into_string().unwrap(), "anything");

        let result = convert_value_to_type("anything", &FieldType::String, false).unwrap();
        assert_eq!(result.clone().into_string().unwrap(), "anything");
    }
}
