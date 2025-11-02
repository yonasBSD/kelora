use std::cell::Cell;
use std::hint::black_box;
use std::thread;

use criterion::{criterion_group, criterion_main, Criterion};
use regex::Regex;

use kelora::rhai_functions::micro_search::{ilike_impl, like_impl, matches_impl};

fn bench_like_ascii_prefix(c: &mut Criterion) {
    let haystack = "access.log";
    let pattern = "access*";
    c.bench_function("like_ascii_prefix", |b| {
        b.iter(|| {
            black_box(like_impl(black_box(haystack), black_box(pattern)));
        });
    });
}

fn bench_like_ascii_suffix(c: &mut Criterion) {
    let haystack = "access.log";
    let pattern = "*.log";
    c.bench_function("like_ascii_suffix", |b| {
        b.iter(|| {
            black_box(like_impl(black_box(haystack), black_box(pattern)));
        });
    });
}

fn bench_like_ascii_middle(c: &mut Criterion) {
    let haystack = "user-123-admin";
    let pattern = "user-*-admin";
    c.bench_function("like_ascii_middle", |b| {
        b.iter(|| {
            black_box(like_impl(black_box(haystack), black_box(pattern)));
        });
    });
}

fn bench_like_unicode(c: &mut Criterion) {
    let haystack = "café🚀user";
    let pattern = "café*";
    c.bench_function("like_unicode", |b| {
        b.iter(|| {
            black_box(like_impl(black_box(haystack), black_box(pattern)));
        });
    });
}

fn bench_ilike_unicode_fold(c: &mut Criterion) {
    let haystack = "STRAẞE-user";
    let pattern = "strasse*";
    c.bench_function("ilike_unicode_fold", |b| {
        b.iter(|| {
            black_box(ilike_impl(black_box(haystack), black_box(pattern)));
        });
    });
}

fn bench_like_vs_contains_like(c: &mut Criterion) {
    let haystack = "user-123-admin";
    let pattern = "user-*-admin";
    c.bench_function("like_vs_contains_like", |b| {
        b.iter(|| {
            black_box(like_impl(black_box(haystack), black_box(pattern)));
        });
    });
}

fn bench_like_vs_contains_contains(c: &mut Criterion) {
    let haystack = "user-123-admin";
    let needle = "user-";
    c.bench_function("like_vs_contains_contains", |b| {
        b.iter(|| {
            black_box(haystack.contains(black_box(needle)));
        });
    });
}

fn bench_like_vs_regex_simple(c: &mut Criterion) {
    let haystack = "user-123-admin";
    let regex = Regex::new("^user-.*-admin$").expect("valid regex");
    c.bench_function("like_vs_regex_simple", |b| {
        b.iter(|| {
            black_box(regex.is_match(black_box(haystack)));
        });
    });
}

fn bench_matches_cached(c: &mut Criterion) {
    let haystack = "user not found";
    let pattern = r"user\s+not\s+found";
    c.bench_function("matches_cached", |b| {
        b.iter(|| {
            black_box(matches_impl(black_box(haystack), black_box(pattern)).unwrap());
        });
    });
}

fn bench_matches_dynamic(c: &mut Criterion) {
    let haystack = "user not found";
    let counter = Cell::new(0usize);
    c.bench_function("matches_dynamic", |b| {
        b.iter(|| {
            let idx = counter.get();
            counter.set(idx.wrapping_add(1));
            let pattern = format!(r"user\s+not\s+found\s+{}", idx);
            black_box(matches_impl(black_box(haystack), black_box(&pattern)).unwrap());
        });
    });
}

fn bench_matches_parallel(c: &mut Criterion) {
    let haystack = "user not found";
    let pattern = r"user\s+not\s+found";
    c.bench_function("matches_parallel", |b| {
        b.iter(|| {
            thread::scope(|scope| {
                for _ in 0..4 {
                    scope.spawn(|| {
                        for _ in 0..32 {
                            black_box(matches_impl(haystack, pattern).unwrap());
                        }
                    });
                }
            });
        });
    });
}

criterion_group!(
    micro_search_benches,
    bench_like_ascii_prefix,
    bench_like_ascii_suffix,
    bench_like_ascii_middle,
    bench_like_unicode,
    bench_ilike_unicode_fold,
    bench_like_vs_contains_like,
    bench_like_vs_contains_contains,
    bench_like_vs_regex_simple,
    bench_matches_cached,
    bench_matches_dynamic,
    bench_matches_parallel
);
criterion_main!(micro_search_benches);
