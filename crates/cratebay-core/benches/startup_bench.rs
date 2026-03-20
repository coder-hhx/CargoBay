//! Startup / initialization benchmarks.
//!
//! Measures the time to initialize core subsystems as specified in testing-spec.md:
//! - Database open + migrate (cold start)
//! - Storage init with in-memory DB
//! - Validation function throughput

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use cratebay_core::storage;
use cratebay_core::validation;

fn bench_db_init_in_memory(c: &mut Criterion) {
    c.bench_function("db_init_in_memory", |b| {
        b.iter(|| {
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA foreign_keys = ON;
                 PRAGMA busy_timeout = 5000;
                 PRAGMA cache_size = -2000;
                 PRAGMA temp_store = MEMORY;",
            )
            .unwrap();
            storage::migrate(black_box(&conn)).unwrap();
        })
    });
}

fn bench_db_init_file(c: &mut Criterion) {
    c.bench_function("db_init_temp_file", |b| {
        b.iter(|| {
            let dir = tempfile::tempdir().unwrap();
            let db_path = dir.path().join("bench.db");
            let conn = storage::init(black_box(&db_path)).unwrap();
            drop(conn);
        })
    });
}

fn bench_validation_container_name(c: &mut Criterion) {
    c.bench_function("validate_container_name", |b| {
        b.iter(|| {
            let _ = validation::validate_container_name(black_box("my-awesome-container-01"));
        })
    });
}

fn bench_validation_container_name_reject(c: &mut Criterion) {
    c.bench_function("validate_container_name_reject", |b| {
        b.iter(|| {
            let _ = validation::validate_container_name(black_box("../escape-attempt"));
        })
    });
}

fn bench_migration_idempotent(c: &mut Criterion) {
    // After initial migration, re-running should be fast (no-op)
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;",
    )
    .unwrap();
    storage::migrate(&conn).unwrap();

    c.bench_function("migration_idempotent", |b| {
        b.iter(|| {
            storage::migrate(black_box(&conn)).unwrap();
        })
    });
}

criterion_group!(
    benches,
    bench_db_init_in_memory,
    bench_db_init_file,
    bench_validation_container_name,
    bench_validation_container_name_reject,
    bench_migration_idempotent,
);
criterion_main!(benches);
