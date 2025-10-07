# Extract and Mask Sensitive Data

Safely process logs containing PII, credentials, and sensitive information using masking, pseudonymization, and redaction techniques.

## Problem

Your logs contain sensitive data like IP addresses, emails, credit card numbers, or personally identifiable information (PII). You need to analyze the logs while protecting sensitive information or comply with privacy regulations.

## Solutions

### Mask IP Addresses

Mask IP addresses to protect privacy while maintaining network analysis capabilities:

```bash
# Mask last octet (default)
> kelora -f json app.log \
    --exec 'e.client_ip = e.client_ip.mask_ip()'

# Mask last 2 octets for stronger anonymization
> kelora -f json app.log \
    --exec 'e.client_ip = e.client_ip.mask_ip(2)'

# Mask last 3 octets
> kelora -f json app.log \
    --exec 'e.client_ip = e.client_ip.mask_ip(3)'
```

**Results:**
- `192.168.1.100` with `mask_ip(1)` becomes `192.168.1.0`
- `192.168.1.100` with `mask_ip(2)` becomes `192.168.0.0`
- `192.168.1.100` with `mask_ip(3)` becomes `192.0.0.0`

### Generate Pseudonyms

Create consistent pseudonyms using domain separation (requires `KELORA_SECRET` environment variable):

```bash
# Set secret for pseudonymization
> export KELORA_SECRET="your-secret-key-here"

# Pseudonymize user emails
> kelora -f json users.log \
    --exec 'e.user_pseudo = pseudonym(e.email, "users")' \
    --exec 'e.email = ()'

# Pseudonymize with different domains
> kelora -f json app.log \
    --exec 'e.user_id = pseudonym(e.user_email, "users")' \
    --exec 'e.session_id = pseudonym(e.session_token, "sessions")' \
    --exec 'e.user_email = ()' \
    --exec 'e.session_token = ()'
```

**Benefits:**
- Same input always produces same pseudonym (consistency)
- Different domains produce different pseudonyms (separation)
- Cannot reverse back to original (one-way)
- Requires secret key (security)

### Hash Sensitive Fields

Hash data for grouping and deduplication without exposing original values:

```bash
# SHA-256 hash (default)
> kelora -f json app.log \
    --exec 'e.user_hash = e.username.hash()' \
    --exec 'e.username = ()'

# Fast hash for grouping (xxh3)
> kelora -f json app.log \
    --exec 'e.session_hash = e.session_id.hash("xxh3")' \
    --exec 'e.session_id = ()'

# MD5 for legacy compatibility
> kelora -f json app.log \
    --exec 'e.email_hash = e.email.hash("md5")' \
    --exec 'e.email = ()'

# BLAKE3 for high-speed cryptographic hashing
> kelora -f json app.log \
    --exec 'e.api_key_hash = e.api_key.hash("blake3")' \
    --exec 'e.api_key = ()'
```

**Available algorithms:** `sha256` (default), `sha1`, `md5`, `xxh3`, `blake3`

### Remove Sensitive Fields

Delete fields entirely from events:

```bash
# Remove single field
> kelora -f json app.log \
    --exec 'e.password = ()'

# Remove multiple sensitive fields
> kelora -f json app.log \
    --exec 'e.password = (); e.api_key = (); e.ssn = (); e.credit_card = ()'

# Conditional removal based on level
> kelora -f json app.log \
    --exec 'if e.level != "DEBUG" { e.stack_trace = (); e.locals = () }'

# Remove all except specific fields
> kelora -f json app.log \
    --exec 'let timestamp = e.timestamp; let level = e.level; let message = e.message' \
    --exec 'e = ()' \
    --exec 'e.timestamp = timestamp; e.level = level; e.message = message'
```

### Extract and Redact Pattern

Extract sensitive data for analysis, then redact from output:

```bash
# Extract credit card type, then redact number
> kelora -f json transactions.log \
    --exec 'e.card_type = e.card_number.extract_re(r"^(\\d{4})", 1)' \
    --exec 'e.card_last4 = e.card_number.extract_re(r"(\\d{4})$", 1)' \
    --exec 'e.card_number = "REDACTED"'

# Extract email domain, remove local part
> kelora -f json users.log \
    --exec 'let email = e.email.parse_email()' \
    --exec 'e.email_domain = email.domain' \
    --exec 'e.email = ()'

# Extract URL domain, remove path/query
> kelora -f json requests.log \
    --exec 'let url = e.request_url.parse_url()' \
    --exec 'e.domain = url.host' \
    --exec 'e.has_query = url.query != ""' \
    --exec 'e.request_url = ()'
```

### Mask Within Text Fields

Find and mask sensitive patterns in log messages:

