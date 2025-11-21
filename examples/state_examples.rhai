// state_examples.rhai — Common patterns for using the `state` global map
//
// The `state` global map enables complex stateful processing across events.
// Only works in sequential mode (not with --parallel).
//
// WHEN TO USE `state`:
//   - Deduplication (tracking seen IDs)
//   - Cross-event dependencies
//   - Storing complex objects (nested maps, arrays)
//   - Conditional logic based on previous events
//
// WHEN NOT TO USE `state`:
//   - Simple counting or metrics → use track_count(), track_sum(), etc.
//     (they work in parallel mode too)
//
// Usage:
//   kelora -j logs.jsonl --exec-file examples/state_examples.rhai
//
// See also:
//   - tests/state_integration_test.rs for comprehensive examples
//   - docs/reference/script-variables.md for full API reference
//   - docs/how-to/power-user-techniques.md for use cases

// ============================================================================
// PATTERN 1: Deduplication - Track seen IDs
// ============================================================================
//
// Skip duplicate events based on request_id
//
// if !state.contains(e.request_id) {
//     state[e.request_id] = true;
//     // Process first occurrence
// } else {
//     // Skip duplicate
//     e = ();
// }


// ============================================================================
// PATTERN 2: Sequential numbering across all events
// ============================================================================
//
// Assign a global sequence number to each event
// Use with --begin to initialize:
//   kelora -j logs.jsonl --begin 'state["seq"] = 0' \
//     --exec-file examples/state_examples.rhai
//
// state["seq"] = (state["seq"] ?? 0) + 1;
// e.sequence_number = state["seq"];


// ============================================================================
// PATTERN 3: Track complex per-entity state
// ============================================================================
//
// Store nested maps with multiple attributes per user
//
// if !state.contains(e.user_id) {
//     state[e.user_id] = #{
//         login_count: 0,
//         last_seen: (),
//         errors: [],
//         total_bytes: 0
//     };
// }
//
// let user_state = state[e.user_id];
// user_state.login_count += 1;
// user_state.last_seen = e.timestamp;
// user_state.total_bytes += e.bytes ?? 0;
//
// if e.has("error") {
//     user_state.errors.push(e.error);
// }
//
// state[e.user_id] = user_state;
// e.user_login_count = user_state.login_count;


// ============================================================================
// PATTERN 4: Conditional processing based on previous events
// ============================================================================
//
// Only process events after seeing a specific trigger event
//
// if e.event_type == "session_start" {
//     state["session_active"] = true;
//     state["session_id"] = e.session_id;
// }
//
// if state["session_active"] == true {
//     // Process events within active session
//     e.active_session_id = state["session_id"];
// } else {
//     // Skip events outside session
//     e = ();
// }
//
// if e.event_type == "session_end" {
//     state["session_active"] = false;
// }


// ============================================================================
// PATTERN 5: Convert state to regular map for output
// ============================================================================
//
// Use with --end to print final state:
//   kelora -j logs.jsonl --exec-file examples/state_examples.rhai \
//     --end 'print(state.to_map().to_logfmt())' -q
//
// state[e.level] = (state.get(e.level) ?? 0) + 1;


// ============================================================================
// PATTERN 6: Track first and last occurrence
// ============================================================================
//
// Store both first and last timestamp for each unique ID
//
// if !state.contains(e.request_id) {
//     state[e.request_id] = #{
//         first_seen: e.timestamp,
//         last_seen: e.timestamp,
//         count: 1
//     };
// } else {
//     let req_state = state[e.request_id];
//     req_state.last_seen = e.timestamp;
//     req_state.count += 1;
//     state[e.request_id] = req_state;
// }
//
// e.occurrence_count = state[e.request_id].count;


// ============================================================================
// AVAILABLE STATE OPERATIONS
// ============================================================================
//
// Indexing:
//   state["key"]                 Get/set values (returns () if not exists)
//   state["key"] = value         Set a value
//
// Methods:
//   state.contains("key")        Check if key exists
//   state.get("key")             Get value (returns () if not exists)
//   state.set("key", value)      Set value
//   state.len()                  Number of keys
//   state.is_empty()             True if no keys
//   state.keys()                 Array of all keys
//   state.values()               Array of all values
//   state.clear()                Remove all entries
//   state.remove("key")          Remove specific key
//
// Operators:
//   state += #{k: v}             Merge map into state
//   state.mixin(#{k: v})         Merge map into state (same as +=)
//   state.fill_with(#{k: v})     Replace entire state with map
//
// Conversion:
//   state.to_map()               Convert to regular Rhai map
//                                (needed for to_logfmt(), to_kv(), etc.)
