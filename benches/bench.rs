use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lib::runner::QueryProducer;
use lib::utils::Argument;

fn bench_producer(c: &mut Criterion) {
    let mut argument = Argument::default();
    argument.max = 10000000000;
    let mut producer = QueryProducer::new(argument);

    c.bench_function("producer", |b| b.iter(||{
        producer.retrieve();
    }));
}

criterion_group!(benches, bench_producer);
criterion_main!(benches);