📄 Input Format Spec: docker

Format Name

-f docker


⸻

🎯 Purpose

Parse log output from:
	•	docker logs (single container)
	•	docker compose logs (multi-container, prefixed)

Into structured Kelora events with the following fields:
	•	msg (required): the main log message
	•	src (optional): container/service name from Compose
	•	ts (optional): parsed timestamp, if present

⸻

🧬 Input Variants

1. Compose with timestamp

web_1    | 2024-07-27T12:34:56.123456789Z GET /health 200

➡

{
  "src": "web_1",
  "ts": "2024-07-27T12:34:56.123456789Z",
  "msg": "GET /health 200"
}


⸻

2. Compose without timestamp

db_1     | Connection established

➡

{
  "src": "db_1",
  "msg": "Connection established"
}


⸻

3. Raw docker logs with timestamp

2024-07-27T12:34:56Z GET /api

➡

{
  "ts": "2024-07-27T12:34:56Z",
  "msg": "GET /api"
}


⸻

4. Raw docker logs without timestamp

Started app in 3.1s

➡

{
  "msg": "Started app in 3.1s"
}


⸻

🔎 Parsing Logic
	1.	Split on first | (Compose prefix)
	•	If found:
	•	Left becomes source (trimmed)
	•	Right becomes payload
	•	If not found:
	•	Entire line is payload
	2.	Try to parse timestamp from start of payload
	•	If payload begins with a known timestamp format:
	•	Extract timestamp as ts
	•	Remaining string becomes msg
	•	If no timestamp:
	•	Entire payload is msg
	3.	Trim all fields

⸻

🕓 Timestamp Parsing
	•	Supports RFC3339/ISO8601 with/without nanoseconds
	•	Example accepted formats:
	•	2024-07-27T12:34:56Z
	•	2024-07-27T12:34:56.123Z
	•	2024-07-27T12:34:56.123456789Z

Uses the same adaptive timestamp parser as other formats, respecting:
	•	--ts-format
	•	--ts-field (not applicable for this format, ignored)
	•	--input-tz

⸻

⚙️ Options

Flag	Description
--strict	Fail on malformed input (invalid timestamp, no msg)
--input-tz	Timezone to assume for naive timestamps
--docker-drop-source (optional)	Do not include the source field in output (discard Compose prefixes)


⸻

📦 Output Schema

Event {
  fields: IndexMap<String, FieldValue> = {
    "msg": "...",              // always present
    "src": "...",              // optional
    "ts": "...",               // optional, parsed as DateTime
  },
  ts: Option<DateTime>,         // populated from "ts" field
  level: Option<String>,        // inferred manually if user defines it
  msg: Option<String>,          // set from "msg" field
}


⸻

❌ Not Supported
	•	Mixed formats (Compose + JSON)
	•	Docker logs in JSON mode (--log-driver=json-file) — use -f jsonl instead
	•	Container labels, stream identifiers, etc. (not in text logs)

⸻

🧪 Example CLI Usage

docker compose logs --timestamps | kelora -f docker --filter 'e.src == "web" && e.msg.contains("500")'

docker logs myapp | kelora -f docker --filter 'e.msg.contains("timeout")'
