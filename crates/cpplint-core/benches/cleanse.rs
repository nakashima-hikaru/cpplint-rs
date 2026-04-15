use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn bench_cleanse_raw_strings(c: &mut Criterion) {
    let mut raw_lines = vec![];
    for i in 0..100 {
        raw_lines.push(format!("    const char* s{} = R\"(This is a simple raw string)\";", i));
        raw_lines.push(format!("    const char* s2_{} = R\"foo(This is a raw string with delimiter)foo\";", i));
        raw_lines.push(format!("    const char* s3_{} = R\"(This is a multiline\nraw string)foo\";", i));
    }

    c.bench_function("cleanse_raw_strings", |b| {
        b.iter(|| cpplint_core::cleanse::cleanse_raw_strings(black_box(&raw_lines)))
    });
}

criterion_group!(benches, bench_cleanse_raw_strings);
criterion_main!(benches);
