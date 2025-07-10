# 📜 Specification: `!` Shell Aliases as Input Streams

---

## ✨ Summary

Kelora supports `!`-prefixed aliases in configuration files. These aliases are treated as **shell commands** whose `stdout` becomes the **input stream for Kelora**. They allow dynamic integration with log sources like `tail`, `kubectl logs`, `gcloud logging read`, etc.

There is **no argument passing or interpolation**. To customize these commands, users must use **environment variables**, which are resolved by the shell — not by Kelora.

---

## 🧠 Core Behavior

### ✅ Alias Detection

* An alias whose value starts with `!` (optionally preceded by whitespace) is treated as a shell alias.
* The `!` and leading space are stripped.
* The remaining string is treated as a shell command to be executed.

### ✅ Shell Execution

* Kelora executes the shell command using the system shell:

  * On Unix: `/bin/sh -c`
  * On Windows: `%COMSPEC% /C` (fallback: document as Unix-only if needed)
* The **stdout** of the command is captured.
* Kelora uses that output as if it were standard input (`stdin`), and proceeds with normal log processing.

### ✅ CLI Argument Handling

* All CLI arguments after `-a aliasname` are interpreted by Kelora.
* **No part of the shell alias receives any of these arguments.**
* Kelora parses the output of the shell alias using the provided flags (`--format`, `--filter`, etc.).

---

## 🔐 Safety and Scope Discipline

### ✅ Only Environment Variable Customization

* If customization is needed (e.g., selecting a file or log group), users must provide values via environment variables.
* Kelora does **not** support:

  * Argument forwarding to shell aliases
  * Placeholder interpolation (e.g., `$1`, `{logfile}`)

### ❌ No Additional Mechanisms

* No dry-run (`--dry-run-alias`)
* No quoting helpers or argument escaping
* No alias nesting or recursion
* No interactive shell fallback

---

## 📦 Example: Basic Usage

```ini
[aliases]
follow-nginx = !tail -f "$LOGFILE"
```

```bash
LOGFILE=/var/log/nginx/access.log kelora -a follow-nginx -f line --filter 'line.contains("404")'
```

→ Kelora executes `tail -f "$LOGFILE"` in `/bin/sh`, captures the output, and runs full log processing on it.

---

## ☁️ Example: Cloud Logs

```ini
[aliases]
gcp-logs = !gcloud logging read "logName=projects/$PROJECT/logs/$LOGNAME" --format json
```

```bash
PROJECT=myproject LOGNAME=syslog kelora -a gcp-logs -f jsonl --filter 'level == "ERROR"'
```

---

## 📌 Summary Table

| Behavior                                | Supported |
| --------------------------------------- | --------- |
| Shell alias input                       | ✅         |
| CLI args passed to shell                | ❌         |
| CLI args interpreted by Kelora          | ✅         |
| Customization via environment variables | ✅         |
| String interpolation or argument tokens | ❌         |
| Dry run / show-resolved-command         | ❌         |
| Recursive aliases                       | ❌         |

---

## 📎 Implementation Notes

* Internally, spawn the shell command with:

  ```rust
  Command::new("/bin/sh")
      .arg("-c")
      .arg(command_string)
      .stdout(Stdio::piped())
  ```
* Use the command’s `stdout` as the input stream (`Reader`) for Kelora’s pipeline
* Any error spawning or reading from the process is treated like a read error from a file

---

## ⚠️ Security Considerations

* Shell aliases are **user-controlled code**
* Config files with `!` aliases should be **trusted** and **not world-writable**
* Environment variables allow safe, explicit customization
* No parsing or substitution is done by Kelora — all shell behavior is delegated to the system shell
