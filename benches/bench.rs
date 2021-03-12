use criterion::{criterion_group, criterion_main, Criterion};
use crossbeam_channel::IntoIter;
use lib::runner::cache::Cache;
use lib::runner::QueryProducer;
use lib::utils::{Argument, Protocol};

fn bench_producer(c: &mut Criterion) {
    let mut argument = Argument::default();
    argument.max = 10000000000;
    argument.qps = 0;
    let mut producer = QueryProducer::new(argument);
    c.bench_function("producer", |b| {
        b.iter(|| {
            producer.retrieve();
        })
    });
}

fn bench_cache_with_static_id(c: &mut Criterion) {
    let mut argument = Argument::default();
    argument.packet_id = 1220;
    argument.protocol = Protocol::UDP;
    argument.domain = "baidu.com".to_owned();
    argument.qty = "A".to_owned();

    let mut cache = Cache::new(&argument);
    c.bench_function("cache with static id", |b| {
        b.iter(|| {
            let (_, _) = cache.build_message();
        })
    });
}

fn bench_cache_with_dynamic_id(c: &mut Criterion) {
    let mut argument = Argument::default();
    argument.packet_id = 0;
    argument.protocol = Protocol::UDP;
    argument.domain = "baidu.com".to_owned();
    argument.qty = "A".to_owned();

    let mut cache = Cache::new(&argument);
    c.bench_function("cache with dynamic id", |b| {
        b.iter(|| {
            let (_, _) = cache.build_message();
        })
    });
}

fn bench_time_consumer_api_instance(c: &mut Criterion) {
    let start_instance_time = std::time::Instant::now();
    c.bench_function("bench instance duration", |b| {
        b.iter(|| {
            start_instance_time.elapsed().as_secs_f64();
        })
    });
}

fn bench_time_consumer_api_system(c: &mut Criterion) {
    let start_system_time = std::time::SystemTime::now();
    c.bench_function("bench system duration", |b| {
        b.iter(|| {
            start_system_time.elapsed().unwrap().as_secs_f64();
        })
    });
}

fn bench_time_consumer_api_chrono(c: &mut Criterion) {
    let start_time = chrono::Utc::now();
    c.bench_function("bench chrono duration", |b| {
        b.iter(|| (chrono::Utc::now().time() - start_time.time()).num_nanoseconds())
    });
}

criterion_group!(
    benches,
    // bench_producer,
    // bench_cache_with_static_id,
    // bench_cache_with_dynamic_id,
    bench_time_consumer_api_system,
    bench_time_consumer_api_instance,
    bench_time_consumer_api_chrono,
);
criterion_main!(benches);
