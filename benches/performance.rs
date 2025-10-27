//! Performance benchmarks for MosaicTerm
//!
//! This file contains performance benchmarks to ensure MosaicTerm
//! meets its performance targets for rendering and command execution.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mosaicterm::ansi::AnsiParser;
use mosaicterm::models::CommandBlock;
use std::path::PathBuf;

/// Benchmark ANSI parsing performance
fn bench_ansi_parsing(c: &mut Criterion) {
    let mut parser = AnsiParser::new();
    let test_text = "\x1b[31mRed text\x1b[0m \x1b[1mBold\x1b[0m \x1b[32mGreen\x1b[0m";

    c.bench_function("ansi_parsing", |b| {
        b.iter(|| {
            let _ = parser.parse(black_box(test_text));
        });
    });
}

/// Benchmark command block creation
fn bench_block_creation(c: &mut Criterion) {
    c.bench_function("block_creation", |b| {
        b.iter(|| {
            let mut block = CommandBlock::new(
                black_box("ls -la".to_string()),
                black_box(PathBuf::from("/tmp")),
            );

            // Add some output lines
            for i in 0..10 {
                block.add_output_line(mosaicterm::models::OutputLine {
                    text: format!("output line {}", i),
                    ansi_codes: vec![],
                    line_number: i,
                    timestamp: chrono::Utc::now(),
                });
            }

            black_box(block);
        });
    });
}

/// Benchmark large text processing
fn bench_large_text_processing(c: &mut Criterion) {
    let mut parser = AnsiParser::new();
    let large_text =
        "Normal text ".repeat(1000) + "\x1b[31mRed text\x1b[0m " + &"More text ".repeat(1000);

    c.bench_function("large_text_processing", |b| {
        b.iter(|| {
            let _ = parser.parse(black_box(&large_text));
        });
    });
}

criterion_group!(
    benches,
    bench_ansi_parsing,
    bench_block_creation,
    bench_large_text_processing
);
criterion_main!(benches);
