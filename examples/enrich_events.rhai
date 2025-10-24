// This script enriches log events with additional computed fields

// Add timestamp metadata
if e.has_field("timestamp") {
    let ts = e.timestamp;
    if ts.contains("T") {
        let parts = ts.split("T");
        e.date = parts[0];
        if parts.len() > 1 {
            e.time = parts[1].split("Z")[0];
        }
    }
}

// Classify by severity
if e.has_field("level") {
    e.severity = if e.level == "ERROR" || e.level == "CRITICAL" {
        "high"
    } else if e.level == "WARN" {
        "medium"
    } else {
        "low"
    };
}

// Add duration classification if present
if e.has_field("duration_ms") {
    e.speed = if e.duration_ms < 100 {
        "fast"
    } else if e.duration_ms < 1000 {
        "normal"
    } else {
        "slow"
    };
}

// Track by service and level
if e.has_field("service") && e.has_field("level") {
    track_count(e.service + ":" + e.level);
}
