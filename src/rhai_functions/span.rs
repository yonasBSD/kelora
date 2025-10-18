use chrono::{DateTime, Utc};
use rhai::{Array, Dynamic, Engine, Map};

use crate::event::Event;
use crate::rhai_functions::datetime::DateTimeWrapper;

#[derive(Clone)]
pub struct SpanBinding {
    span_id: String,
    span_start: Option<DateTime<Utc>>,
    span_end: Option<DateTime<Utc>>,
    events: Array,
    size: i64,
    metrics: Map,
}

impl SpanBinding {
    pub fn new(
        span_id: String,
        span_start: Option<DateTime<Utc>>,
        span_end: Option<DateTime<Utc>>,
        events: &[Event],
        metrics: Map,
    ) -> Self {
        let event_maps = events
            .iter()
            .map(event_to_map)
            .map(Dynamic::from)
            .collect::<Array>();

        Self {
            span_id,
            span_start,
            span_end,
            events: event_maps,
            metrics,
            size: events.len() as i64,
        }
    }

    pub fn get_id(&mut self) -> String {
        self.span_id.clone()
    }

    pub fn get_start(&mut self) -> Dynamic {
        match self.span_start {
            Some(dt) => Dynamic::from(DateTimeWrapper::from_utc(dt)),
            None => Dynamic::UNIT,
        }
    }

    pub fn get_end(&mut self) -> Dynamic {
        match self.span_end {
            Some(dt) => Dynamic::from(DateTimeWrapper::from_utc(dt)),
            None => Dynamic::UNIT,
        }
    }

    pub fn get_size(&mut self) -> i64 {
        self.size
    }

    pub fn get_events(&mut self) -> Array {
        self.events.clone()
    }

    pub fn get_metrics(&mut self) -> Map {
        self.metrics.clone()
    }
}

pub fn register_functions(engine: &mut Engine) {
    engine.register_type_with_name::<SpanBinding>("Span");
    engine.register_get("id", SpanBinding::get_id);
    engine.register_get("start", SpanBinding::get_start);
    engine.register_get("end", SpanBinding::get_end);
    engine.register_get("size", SpanBinding::get_size);
    engine.register_get("events", SpanBinding::get_events);
    engine.register_get("metrics", SpanBinding::get_metrics);
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
