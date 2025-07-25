// Test script for metrics tracking
track_count("total_events");
track_count("error_events", if e.level == "error" { 1 } else { 0 });
track_max("max_timestamp", e.timestamp);
track_unique("users", e.user);

// Test accessing metrics within script
if metrics["total_events"] > 3 {
    print("Processed more than 3 events");
}

// Always emit the event
true