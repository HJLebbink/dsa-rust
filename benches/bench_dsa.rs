// Intel Data Streaming Accelerator (DSA) Rust Bindings
// Copyright 2025 Henk-Jan Lebbink
// SPDX-License-Identifier: MIT

//! Benchmarks comparing DSA hardware acceleration vs software implementations.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

/// Benchmark CRC32 computation: DSA vs crc32fast.
fn bench_crc32(c: &mut Criterion) {
    let sizes: Vec<usize> = vec![
        1024,            // 1 KB
        4 * 1024,        // 4 KB
        16 * 1024,       // 16 KB
        64 * 1024,       // 64 KB
        256 * 1024,      // 256 KB
        1024 * 1024,     // 1 MB
        4 * 1024 * 1024, // 4 MB
    ];

    let mut group = c.benchmark_group("crc32");

    for size in sizes {
        let data: Vec<u8> = (0..size).map(|i| (i & 0xFF) as u8).collect();

        group.throughput(Throughput::Bytes(size as u64));

        // Software baseline using crc32fast
        group.bench_with_input(BenchmarkId::new("crc32fast", size), &data, |b, data| {
            b.iter(|| crc32fast::hash(data));
        });

        // DSA hardware (only if available)
        #[cfg(target_os = "linux")]
        {
            if let Ok(engine) = intel_dsa::DsaEngine::open_first() {
                group.bench_with_input(BenchmarkId::new("dsa", size), &data, |b, data| {
                    b.iter(|| engine.crc32(data).unwrap());
                });
            }
        }
    }

    group.finish();
}

/// Benchmark memory copy: DSA vs std::ptr::copy_nonoverlapping.
fn bench_memcpy(c: &mut Criterion) {
    let sizes: Vec<usize> = vec![
        4 * 1024,        // 4 KB
        64 * 1024,       // 64 KB
        1024 * 1024,     // 1 MB
        4 * 1024 * 1024, // 4 MB
    ];

    let mut group = c.benchmark_group("memcpy");

    for size in sizes {
        let src: Vec<u8> = (0..size).map(|i| (i & 0xFF) as u8).collect();
        let mut dst_software = vec![0u8; size];
        let _dst_dsa = vec![0u8; size];

        group.throughput(Throughput::Bytes(size as u64));

        // Software baseline
        group.bench_with_input(BenchmarkId::new("std_copy", size), &src, |b, src| {
            b.iter(|| unsafe {
                std::ptr::copy_nonoverlapping(src.as_ptr(), dst_software.as_mut_ptr(), src.len());
            });
        });

        // DSA hardware (only if available)
        #[cfg(target_os = "linux")]
        {
            if let Ok(engine) = intel_dsa::DsaEngine::open_first() {
                group.bench_with_input(BenchmarkId::new("dsa", size), &src, |b, src| {
                    b.iter(|| engine.memcpy(&mut dst_dsa, src).unwrap());
                });
            }
        }
    }

    group.finish();
}

/// Benchmark memory compare: DSA vs slice comparison.
fn bench_memcmp(c: &mut Criterion) {
    let sizes: Vec<usize> = vec![
        4 * 1024,    // 4 KB
        64 * 1024,   // 64 KB
        1024 * 1024, // 1 MB
    ];

    let mut group = c.benchmark_group("memcmp");

    for size in sizes {
        let a: Vec<u8> = (0..size).map(|i| (i & 0xFF) as u8).collect();
        let b = a.clone();

        group.throughput(Throughput::Bytes(size as u64 * 2)); // Reading both buffers

        // Software baseline
        group.bench_with_input(
            BenchmarkId::new("slice_eq", size),
            &(&a, &b),
            |bench, (a, b)| {
                bench.iter(|| a == b);
            },
        );

        // DSA hardware (only if available)
        #[cfg(target_os = "linux")]
        {
            if let Ok(engine) = intel_dsa::DsaEngine::open_first() {
                group.bench_with_input(
                    BenchmarkId::new("dsa", size),
                    &(&a, &b),
                    |bench, (a, b)| {
                        bench.iter(|| engine.memcmp(a, b).unwrap());
                    },
                );
            }
        }
    }

    group.finish();
}

criterion_group!(benches, bench_crc32, bench_memcpy, bench_memcmp);
criterion_main!(benches);
