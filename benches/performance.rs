//! Performance benchmarks for MosaicTerm
//!
//! This file contains comprehensive performance benchmarks to ensure MosaicTerm
//! meets its performance targets for rendering, parsing, and command execution.

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use mosaicterm::models::{CommandBlock, OutputLine};
use mosaicterm::terminal::ansi_parser::AnsiParser;
use mosaicterm::terminal::output::{OutputChunk, OutputProcessor};
use mosaicterm::terminal::StreamType;
use std::path::PathBuf;

// ============================================================================
// ANSI Parsing Benchmarks
// ============================================================================

fn bench_ansi_parsing_simple(c: &mut Criterion) {
    c.bench_function("ansi/parse_simple", |b| {
        b.iter_batched(
            || {
                let mut parser = AnsiParser::new();
                let text = "\x1b[31mRed text\x1b[0m";
                (parser, text)
            },
            |(mut parser, text)| parser.parse(black_box(text)),
            BatchSize::SmallInput,
        );
    });
}

fn bench_ansi_parsing_complex(c: &mut Criterion) {
    c.bench_function("ansi/parse_complex", |b| {
        b.iter_batched(
            || {
                let mut parser = AnsiParser::new();
                let text = "\x1b[1;31mBold Red\x1b[0m \x1b[4;32mUnderline Green\x1b[0m \x1b[38;5;196mBright Red\x1b[0m";
                (parser, text)
            },
            |(mut parser, text)| parser.parse(black_box(text)),
            BatchSize::SmallInput,
        );
    });
}

fn bench_ansi_parsing_large(c: &mut Criterion) {
    c.bench_function("ansi/parse_large", |b| {
        b.iter_batched(
            || {
                let mut parser = AnsiParser::new();
                let text = "Normal text ".repeat(1000) 
                    + "\x1b[31mRed text\x1b[0m " 
                    + &"More text ".repeat(1000);
                (parser, text)
            },
            |(mut parser, text)| parser.parse(black_box(&text)),
            BatchSize::LargeInput,
        );
    });
}

fn bench_ansi_parsing_no_ansi(c: &mut Criterion) {
    c.bench_function("ansi/parse_plain_text", |b| {
        b.iter_batched(
            || {
                let mut parser = AnsiParser::new();
                let text = "Plain text without any ANSI codes".repeat(100);
                (parser, text)
            },
            |(mut parser, text)| parser.parse(black_box(&text)),
            BatchSize::SmallInput,
        );
    });
}

fn bench_ansi_parsing_rgb(c: &mut Criterion) {
    c.bench_function("ansi/parse_rgb_colors", |b| {
        b.iter_batched(
            || {
                let mut parser = AnsiParser::new();
                let text = "\x1b[38;2;255;0;0mRGB Red\x1b[0m \x1b[38;2;0;255;0mRGB Green\x1b[0m";
                (parser, text)
            },
            |(mut parser, text)| parser.parse(black_box(text)),
            BatchSize::SmallInput,
        );
    });
}

// ============================================================================
// Output Processing Benchmarks
// ============================================================================

fn bench_output_processing_simple(c: &mut Criterion) {
    c.bench_function("output/process_simple", |b| {
        b.iter_batched(
            || {
                let mut processor = OutputProcessor::new();
                let chunk = OutputChunk {
                    data: b"Hello, World!\n".to_vec(),
                    timestamp: chrono::Utc::now(),
                    is_complete: false,
                    stream_type: StreamType::Stdout,
                };
                (processor, chunk)
            },
            |(mut processor, chunk)| processor.process_chunk(black_box(chunk)),
            BatchSize::SmallInput,
        );
    });
}

fn bench_output_processing_multiline(c: &mut Criterion) {
    c.bench_function("output/process_multiline", |b| {
        b.iter_batched(
            || {
                let mut processor = OutputProcessor::new();
                let data = (0..100).map(|i| format!("Line {}\n", i)).collect::<String>();
                let chunk = OutputChunk {
                    data: data.as_bytes().to_vec(),
                    timestamp: chrono::Utc::now(),
                    is_complete: false,
                    stream_type: StreamType::Stdout,
                };
                (processor, chunk)
            },
            |(mut processor, chunk)| processor.process_chunk(black_box(chunk)),
            BatchSize::SmallInput,
        );
    });
}

fn bench_output_processing_ansi(c: &mut Criterion) {
    c.bench_function("output/process_ansi", |b| {
        b.iter_batched(
            || {
                let mut processor = OutputProcessor::new();
                let data = (0..50)
                    .map(|i| format!("\x1b[{}mLine {}\x1b[0m\n", 30 + (i % 8), i))
                    .collect::<String>();
                let chunk = OutputChunk {
                    data: data.as_bytes().to_vec(),
                    timestamp: chrono::Utc::now(),
                    is_complete: false,
                    stream_type: StreamType::Stdout,
                };
                (processor, chunk)
            },
            |(mut processor, chunk)| processor.process_chunk(black_box(chunk)),
            BatchSize::SmallInput,
        );
    });
}

