#!/usr/bin/env python3
"""
Generate consistent benchmark test data for Kelora performance testing.
Creates predictable JSON log data for repeatable benchmarks.
"""

import json
import random
from datetime import datetime, timedelta
import sys

def generate_log_entry(base_time, entry_id):
    """Generate a single log entry with realistic structure."""
    
    # Deterministic random seed based on entry_id for reproducible data
    random.seed(entry_id)
    
    levels = ["DEBUG", "INFO", "WARN", "ERROR"]
    components = ["api", "database", "auth", "cache", "scheduler", "disk", "network"]
    methods = ["GET", "POST", "PUT", "DELETE", "PATCH"]
    status_codes = [200, 201, 400, 401, 403, 404, 500, 502, 503]
    
    # Generate timestamp
    timestamp = base_time + timedelta(seconds=entry_id)
    
    # Base log structure
    log_entry = {
        "timestamp": timestamp.isoformat() + "Z",
        "level": random.choice(levels),
        "component": random.choice(components),
        "message": f"Operation {entry_id} completed",
        "request_id": f"req-{entry_id:06d}",
        "host": f"host-{random.randint(1, 10)}.example.com",
        "user_id": f"user{random.randint(1, 1000)}",
    }
    
    # Add component-specific fields
    if log_entry["component"] == "api":
        log_entry.update({
            "method": random.choice(methods),
            "endpoint": f"/api/v1/resource/{random.randint(1, 100)}",
            "status": random.choice(status_codes),
            "response_time": f"{random.randint(10, 500)}ms",
            "ip": f"192.168.{random.randint(1, 255)}.{random.randint(1, 255)}",
        })
    elif log_entry["component"] == "database":
        log_entry.update({
            "query_type": random.choice(["SELECT", "INSERT", "UPDATE", "DELETE"]),
            "duration_ms": random.randint(1, 1000),
            "rows_affected": random.randint(0, 100),
            "table": f"table_{random.randint(1, 20)}",
        })
    elif log_entry["component"] == "auth":
        log_entry.update({
            "action": random.choice(["login", "logout", "refresh", "validate"]),
            "success": random.choice([True, False]),
            "method": random.choice(["password", "token", "oauth"]),
        })
    
    # Add error details for ERROR level
    if log_entry["level"] == "ERROR":
        log_entry.update({
            "error_code": f"ERR_{random.randint(1000, 9999)}",
            "stack_trace": f"at line {random.randint(1, 500)} in module {random.choice(components)}",
        })
    
    return log_entry

def main():
    if len(sys.argv) != 2:
        print("Usage: python3 generate_test_data.py <num_lines>")
        sys.exit(1)
    
    try:
        num_lines = int(sys.argv[1])
    except ValueError:
        print("Error: num_lines must be an integer")
        sys.exit(1)
    
    # Fixed base time for reproducible data
    base_time = datetime(2024, 1, 1, 0, 0, 0)
    
    for i in range(num_lines):
        log_entry = generate_log_entry(base_time, i)
        print(json.dumps(log_entry, separators=(',', ':')))

if __name__ == "__main__":
    main()