```bash
# Mask credit card numbers in message
> kelora -f json app.log \
    --exec 'e.message = e.message.replace(r"\\b\\d{16}\\b", "[CARD_REDACTED]")'

# Mask email addresses
> kelora -f json app.log \
    --exec 'e.message = e.message.replace(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}", "[EMAIL_REDACTED]")'

# Mask API keys (various formats)
> kelora -f json app.log \
    --exec 'e.message = e.message.replace(r"api[_-]?key[=:]\\s*[a-zA-Z0-9]{20,}", "api_key=[REDACTED]")'

# Mask IP addresses in text
> kelora -f json app.log \
    --exec 'let ip = e.message.extract_ip()' \
    --exec 'if ip != "" { e.message = e.message.replace(ip, ip.mask_ip(2)) }'
```

## Real-World Examples

### GDPR-Compliant Log Processing

```bash
> export KELORA_SECRET="gdpr-compliance-secret"
> kelora -f json user_activity.log \
    --exec 'e.user_pseudo = pseudonym(e.user_email, "users")' \
    --exec 'e.ip_masked = e.ip_address.mask_ip(2)' \
    --exec 'e.user_email = (); e.ip_address = (); e.full_name = ()' \
    --keys timestamp,user_pseudo,ip_masked,action \
    -F json > anonymized.json
```

### Security Audit Log Sanitization

```bash
> kelora -f json security.log \
    --exec 'e.username_hash = e.username.hash("sha256")' \
    --exec 'e.source_ip = e.source_ip.mask_ip(2)' \
    --exec 'e.session_hash = e.session_id.hash("xxh3")' \
    --exec 'e.username = (); e.session_id = ()' \
    --filter 'e.event_type != "login_success"' \
    -F json > sanitized_audit.json
```

### Payment Log Redaction

```bash
> kelora -f json payments.log \
    --exec 'e.card_last4 = e.card_number.extract_re(r"(\\d{4})$", 1)' \
    --exec 'e.card_bin = e.card_number.extract_re(r"^(\\d{6})", 1)' \
    --exec 'e.amount_bucket = floor(e.amount / 100) * 100' \
    --exec 'e.card_number = (); e.cvv = (); e.expiry = ()' \
    --keys timestamp,card_bin,card_last4,amount_bucket,status
```

### Multi-Tenant Data Isolation

```bash
> export KELORA_SECRET="tenant-separation-key"
> kelora -f json multi_tenant.log \
    --exec 'e.tenant_id = pseudonym(e.tenant_name, "tenants")' \
    --exec 'e.user_id = pseudonym(e.user_email, e.tenant_name)' \
    --exec 'e.tenant_name = (); e.user_email = (); e.user_name = ()' \
    -F json > isolated_logs.json
```

### Database Query Sanitization

```bash
> kelora -f json db_queries.log \
    --exec 'e.query_hash = e.query.hash("xxh3")' \
    --exec 'e.table = e.query.extract_re(r"FROM\\s+(\\w+)", 1)' \
    --exec 'e.has_where = e.query.contains("WHERE")' \
    --exec 'e.query = "REDACTED"' \
    --keys timestamp,user,table,duration_ms,rows,query_hash
```

### API Key and Token Redaction

```bash
> kelora -f combined access.log \
    --exec 'if e.path.contains("key=") || e.path.contains("token=") {
      let params = e.path.after("?").parse_query_params();
      if params.contains("key") { e.has_api_key = true };
      if params.contains("token") { e.has_token = true };
      e.path = e.path.before("?") + "?[PARAMS_REDACTED]"
    }' \
    --keys timestamp,ip,method,path,status
```

### Healthcare Data De-identification

```bash
> export KELORA_SECRET="hipaa-compliance-secret"
> kelora -f json health_records.log \
    --exec 'e.patient_id = pseudonym(e.patient_ssn, "patients")' \
    --exec 'e.provider_id = pseudonym(e.doctor_name, "providers")' \
    --exec 'e.age_bracket = floor(e.age / 10) * 10' \
    --exec 'e.zip_prefix = e.zip_code.slice(":3")' \
    --exec 'e.patient_ssn = (); e.patient_name = (); e.doctor_name = ()' \
    --exec 'e.age = (); e.zip_code = (); e.phone = ()' \
    -F json > deidentified.json
```

### Session Tracking Without PII

```bash
> export KELORA_SECRET="session-tracking-secret"
> kelora -f json app.log \
    --exec 'e.session_hash = pseudonym(e.session_id, "sessions")' \
    --exec 'e.user_hash = pseudonym(e.user_id, "users")' \
    --exec 'e.ip_network = e.ip.mask_ip(2)' \
    --exec 'e.session_id = (); e.user_id = (); e.ip = ()' \
    --exec 'track_unique("sessions", e.session_hash)' \
    --metrics
```

