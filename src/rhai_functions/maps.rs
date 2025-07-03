use rhai::{Engine, Map};

/// Registers map merge/enrich operators to the Rhai engine.
pub fn register_functions(engine: &mut Engine) {
    // event.merge(map): overwrites existing keys
    engine.register_fn("merge", |lhs: &mut Map, rhs: Map| {
        for (k, v) in rhs {
            lhs.insert(k, v);
        }
    });

    // event.enrich(map): inserts only if key is missing
    engine.register_fn("enrich", |lhs: &mut Map, rhs: Map| {
        for (k, v) in rhs {
            if !lhs.contains_key(&k) {
                lhs.insert(k, v);
            }
        }
    });

    // event += map: shorthand for merge()
    engine.register_fn("+=", |lhs: &mut Map, rhs: Map| {
        for (k, v) in rhs {
            lhs.insert(k, v);
        }
    });
}
