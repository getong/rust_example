use std::{
  io::{self, Read, Write},
  time::Instant,
};

use brotli::{
  Allocator, BrotliCompress, BrotliCompressCustomIoCustomDict, BrotliDecompress,
  BrotliDecompressCustomIoCustomDict, CompressorWriter, Decompressor, DecompressorWriter,
  HeapAlloc, IoReaderWrapper, IoWriterWrapper, SliceWrapperMut,
  enc::{
    BrotliEncoderParams, StandardAlloc,
    backward_references::BrotliEncoderMode,
    encode::{BrotliEncoderOperation, BrotliEncoderStateStruct},
    interface::{InputPair, InputReferenceMut, PredictionModeContextMap, StaticCommand},
  },
};

/// 打印压缩统计信息
fn report(label: &str, original: usize, compressed: usize) {
  println!(
    "  {:<34} {:>8} -> {:>8} 字节 ({:.1}%)",
    label,
    original,
    compressed,
    compressed as f64 / original as f64 * 100.0
  );
}

/// 方式一：一次性（one-shot）压缩 / 解压，适合数据已完整在内存中的场景
fn one_shot(data: &[u8]) {
  let params = BrotliEncoderParams {
    // quality: 0-11，越大压缩率越高但越慢；11 是默认最高质量
    quality: 11,
    // lgwin: 滑动窗口大小的 log2，范围 10-24，越大压缩率越高但耗内存越多
    lgwin: 22,
    ..Default::default()
  };

  let mut compressed = Vec::new();
  BrotliCompress(&mut &data[..], &mut compressed, &params).expect("压缩失败");

  let mut decompressed = Vec::new();
  BrotliDecompress(&mut &compressed[..], &mut decompressed).expect("解压失败");

  assert_eq!(data, decompressed.as_slice());
  println!("[one-shot]");
  report("原始 -> 压缩", data.len(), compressed.len());
  println!();
}

/// 方式二：流式压缩 / 解压，适合大文件或网络流，不需要一次性载入全部数据
fn streaming(data: &[u8]) {
  // CompressorWriter 包装任意 Write，写入的数据会被边写边压缩
  let mut compressed = Vec::new();
  {
    // 参数：底层 writer、内部缓冲区大小、quality、lgwin
    let mut writer = CompressorWriter::new(&mut compressed, 4096, 9, 22);
    // 模拟分块写入（例如从文件或 socket 逐块读取）
    for chunk in data.chunks(1024) {
      writer.write_all(chunk).expect("写入失败");
    }
    // writer 在 drop 时会自动 flush 并写入结束标记
  }

  // Decompressor 包装任意 Read，读取时边读边解压
  let mut decompressed = Vec::new();
  let mut reader = Decompressor::new(&compressed[..], 4096);
  reader.read_to_end(&mut decompressed).expect("解压失败");

  assert_eq!(data, decompressed.as_slice());
  println!("[流式 writer/reader]");
  report("原始 -> 压缩", data.len(), compressed.len());
  println!();
}

/// 方式三：按内容类型选择编码模式（mode），针对数据特征做专门优化
fn compression_modes(data: &[u8]) {
  println!("[编码模式 mode]");
  println!("  说明：GENERIC 通用二进制；TEXT 文本/HTML/JSON/XML；FONT 字体文件(WOFF2 等)");
  let modes = [
    (BrotliEncoderMode::BROTLI_MODE_GENERIC, "GENERIC"),
    (BrotliEncoderMode::BROTLI_MODE_TEXT, "TEXT"),
    (BrotliEncoderMode::BROTLI_MODE_FONT, "FONT"),
  ];
  for (mode, name) in modes {
    let params = BrotliEncoderParams {
      mode,
      ..Default::default()
    };
    let mut compressed = Vec::new();
    BrotliCompress(&mut &data[..], &mut compressed, &params).expect("压缩失败");
    report(name, data.len(), compressed.len());
  }
  println!();
}

/// 方式四：质量等级权衡（quality 0-11），展示压缩率与耗时的关系
fn quality_sweep(data: &[u8]) {
  println!("[质量等级 quality 0-11]");
  println!("  说明：quality 越高压缩率越好但越慢；0-4 高速、5-8 平衡、9-11 高压缩率");
  for q in [0, 1, 2, 4, 5, 6, 8, 9, 10, 11] {
    let params = BrotliEncoderParams {
      quality: q,
      ..Default::default()
    };
    let mut compressed = Vec::new();
    let start = Instant::now();
    BrotliCompress(&mut &data[..], &mut compressed, &params).expect("压缩失败");
    let elapsed = start.elapsed();
    println!(
      "  q={:<2}   {:>8} -> {:>8} 字节 ({:.1}%)   耗时 {:?}",
      q,
      data.len(),
      compressed.len(),
      compressed.len() as f64 / data.len() as f64 * 100.0,
      elapsed
    );
  }
  println!();
}

