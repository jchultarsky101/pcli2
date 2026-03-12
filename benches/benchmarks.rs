//! Benchmarks for PCLI2
//!
//! Run with: cargo bench

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pcli2::model::normalize_path;

fn bench_normalize_path(c: &mut Criterion) {
    c.bench_function("normalize_path_basic", |b| {
        b.iter(|| normalize_path(black_box("/Root/Folder/Asset.stl")))
    });

    c.bench_function("normalize_path_with_home", |b| {
        b.iter(|| normalize_path(black_box("~/projects/physna/models/part.stl")))
    });

    c.bench_function("normalize_path_consecutive_slashes", |b| {
        b.iter(|| normalize_path(black_box("/Root//Folder///Asset.stl")))
    });

    c.bench_function("normalize_path_trailing_slash", |b| {
        b.iter(|| normalize_path(black_box("/Root/Folder/")))
    });

    c.bench_function("normalize_path_root", |b| {
        b.iter(|| normalize_path(black_box("/")))
    });
}

fn bench_format_parsing(c: &mut Criterion) {
    use pcli2::format::{OutputFormat, OutputFormatOptions};

    #[allow(clippy::result_large_err)]
    c.bench_function("parse_format_json", |b| {
        b.iter(|| {
            OutputFormat::from_string_with_options(
                black_box("json"),
                OutputFormatOptions::default(),
            )
        })
    });

    #[allow(clippy::result_large_err)]
    c.bench_function("parse_format_csv", |b| {
        b.iter(|| {
            OutputFormat::from_string_with_options(black_box("csv"), OutputFormatOptions::default())
        })
    });

    #[allow(clippy::result_large_err)]
    c.bench_function("parse_format_tree", |b| {
        b.iter(|| {
            OutputFormat::from_string_with_options(
                black_box("tree"),
                OutputFormatOptions::default(),
            )
        })
    });
}

criterion_group!(benches, bench_normalize_path, bench_format_parsing,);

criterion_main!(benches);