fn bench_output_processing_large(c: &mut Criterion) {
    c.bench_function("output/process_large", |b| {
        b.iter_batched(
            || {
                let mut processor = OutputProcessor::new();
                let data = "X".repeat(10000) + "\n";
                let chunk = OutputChunk {
                    data: data.as_bytes().to_vec(),
                    timestamp: chrono::Utc::now(),
                    is_complete: false,
                    stream_type: StreamType::Stdout,
                };
                (processor, chunk)
            },
            |(mut processor, chunk)| processor.process_chunk(black_box(chunk)),
            BatchSize::LargeInput,
        );
    });
}

// ============================================================================
// Command Block Benchmarks
// ============================================================================

fn bench_block_creation(c: &mut Criterion) {
    c.bench_function("block/create", |b| {
        b.iter(|| {
            let block = CommandBlock::new(
                black_box("ls -la".to_string()),
                black_box(PathBuf::from("/tmp")),
            );
            black_box(block);
        });
    });
}

fn bench_block_with_output(c: &mut Criterion) {
    c.bench_function("block/add_output_10_lines", |b| {
        b.iter_batched(
            || CommandBlock::new("ls -la".to_string(), PathBuf::from("/tmp")),
            |mut block| {
                for i in 0..10 {
                    block.add_output_line(OutputLine {
                        text: format!("output line {}", i),
                        ansi_codes: vec![],
                        line_number: i,
                        timestamp: chrono::Utc::now(),
                    });
                }
                black_box(block);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_block_with_large_output(c: &mut Criterion) {
    c.bench_function("block/add_output_1000_lines", |b| {
        b.iter_batched(
            || CommandBlock::new("cat large_file.txt".to_string(), PathBuf::from("/tmp")),
            |mut block| {
                for i in 0..1000 {
                    block.add_output_line(OutputLine {
                        text: format!("output line {} with some content", i),
                        ansi_codes: vec![],
                        line_number: i,
                        timestamp: chrono::Utc::now(),
                    });
                }
                black_box(block);
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_block_get_plain_output(c: &mut Criterion) {
    c.bench_function("block/get_plain_output", |b| {
        b.iter_batched(
            || {
                let mut block = CommandBlock::new("test".to_string(), PathBuf::from("/tmp"));
                for i in 0..100 {
                    block.add_output_line(OutputLine {
                        text: format!("Line {}", i),
                        ansi_codes: vec![],
                        line_number: i,
                        timestamp: chrono::Utc::now(),
                    });
                }
                block
            },
            |block| black_box(block.get_plain_output()),
            BatchSize::SmallInput,
        );
    });
}

// ============================================================================
// Memory and Throughput Benchmarks
// ============================================================================

fn bench_throughput_command_blocks(c: &mut Criterion) {
    c.bench_function("throughput/create_100_blocks", |b| {
        b.iter(|| {
            let blocks: Vec<_> = (0..100)
                .map(|i| CommandBlock::new(format!("command{}", i), PathBuf::from("/tmp")))
                .collect();
            black_box(blocks);
        });
    });
}

fn bench_throughput_parse_ansi(c: &mut Criterion) {
    c.bench_function("throughput/parse_100_ansi_strings", |b| {
        b.iter_batched(
            || {
                let mut parser = AnsiParser::new();
                let strings: Vec<_> = (0..100)
                    .map(|i| format!("\x1b[{}mText {}\x1b[0m", 30 + (i % 8), i))
                    .collect();
                (parser, strings)
            },
            |(mut parser, strings)| {
                for s in strings {
                    let _ = parser.parse(&s);
                }
            },
            BatchSize::SmallInput,
        );
    });
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    ansi_benchmarks,
    bench_ansi_parsing_simple,
    bench_ansi_parsing_complex,
    bench_ansi_parsing_large,
    bench_ansi_parsing_no_ansi,
    bench_ansi_parsing_rgb,
);

criterion_group!(
    output_benchmarks,
    bench_output_processing_simple,
    bench_output_processing_multiline,
    bench_output_processing_ansi,
    bench_output_processing_large,
);

criterion_group!(
    block_benchmarks,
    bench_block_creation,
    bench_block_with_output,
    bench_block_with_large_output,
    bench_block_get_plain_output,
);

criterion_group!(
    throughput_benchmarks,
    bench_throughput_command_blocks,
    bench_throughput_parse_ansi,
);

criterion_main!(
    ansi_benchmarks,
    output_benchmarks,
    block_benchmarks,
    throughput_benchmarks
);
