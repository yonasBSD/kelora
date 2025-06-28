# 🔁 Kelora Processing Pipeline

This is how Kelora processes input logs. 
Each stage corresponds to a CLI flag and occurs in a strict order unless otherwise noted.

### 1. Input

| Stage         | Description                              | Flag(s)            |
| ------------- | ---------------------------------------- | ------------------ |
| Source        | Read from stdin or file                  | *(implicit)*       |
| Decompression | Auto-decompress gzip/zstd based on input | *(automatic)*      |
| Skip Lines    | Skip first N raw input lines             | `--skip`           |
| Ignore Lines  | Skip lines matching a regex              | `--ignore-pattern` |


### 2. Chunking

| Stage         | Description                              | Flag(s)     |
| ------------- | ---------------------------------------- | ----------- |
| Line Grouping | Combine raw lines into multi-line chunks | `--chunker` |

### 3. Parsing

| Stage            | Description                           | Flag(s)          |
| ---------------- | ------------------------------------- | ---------------- |
| Format Selection | Choose parser: `json`, `logfmt`, etc. | `-f`, `--format` |
| Format Detection | Auto-detect format per line           | `-f auto`        |


### 4. Scripting (Ordered, Repeatable)

These flags define user logic that transforms, filters, or replaces events. 
They are applied **in the order they appear on the CLI**.

| Stage     | Description                             | Flag(s)    |
| --------- | --------------------------------------- | ---------- |
| Filtering | Drop event if condition is false        | `--filter` |
| Execution | Mutate, track, print, or emit events    | `--exec`   |
| Mapping   | Replace the event with a new expression | `--map`    |

> Notes:
>
> * `--map` is **pure**: no injected variables, no side effects
> * `--exec` and `--filter` inject valid field names as variables


### 5. Limiting

| Stage      | Description                  | Flag(s)  |
| ---------- | ---------------------------- | -------- |
| Emit Limit | Stop after emitting N events | `--take` |


### 6. Formatting

| Stage         | Description                        | Flag(s)                 |
| ------------- | ---------------------------------- | ----------------------- |
| Output Format | Format structured event for output | `-F`, `--output-format` |


### 7. Output

| Stage              | Description                              | Flag(s)       |
| ------------------ | ---------------------------------------- | ------------- |
| Output Destination | Write to stdout or file                  | `--output`    |
| Compression        | Inferred from file extension (.gz, .zst) | *(automatic)* |

