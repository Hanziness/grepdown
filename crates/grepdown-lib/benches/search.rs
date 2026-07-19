use criterion::{Criterion, criterion_group, criterion_main};
use grepdown_lib::MDDBProject;
use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().expect("grepdown should be nested inside a workspace")
        .parent().expect("workspace root should exist")
        .to_path_buf()
}

fn bench_root() -> PathBuf {
    workspace_root().join("target/bench-data/rust")
}

fn db_path() -> PathBuf {
    bench_root().join("md.db")
}

const SEARCH_LIMIT: usize = 10;

const QUERIES: &[&str] = &[
    "iterator",
    "unsafe",
    "async*",
    "lifetime borrow",
    "trait OR impl",
];

fn delete_db() {
    let db = db_path();
    if db.exists() {
        fs::remove_file(&db).expect("failed to delete benchmark DB");
    }
}

fn bench_refresh_initial(c: &mut Criterion) {
    let mut group = c.benchmark_group("refresh");

    group.bench_function("initial", |b| {
        b.iter_batched(
            || {
                delete_db();
                MDDBProject::new(bench_root()).unwrap()
            },
            |project| {
                project.refresh().unwrap();
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_refresh_noop(c: &mut Criterion) {
    let mut group = c.benchmark_group("refresh");

    // Ensure DB is indexed before benchmarking
    let project = MDDBProject::new(bench_root()).unwrap();
    project.refresh().unwrap();

    group.bench_function("noop", |b| {
        b.iter(|| {
            project.refresh().unwrap();
        });
    });

    group.finish();
}

fn bench_search_cold(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");

    for query in QUERIES {
        let query = *query;
        group.bench_function(format!("cold/{}", query), |b| {
            b.iter_batched(
                || {
                    delete_db();
                    let project = MDDBProject::new(bench_root()).unwrap();
                    project.refresh().unwrap();
                    project
                },
                |project| {
                    project.search(query, SEARCH_LIMIT, None).unwrap();
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_search_warm(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");

    // Ensure DB is indexed before benchmarking
    let project = MDDBProject::new(bench_root()).unwrap();
    project.refresh().unwrap();

    for query in QUERIES {
        group.bench_function(format!("warm/{}", query), |b| {
            b.iter(|| {
                project.search(query, SEARCH_LIMIT, None).unwrap();
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_refresh_initial,
    bench_refresh_noop,
    bench_search_cold,
    bench_search_warm
);
criterion_main!(benches);
