use rhai::{Array, Dynamic, Engine};

/// Register array manipulation functions with the Rhai engine
pub fn register_functions(engine: &mut Engine) {
    // Register sorted function - like Python's sorted(), takes any iterable and returns sorted items
    engine.register_fn("sorted", sorted_array);
}

/// Sort an array and return a new sorted array (like Python's sorted())
///
/// Takes any array and returns a new sorted array without modifying the original.
/// The sorting is done by converting elements to strings and comparing them lexicographically.
/// Numbers are compared as numbers when both elements are numeric.
///
/// # Arguments
/// * `arr` - The array to sort
///
/// # Returns
/// A new array containing the same elements in sorted order
///
/// # Examples
/// ```rhai
/// let numbers = [3, 1, 4, 1, 5];
/// let sorted_nums = sorted(numbers);  // [1, 1, 3, 4, 5]
/// print(numbers);  // [3, 1, 4, 1, 5] - original unchanged
///
/// let strings = ["banana", "apple", "cherry"];
/// let sorted_strings = sorted(strings);  // ["apple", "banana", "cherry"]
///
/// let mixed = [3, "banana", 1, "apple"];
/// let sorted_mixed = sorted(mixed);  // [1, 3, "apple", "banana"]
/// ```
///
/// # Sorting Behavior
/// - Numbers are compared numerically when both elements are numbers
/// - Strings are compared lexicographically
/// - Mixed types: numbers come before strings
/// - Booleans are converted to strings ("false", "true")
/// - Null values are converted to empty strings and sort first
fn sorted_array(mut arr: Array) -> Array {
    // Sort using a custom comparator
    arr.sort_by(|a, b| {
        // Handle null values first
        if a.is_unit() && b.is_unit() {
            return std::cmp::Ordering::Equal;
        }
        if a.is_unit() {
            return std::cmp::Ordering::Less;
        }
        if b.is_unit() {
            return std::cmp::Ordering::Greater;
        }

        // If both are numbers, compare numerically
        if let (Ok(a_num), Ok(b_num)) = (get_number_value(a), get_number_value(b)) {
            return a_num
                .partial_cmp(&b_num)
                .unwrap_or(std::cmp::Ordering::Equal);
        }

        // If one is a number and the other isn't, numbers come first
        if get_number_value(a).is_ok() && get_number_value(b).is_err() {
            return std::cmp::Ordering::Less;
        }
        if get_number_value(a).is_err() && get_number_value(b).is_ok() {
            return std::cmp::Ordering::Greater;
        }

        // Otherwise, compare as strings
        let a_str = a.to_string();
        let b_str = b.to_string();
        a_str.cmp(&b_str)
    });

    arr
}

/// Helper function to extract numeric value from a Dynamic
fn get_number_value(value: &Dynamic) -> Result<f64, ()> {
    if value.is_int() {
        Ok(value.as_int().unwrap_or(0) as f64)
    } else if value.is_float() {
        Ok(value.as_float().unwrap_or(0.0))
    } else if value.is_string() {
        // Try to parse string as number
        let str_val = value.clone().into_string().unwrap_or_default();
        str_val.parse::<f64>().map_err(|_| ())
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhai::{Array, Dynamic};

    #[test]
    fn test_sorted_numbers() {
        let mut arr = Array::new();
        arr.push(Dynamic::from(3i64));
        arr.push(Dynamic::from(1i64));
        arr.push(Dynamic::from(4i64));
        arr.push(Dynamic::from(1i64));
        arr.push(Dynamic::from(5i64));

        let sorted = sorted_array(arr);

        assert_eq!(sorted.len(), 5);
        assert_eq!(sorted[0].as_int().unwrap(), 1i64);
        assert_eq!(sorted[1].as_int().unwrap(), 1i64);
        assert_eq!(sorted[2].as_int().unwrap(), 3i64);
        assert_eq!(sorted[3].as_int().unwrap(), 4i64);
        assert_eq!(sorted[4].as_int().unwrap(), 5i64);
    }

    #[test]
    fn test_sorted_strings() {
        let mut arr = Array::new();
        arr.push(Dynamic::from("banana"));
        arr.push(Dynamic::from("apple"));
        arr.push(Dynamic::from("cherry"));

        let sorted = sorted_array(arr);

        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].clone().into_string().unwrap(), "apple");
        assert_eq!(sorted[1].clone().into_string().unwrap(), "banana");
        assert_eq!(sorted[2].clone().into_string().unwrap(), "cherry");
    }

    #[test]
    fn test_sorted_mixed_types() {
        let mut arr = Array::new();
        arr.push(Dynamic::from(3i64));
        arr.push(Dynamic::from("banana"));
        arr.push(Dynamic::from(1i64));
        arr.push(Dynamic::from("apple"));

        let sorted = sorted_array(arr);

        assert_eq!(sorted.len(), 4);
        // Numbers should come first
        assert_eq!(sorted[0].as_int().unwrap(), 1i64);
        assert_eq!(sorted[1].as_int().unwrap(), 3i64);
        // Then strings
        assert_eq!(sorted[2].to_string(), "apple");
        assert_eq!(sorted[3].to_string(), "banana");
    }

    #[test]
    fn test_sorted_with_floats() {
        let mut arr = Array::new();
        arr.push(Dynamic::from(3.14));
        arr.push(Dynamic::from(1.5));
        arr.push(Dynamic::from(2.718));

        let sorted = sorted_array(arr);

        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].as_float().unwrap(), 1.5);
        assert_eq!(sorted[1].as_float().unwrap(), 2.718);
        assert_eq!(sorted[2].as_float().unwrap(), 3.14);
    }

    #[test]
    fn test_sorted_with_string_numbers() {
        let mut arr = Array::new();
        arr.push(Dynamic::from("10"));
        arr.push(Dynamic::from("2"));
        arr.push(Dynamic::from("100"));

        let sorted = sorted_array(arr);

        assert_eq!(sorted.len(), 3);
        // Should be sorted numerically, not lexicographically
        assert_eq!(sorted[0].to_string(), "2");
        assert_eq!(sorted[1].to_string(), "10");
        assert_eq!(sorted[2].to_string(), "100");
    }

    #[test]
    fn test_sorted_empty_array() {
        let arr = Array::new();
        let sorted = sorted_array(arr);
        assert_eq!(sorted.len(), 0);
    }

    #[test]
    fn test_sorted_single_element() {
        let mut arr = Array::new();
        arr.push(Dynamic::from("single"));

        let sorted = sorted_array(arr);

        assert_eq!(sorted.len(), 1);
        assert_eq!(sorted[0].to_string(), "single");
    }

    #[test]
    fn test_sorted_with_booleans() {
        let mut arr = Array::new();
        arr.push(Dynamic::from(true));
        arr.push(Dynamic::from(false));
        arr.push(Dynamic::from("apple"));

        let sorted = sorted_array(arr);

        assert_eq!(sorted.len(), 3);
        // Booleans are converted to strings and sorted lexicographically
        assert_eq!(sorted[0].to_string(), "apple");
        assert_eq!(sorted[1].to_string(), "false");
        assert_eq!(sorted[2].to_string(), "true");
    }
}