### Extract URLs Without Query Parameters

```bash
> kelora -f json web_requests.log \
    --exec 'let url = e.full_url.parse_url()' \
    --exec 'e.base_url = url.scheme + "://" + url.host + url.path' \
    --exec 'e.has_params = url.query != ""' \
    --exec 'e.param_count = url.query.split("&").len()' \
    --exec 'e.full_url = ()' \
    --keys timestamp,base_url,has_params,param_count
```

## Automation and Export

### Automated Daily Sanitization

```bash
#!/bin/bash
export KELORA_SECRET="$(cat /secure/kelora.secret)"

kelora -f json /var/log/app-$(date +%Y-%m-%d).log \
    --exec 'e.user_id = pseudonym(e.email, "users")' \
    --exec 'e.ip = e.ip.mask_ip(2)' \
    --exec 'e.email = (); e.phone = ()' \
    --parallel \
    -F json > "/archive/sanitized-$(date +%Y-%m-%d).json"
```

### Validate Redaction

```bash
# Check for common sensitive patterns in output
> kelora -f json sanitized.log -qq \
    --filter 'e.message.has_matches("\\b\\d{3}-\\d{2}-\\d{4}\\b")' \
    && echo "WARNING: SSN pattern found!" \
    || echo "No SSN patterns detected"

# Check for email addresses
> kelora -f json sanitized.log -qq \
    --filter 'e.message.has_matches("[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}")' \
    && echo "WARNING: Email addresses found!" \
    || echo "No email addresses detected"
```

### Export to Secure Storage

```bash
> kelora -f json sensitive.log \
    --exec 'e.user_hash = e.email.hash("sha256")' \
    --exec 'e.email = ()' \
    --parallel \
    -F json | gzip > secure_logs_$(date +%Y%m%d).json.gz
```

## Tips

**Security:**
- Always use strong, random `KELORA_SECRET` for pseudonymization
- Store secrets securely (environment variables, secret managers)
- Use domain separation in `pseudonym()` to prevent cross-context correlation
- Prefer `pseudonym()` over `hash()` when consistency across datasets is needed
- Use `hash("xxh3")` for grouping without cryptographic security needs

**Compliance:**
- Document your masking strategy for audit purposes
- Test redaction patterns against sample sensitive data
- Use `--stats` to track how many events were processed
- Consider using `--metrics` to count redacted fields
- Validate output before sharing with third parties

**Performance:**
- Use `--parallel` for large-scale sanitization
- Hash with `xxh3` is faster than cryptographic hashes for grouping
- Remove fields early in pipeline to reduce processing overhead
- Use `mask_ip()` instead of regex for IP address masking

**Validation:**
```bash
# Count events with potential PII
> kelora -f json output.log -q \
    --exec 'if e.message.has_matches("\\b\\d{16}\\b") { track_count("cards") }' \
    --exec 'if e.message.has_matches("@") { track_count("emails") }' \
    --metrics

# Sample output for manual inspection
> kelora -f json sanitized.log --take 100 | less
```

**Common Patterns:**
```bash
# Mask all IPs in event
for field in ["client_ip", "server_ip", "source_ip"] {
    if e.contains(field) {
        e[field] = e[field].mask_ip(2)
    }
}

# Remove all fields containing "password" or "secret"
for (field, value) in e {
    if field.to_lower().contains("password") || field.to_lower().contains("secret") {
        e[field] = ()
    }
}
```

## Troubleshooting

**Pseudonym consistency issues:**
```bash
# Ensure KELORA_SECRET is set and consistent
> echo $KELORA_SECRET
# Must be the same across runs for consistent pseudonyms
```

**IP masking not working:**
```bash
# Validate IP format first
> kelora -f json app.log \
    --filter 'e.ip.is_ipv4()' \
    --exec 'e.ip = e.ip.mask_ip()'
```

**Pattern not matching:**
```bash
# Test regex patterns
> kelora -f json app.log \
    --exec 'e.test_match = e.field.has_matches("your_pattern")' \
    --filter 'e.test_match' \
    --take 10
```

**Hashing performance:**
```bash
# Use faster algorithms for non-cryptographic needs
e.group_id = e.user_id.hash("xxh3")  # Fast
e.secure_id = e.user_id.hash("blake3")  # Cryptographic + Fast
e.legacy_id = e.user_id.hash("sha256")  # Cryptographic
```

## See Also

- [Function Reference](../reference/functions.md) - Complete list of masking and hashing functions
- [Configuration System](../concepts/configuration-system.md) - Centralize redaction defaults
- [Monitor Application Health](monitor-application-health.md) - Health metrics without exposing PII
- [Analyze Web Traffic](analyze-web-traffic.md) - Web log analysis with IP masking
