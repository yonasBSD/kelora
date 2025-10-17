use std::cell::RefCell;

use chrono::{DateTime, Utc};
use rhai::{Array, Dynamic, Engine, EvalAltResult, Map, Position};

use crate::event::Event;
use crate::rhai_functions::datetime::DateTimeWrapper;

#[derive(Clone)]
pub struct SpanContextData {
    span_id: Option<String>,
    span_start: Option<DateTime<Utc>>,
    span_end: Option<DateTime<Utc>>,
    events: Vec<Map>,
    size: usize,
    metrics: Map,
}

impl SpanContextData {
    pub fn new(
        span_id: String,
        span_start: Option<DateTime<Utc>>,
        span_end: Option<DateTime<Utc>>,
        events: &[Event],
        metrics: Map,
    ) -> Self {
        let event_maps = events.iter().map(event_to_map).collect::<Vec<_>>();

        Self {
            span_id: Some(span_id),
            span_start,
            span_end,
            size: event_maps.len(),
            events: event_maps,
            metrics,
        }
    }
}

thread_local! {
    static ACTIVE_CONTEXT: RefCell<Option<SpanContextData>> = const { RefCell::new(None) };
}

pub fn set_span_context(ctx: SpanContextData) {
    ACTIVE_CONTEXT.with(|slot| {
        *slot.borrow_mut() = Some(ctx);
    });
}

pub fn clear_span_context() {
    ACTIVE_CONTEXT.with(|slot| {
        slot.borrow_mut().take();
    });
}

pub fn register_functions(engine: &mut Engine) {
    engine.register_fn("span_start", span_start);
    engine.register_fn("span_end", span_end);
    engine.register_fn("span_id", span_id);
    engine.register_fn("span_size", span_size);
    engine.register_fn("span_events", span_events);
    engine.register_fn("span_metrics", span_metrics);
}

fn span_start() -> Result<Dynamic, Box<EvalAltResult>> {
    with_context(|ctx| match ctx.span_start {
        Some(dt) => Ok(Dynamic::from(DateTimeWrapper::from_utc(dt))),
        None => Ok(Dynamic::UNIT),
    })
}

fn span_end() -> Result<Dynamic, Box<EvalAltResult>> {
    with_context(|ctx| match ctx.span_end {
        Some(dt) => Ok(Dynamic::from(DateTimeWrapper::from_utc(dt))),
        None => Ok(Dynamic::UNIT),
    })
}

fn span_id() -> Result<String, Box<EvalAltResult>> {
    with_context(|ctx| Ok(ctx.span_id.clone().unwrap_or_default()))
}

fn span_size() -> Result<i64, Box<EvalAltResult>> {
    with_context(|ctx| Ok(ctx.size as i64))
}

fn span_events() -> Result<Array, Box<EvalAltResult>> {
    with_context(|ctx| {
        let array = ctx
            .events
            .iter()
            .cloned()
            .map(Dynamic::from)
            .collect::<Array>();
        Ok(array)
    })
}

fn span_metrics() -> Result<Map, Box<EvalAltResult>> {
    with_context(|ctx| Ok(ctx.metrics.clone()))
}

fn with_context<F, R>(f: F) -> Result<R, Box<EvalAltResult>>
where
    F: FnOnce(&SpanContextData) -> Result<R, Box<EvalAltResult>>,
{
    ACTIVE_CONTEXT.with(|slot| {
        let guard = slot.borrow();
        if let Some(ref ctx) = *guard {
            f(ctx)
        } else {
            Err(Box::new(EvalAltResult::ErrorRuntime(
                "span_* helpers are only available inside --span-close".into(),
                Position::NONE,
            )))
        }
    })
}

fn event_to_map(event: &Event) -> Map {
    let mut map = Map::new();

    for (k, v) in &event.fields {
        map.insert(k.clone().into(), v.clone());
    }

    map.insert("line".into(), Dynamic::from(event.original_line.clone()));

    if let Some(line_num) = event.line_num {
        map.insert("line_num".into(), Dynamic::from(line_num as i64));
    }

    if let Some(filename) = &event.filename {
        map.insert("filename".into(), Dynamic::from(filename.clone()));
    }

    if let Some(status) = event.span.status {
        map.insert("span_status".into(), Dynamic::from(status.as_str()));
    }

    if let Some(span_id) = &event.span.span_id {
        map.insert("span_id".into(), Dynamic::from(span_id.clone()));
    }

    if let Some(span_start) = event.span.span_start {
        map.insert(
            "span_start".into(),
            Dynamic::from(DateTimeWrapper::from_utc(span_start)),
        );
    }

    if let Some(span_end) = event.span.span_end {
        map.insert(
            "span_end".into(),
            Dynamic::from(DateTimeWrapper::from_utc(span_end)),
        );
    }

    map
}
