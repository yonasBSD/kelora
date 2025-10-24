// Reusable helper functions for log analysis

// Check if event is a problem (error or slow)
fn is_problem(event) {
    let is_error = event.level == "ERROR" || event.level == "CRITICAL";
    let is_slow = event.has_field("duration_ms") && event.duration_ms > 1000;
    is_error || is_slow
}

// Classify severity based on level and metrics
fn classify_severity(level, value) {
    if level == "CRITICAL" || level == "ERROR" {
        "high"
    } else if level == "WARN" || value > 1000 {
        "medium"
    } else {
        "low"
    }
}

// Extract domain from URL or email
fn extract_domain(text) {
    if text.contains("@") {
        // Email format
        let parts = text.split("@");
        if parts.len() > 1 {
            parts[1]
        } else {
            "unknown"
        }
    } else if text.contains("://") {
        // URL format
        let after_protocol = text.split("://")[1];
        let domain_part = after_protocol.split("/")[0];
        let without_port = domain_part.split(":")[0];
        without_port
    } else {
        text
    }
}

// Mask sensitive data
fn mask_sensitive(value) {
    if value.len() <= 4 {
        "***"
    } else {
        value.sub_string(0, 2) + "***" + value.sub_string(value.len() - 2, 2)
    }
}
