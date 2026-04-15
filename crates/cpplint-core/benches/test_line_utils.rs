#![feature(portable_simd)]
use std::simd::cmp::SimdPartialEq;
use std::simd::u8x32;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

pub fn get_indent_level_simd(line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i + 32 <= bytes.len() {
        let chunk = u8x32::from_slice(&bytes[i..i + 32]);
        let mask = chunk.simd_eq(u8x32::splat(b' ')).to_bitmask();
        let ones = mask.trailing_ones() as usize;
        count += ones;
        if ones < 32 {
            return count;
        }
        i += 32;
    }
    for &b in &bytes[i..] {
        if b == b' ' {
            count += 1;
        } else {
            break;
        }
    }
    count
}

pub fn get_indent_level_simple(line: &str) -> usize {
    line.as_bytes()
        .iter()
        .take_while(|&&b| b == b' ')
        .count()
}

fn criterion_benchmark(c: &mut Criterion) {
    let line1 = "    hello world";
    let line2 = "                                hello world";
    let line3 = "hello world";
    c.bench_function("simd_short", |b| b.iter(|| get_indent_level_simd(black_box(line1))));
    c.bench_function("simple_short", |b| b.iter(|| get_indent_level_simple(black_box(line1))));

    c.bench_function("simd_long", |b| b.iter(|| get_indent_level_simd(black_box(line2))));
    c.bench_function("simple_long", |b| b.iter(|| get_indent_level_simple(black_box(line2))));

    c.bench_function("simd_none", |b| b.iter(|| get_indent_level_simd(black_box(line3))));
    c.bench_function("simple_none", |b| b.iter(|| get_indent_level_simple(black_box(line3))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