/// 方式五：预提供大小提示（size_hint），让编码器提前分配合适的窗口
fn size_hint(data: &[u8]) {
  println!("[大小提示 size_hint]");
  println!("  说明：Web 服务若已知 Content-Length，传入 size_hint 可略微提升压缩率");

  let params = BrotliEncoderParams {
    quality: 5,
    ..Default::default()
  };
  let mut without = Vec::new();
  BrotliCompress(&mut &data[..], &mut without, &params).expect("压缩失败");

  let params = BrotliEncoderParams {
    quality: 5,
    size_hint: data.len(),
    ..Default::default()
  };
  let mut with = Vec::new();
  BrotliCompress(&mut &data[..], &mut with, &params).expect("压缩失败");

  report("无 size_hint", data.len(), without.len());
  report("有 size_hint", data.len(), with.len());
  println!();
}

/// 方式六：自定义共享字典，适合大量包含共同词表的"小"数据（如 HTTP 响应、JSON 消息）
fn custom_dictionary() {
  println!("[自定义共享字典 custom dictionary]");
  println!("  说明：对一小簇彼此相似的小数据（如 API 响应）预训练一份共享字典，可显著减小体积");

  // 共享字典：可视为所有 payload 中共同的"模板"前缀
  let dict = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ";
  let payloads: &[&[u8]] = &[
    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 128\r\n{\"name\":\"alice\",\"online\":true,\"msg\":\"hello brotli\"}",
    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 96\r\n{\"name\":\"bob\",\"online\":false,\"msg\":\"goodbye\"}",
    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 64\r\n{\"name\":\"carol\",\"online\":true}",
  ];

  let params = BrotliEncoderParams {
    quality: 5,
    ..Default::default()
  };

  for payload in payloads {
    // 带字典压缩
    let mut with_dict = Vec::new();
    BrotliCompressCustomIoCustomDict(
      &mut IoReaderWrapper(&mut &payload[..]),
      &mut IoWriterWrapper(&mut with_dict),
      &mut [0u8; 4096],
      &mut [0u8; 4096],
      &params,
      StandardAlloc::default(),
      &mut |_: &mut PredictionModeContextMap<InputReferenceMut>,
            _: &mut [StaticCommand],
            _: InputPair,
            _: &mut StandardAlloc| (),
      dict,
      io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF"),
    )
    .expect("压缩失败");

    // 不带字典压缩（对照）
    let mut no_dict = Vec::new();
    BrotliCompress(&mut &payload[..], &mut no_dict, &params).expect("压缩失败");

    // 解压时必须传入同样的字典
    let mut alloc = HeapAlloc::default();
    let mut dict_mem = alloc.alloc_cell(dict.len());
    dict_mem.slice_mut().copy_from_slice(dict);
    let mut decompressed = Vec::new();
    BrotliDecompressCustomIoCustomDict(
      &mut IoReaderWrapper(&mut &with_dict[..]),
      &mut IoWriterWrapper(&mut decompressed),
      &mut [0u8; 4096],
      &mut [0u8; 4096],
      HeapAlloc::default(),
      HeapAlloc::default(),
      HeapAlloc::default(),
      dict_mem,
      io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF"),
    )
    .expect("解压失败");

    assert_eq!(*payload, decompressed.as_slice());
    let saved = if no_dict.len() > with_dict.len() {
      no_dict.len() - with_dict.len()
    } else {
      0
    };
    println!(
      "  payload {} 字节: 无字典 {:>4} -> 带字典 {:>4} 字节 (省 {} 字节)",
      payload.len(),
      no_dict.len(),
      with_dict.len(),
      saved
    );
  }
  println!();
}

/// 方式七：增量式编码器，可逐块喂入数据，并支持中途刷出（FLUSH）部分结果
fn incremental(data: &[u8]) {
  println!("[增量式编码器 incremental]");
  println!("  说明：适合 WebSocket/HTTP chunked 等需要边收边发、中途可 FLUSH 部分字节的场景");

  let params = BrotliEncoderParams {
    quality: 6,
    ..Default::default()
  };

  // 创建带显式状态的编码器（可同时用于多种自定义 allocator）
  let mut state = BrotliEncoderStateStruct::new(StandardAlloc::default());
  state.params = params.clone();

  let mut compressed = Vec::new();
  let mut out_buf = [0u8; 4096];
  let mut total_out = Some(0usize);
  let mut flushed_sizes = Vec::new();
  let mut nop = |_: &mut PredictionModeContextMap<InputReferenceMut>,
                 _: &mut [StaticCommand],
                 _: InputPair,
                 _: &mut StandardAlloc| ();

  // 逐块喂入数据（模拟收到一段就处理一段）
  for chunk in data.chunks(1024) {
    let mut available_in = chunk.len();
    let mut next_in_offset = 0usize;
    let mut available_out = out_buf.len();
    let mut next_out_offset = 0usize;
    state
      .compress_stream(
        BrotliEncoderOperation::BROTLI_OPERATION_PROCESS,
        &mut available_in,
        chunk,
        &mut next_in_offset,
        &mut available_out,
        &mut out_buf,
        &mut next_out_offset,
        &mut total_out,
        &mut nop,
      )
      .then_some(())
      .expect("压缩失败");
    compressed.extend_from_slice(&out_buf[.. next_out_offset]);
    assert_eq!(next_in_offset, chunk.len(), "应消费完当前分块");
  }

  // 显式 FLUSH：把内部缓冲的已编码字节全部刷出，用于"先发一段再说"
  let mut available_in = 0usize;
  let mut next_in_offset = 0usize;
  let mut available_out = out_buf.len();
  let mut next_out_offset = 0usize;
  state
    .compress_stream(
      BrotliEncoderOperation::BROTLI_OPERATION_FLUSH,
      &mut available_in,
      &[],
      &mut next_in_offset,
      &mut available_out,
      &mut out_buf,
      &mut next_out_offset,
      &mut total_out,
      &mut nop,
    )
    .then_some(())
    .expect("flush 失败");
  if next_out_offset > 0 {
    compressed.extend_from_slice(&out_buf[.. next_out_offset]);
    flushed_sizes.push(next_out_offset);
  }

  // 显式 FINISH：告知编码器数据已结束，写入收尾字节
  loop {
    let mut available_in = 0usize;
    let mut next_in_offset = 0usize;
    let mut available_out = out_buf.len();
    let mut next_out_offset = 0usize;
    state
      .compress_stream(
        BrotliEncoderOperation::BROTLI_OPERATION_FINISH,
        &mut available_in,
        &[],
        &mut next_in_offset,
        &mut available_out,
        &mut out_buf,
        &mut next_out_offset,
        &mut total_out,
        &mut nop,
      )
      .then_some(())
      .expect("finish 失败");
    compressed.extend_from_slice(&out_buf[.. next_out_offset]);
    if state.is_finished() {
      break;
    }
  }

  let mut decompressed = Vec::new();
  let mut reader = Decompressor::new(&compressed[..], 4096);
  reader.read_to_end(&mut decompressed).expect("解压失败");
  assert_eq!(data, decompressed.as_slice());

  report("原始 -> 压缩(增量)", data.len(), compressed.len());
  println!("  FLUSH 刷出的字节数: {:?}", flushed_sizes);
  println!();
}

/// 方式八：用 DecompressorWriter 边读压缩流边把解压结果写进任意 Writer
fn decompress_to_writer(data: &[u8]) {
  println!("[边读边解压写入 DecompressorWriter]");
  println!("  说明：把压缩字节流直接写入 DecompressorWriter，底层 Writer 得到解压后的数据");

  let params = BrotliEncoderParams {
    quality: 9,
    ..Default::default()
  };
  let mut compressed = Vec::new();
  BrotliCompress(&mut &data[..], &mut compressed, &params).expect("压缩失败");

  // 模拟"下载压缩包 → 边下载边解压落盘"
  let mut out = Vec::new();
  {
    let mut writer = DecompressorWriter::new(&mut out, 4096);
    writer.write_all(&compressed).expect("写入失败");
    writer.close().expect("close 失败");
  }
  assert_eq!(data, out.as_slice());

  report("压缩流 -> 解压结果", compressed.len(), out.len());
  println!();
}

fn main() {
  // 构造一段有重复模式的文本，便于展示压缩效果
  let text = "Brotli 是 Google 开发的通用无损压缩算法，结合了 LZ77、Huffman \
              编码和二阶上下文建模。\n"
    .repeat(200);
  let data = text.as_bytes();

  one_shot(data);
  streaming(data);
  compression_modes(data);
  quality_sweep(data);
  size_hint(data);
  custom_dictionary();
  incremental(data);
  decompress_to_writer(data);
}
