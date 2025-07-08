# `--save-alias` Specification

## 🎯 Goal

Let users persist CLI invocations as reusable aliases in Kelora’s config file — quickly, safely, and without parsing or extra flags.

---

## 🧾 Syntax

```bash
kelora [ARGS...] --save-alias <alias_name>
```

* `ARGS...` = full CLI invocation (flags, filters, inputs, etc.)
* `--save-alias <name>` = defines the alias to store
* No `--force`, no `--no-overwrite`

---

## 🔍 Behavior

1. **At runtime**:

   * Detect `--save-alias <name>`
   * Capture full original `argv`, excluding `--save-alias <name>`

2. **INI Update**:

   * Locate user config file via standard search order (`~/.config/kelora/config.ini`, etc.)
   * Update `[aliases]` section
   * If the alias does **not exist**, add it
   * If the alias **does exist**, overwrite it

3. **User Feedback**:

   ### If alias is **new**:

   ```
   ✅ Alias 'errors' saved to ~/.config/kelora/config.ini
   ```

   ### If alias is **replaced**:

   ```
   ✅ Alias 'errors' saved to ~/.config/kelora/config.ini
   ℹ️ Replaced previous alias:
       errors = --filter 'level == "error"' --stats --brief
   ```

4. **Output Suggestions**:

   * If successful, suggest reuse:

     ```
     → Run with: kelora -a errors your.log
     ```

---

## 📁 Config File Example

Given:

```bash
kelora -f jsonl app.log --filter 'level == "error"' --stats --save-alias errors
```

Will write:

```ini
[aliases]
errors = -f jsonl app.log --filter 'level == "error"' --stats
```

If `errors` existed before, its value is replaced, and the old value is shown in the terminal — **not** kept in the config.

---

## 🧱 Constraints

* Alias names:

  * Must match regex: `^[a-zA-Z_][a-zA-Z0-9_-]{0,63}$`
  * Must be unique within `[aliases]`
* Config file must be writable
* Will create `[aliases]` section if missing

---

## 🔄 Aliases in Use

Once saved:

```bash
kelora -a errors logs.jsonl
```

expands to:

```bash
kelora -f jsonl app.log --filter 'level == "error"' --stats logs.jsonl
```

---

## ✅ Philosophy Alignment

* ✔️ CLI-native, predictable
* ✔️ No persistence beyond config
* ✔️ No silent surprises — overwrites are visible
* ✔️ Encourages composable, reusable workflows

