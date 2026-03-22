//! SQLite storage layer benchmarks.
//!
//! Measures insert and query performance as specified in testing-spec.md:
//! - Message insert throughput
//! - Query 1000 messages (<50ms budget)

use cratebay_core::storage;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Create an in-memory database with schema applied.
fn setup_db() -> rusqlite::Connection {
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
    storage::migrate(&conn).unwrap();
    conn
}

fn bench_message_insert(c: &mut Criterion) {
    let conn = setup_db();
    let conv_id = "bench-conv-1";
    storage::create_conversation(&conn, conv_id, "Benchmark Conversation").unwrap();

    let mut counter = 0u64;

    c.bench_function("insert_message", |b| {
        b.iter(|| {
            counter += 1;
            let msg_id = format!("msg-{counter}");
            storage::save_message(
                black_box(&conn),
                black_box(&msg_id),
                black_box(conv_id),
                black_box("user"),
                black_box("Hello, world! This is a benchmark message."),
                black_box(None),
                black_box(None),
                black_box(None),
                black_box(None),
                black_box(None),
                black_box(counter as i32),
            )
            .unwrap();
        })
    });
}

fn bench_message_query(c: &mut Criterion) {
    let conn = setup_db();
    let conv_id = "bench-conv-query";
    storage::create_conversation(&conn, conv_id, "Query Benchmark").unwrap();

    // Insert 1000 messages
    for i in 0..1000 {
        let msg_id = format!("qmsg-{i}");
        storage::save_message(
            &conn,
            &msg_id,
            conv_id,
            "user",
            &format!("Message {i}: The quick brown fox jumps over the lazy dog"),
            None,
            None,
            None,
            None,
            None,
            i,
        )
        .unwrap();
    }

    c.bench_function("query_1000_messages", |b| {
        b.iter(|| {
            let detail = storage::get_conversation(black_box(&conn), black_box(conv_id)).unwrap();
            assert_eq!(detail.messages.len(), 1000);
        })
    });
}

fn bench_conversation_create(c: &mut Criterion) {
    let conn = setup_db();
    let mut counter = 0u64;

    c.bench_function("create_conversation", |b| {
        b.iter(|| {
            counter += 1;
            let id = format!("conv-bench-{counter}");
            storage::create_conversation(
                black_box(&conn),
                black_box(&id),
                black_box("Benchmark Conversation"),
            )
            .unwrap();
        })
    });
}

fn bench_list_conversations(c: &mut Criterion) {
    let conn = setup_db();

    // Create 100 conversations
    for i in 0..100 {
        let id = format!("list-conv-{i}");
        storage::create_conversation(&conn, &id, &format!("Conversation {i}")).unwrap();
    }

    c.bench_function("list_100_conversations", |b| {
        b.iter(|| {
            let convos =
                storage::list_conversations(black_box(&conn), black_box(100), black_box(0))
                    .unwrap();
            assert_eq!(convos.len(), 100);
        })
    });
}

criterion_group!(
    benches,
    bench_message_insert,
    bench_message_query,
    bench_conversation_create,
    bench_list_conversations,
);
criterion_main!(benches);
