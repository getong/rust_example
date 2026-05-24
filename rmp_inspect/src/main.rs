use std::{
  env,
  error::Error,
  ffi::{OsStr, OsString},
  fmt,
  io::{self, Read},
  path::{Path, PathBuf},
  process,
};

use rmp::{Marker, decode::*};

fn main() {
  let action = match parse_args(env::args_os().skip(1)) {
    Ok(Action::Help) => {
      print_usage();
      return;
    }
    Ok(Action::Dump(path)) => path,
    Err(err) => {
      eprintln!("error: {err}\n");
      print_usage_to_stderr();
      process::exit(2);
    }
  };

  if let Err(err) = dump_file(&action) {
    eprintln!("error: {err}");
    process::exit(1);
  }
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
  Dump(PathBuf),
  Help,
}

#[derive(Debug, PartialEq, Eq)]
enum CliError {
  TooManyArguments { unexpected: OsString },
}

impl fmt::Display for CliError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::TooManyArguments { unexpected } => {
        write!(
          f,
          "unexpected extra argument: {}",
          unexpected.to_string_lossy()
        )
      }
    }
  }
}

impl Error for CliError {}

#[derive(Debug)]
struct ReadFileError {
  path: PathBuf,
  source: io::Error,
}

impl fmt::Display for ReadFileError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "failed to read MessagePack file {}: {}",
      self.path.display(),
      self.source
    )
  }
}

impl Error for ReadFileError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    Some(&self.source)
  }
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Action, CliError> {
  let mut args = args.into_iter();
  let Some(path) = args.next() else {
    return Ok(Action::Dump(default_sample_path()));
  };

  if is_help_arg(&path) {
    return Ok(Action::Help);
  }

  if let Some(unexpected) = args.next() {
    return Err(CliError::TooManyArguments { unexpected });
  }

  Ok(Action::Dump(PathBuf::from(path)))
}

fn is_help_arg(arg: &OsStr) -> bool {
  arg == OsStr::new("-h") || arg == OsStr::new("--help")
}

fn default_sample_path() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/sample.msgpack")
}

fn print_usage() {
  println!("{}", usage());
}

fn print_usage_to_stderr() {
  eprintln!("{}", usage());
}

fn usage() -> &'static str {
  "Usage: rmp_inspect [PATH]\n\nInspect MessagePack bytes from PATH.\nWhen PATH is omitted, \
   examples/sample.msgpack is used."
}

fn dump_file(path: &Path) -> Result<(), Box<dyn Error>> {
  let data = std::fs::read(path).map_err(|source| -> Box<dyn Error> {
    Box::new(ReadFileError {
      path: path.to_path_buf(),
      source,
    })
  })?;
  dump(&mut Indent { i: 0, start: true }, &mut data.as_slice())?;
  println!();
  Ok(())
}

fn dump(indent: &mut Indent, rd: &mut &[u8]) -> Result<(), Box<dyn std::error::Error>> {
  match read_marker(rd).map_err(ValueReadError::from)? {
    Marker::FixPos(n) => print!("U0({n})"),
    Marker::FixNeg(n) => print!("I0({n})"),
    Marker::Null => print!("Null"),
    Marker::True => print!("True"),
    Marker::False => print!("False"),
    Marker::U8 => print!("U8({})", rd.read_data_u8()?),
    Marker::U16 => print!("U16({})", rd.read_data_u16()?),
    Marker::U32 => print!("U32({})", rd.read_data_u32()?),
    Marker::U64 => print!("U64({})", rd.read_data_u64()?),
    Marker::I8 => print!("I8({})", rd.read_data_i8()?),
    Marker::I16 => print!("I16({})", rd.read_data_i16()?),
    Marker::I32 => print!("I32({})", rd.read_data_i32()?),
    Marker::I64 => print!("I64({})", rd.read_data_i64()?),
    Marker::F32 => print!("F32({})", rd.read_data_f32()?),
    Marker::F64 => print!("F64({})", rd.read_data_f64()?),
    Marker::FixStr(len) => print!("Str0(\"{}\")", read_str_data(len.into(), rd)?),
    Marker::Str8 => print!(
      "Str8(\"{}\")",
      read_str_data(rd.read_data_u8()?.into(), rd)?
    ),
    Marker::Str16 => print!(
      "Str16(\"{}\")",
      read_str_data(rd.read_data_u16()?.into(), rd)?
    ),
    Marker::Str32 => print!("Str32(\"{}\")", read_str_data(rd.read_data_u32()?, rd)?),
    Marker::Bin8 => print!(
      "Bin8({})",
      HexDump(&read_bin_data(rd.read_data_u8()?.into(), rd)?)
    ),
    Marker::Bin16 => print!(
      "Bin16({})",
      HexDump(&read_bin_data(rd.read_data_u16()?.into(), rd)?)
    ),
    Marker::Bin32 => print!(
      "Bin32({})",
      HexDump(&read_bin_data(rd.read_data_u32()?, rd)?)
    ),
    Marker::FixArray(len) => dump_array(indent, 0, len.into(), rd)?,
    Marker::Array16 => dump_array(indent, 16, rd.read_data_u16()?.into(), rd)?,
    Marker::Array32 => dump_array(indent, 32, rd.read_data_u32()?, rd)?,
    Marker::FixMap(len) => dump_map(indent, 0, len.into(), rd)?,
    Marker::Map16 => dump_map(indent, 16, rd.read_data_u16()?.into(), rd)?,
    Marker::Map32 => dump_map(indent, 32, rd.read_data_u32()?, rd)?,
    Marker::FixExt1 => return Err(unsupported_marker("FixExt1")),
    Marker::FixExt2 => return Err(unsupported_marker("FixExt2")),
    Marker::FixExt4 => return Err(unsupported_marker("FixExt4")),
    Marker::FixExt8 => return Err(unsupported_marker("FixExt8")),
    Marker::FixExt16 => return Err(unsupported_marker("FixExt16")),
    Marker::Ext8 => return Err(unsupported_marker("Ext8")),
    Marker::Ext16 => return Err(unsupported_marker("Ext16")),
    Marker::Ext32 => return Err(unsupported_marker("Ext32")),
    Marker::Reserved => return Err(unsupported_marker("Reserved")),
  }
  Ok(())
}

fn unsupported_marker(marker: &'static str) -> Box<dyn Error> {
  io::Error::new(
    io::ErrorKind::InvalidData,
    format!("unsupported MessagePack marker: {marker}"),
  )
  .into()
}

fn dump_map(
  indent: &mut Indent,
  ty: u8,
  len: u32,
  rd: &mut &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
  indent.print(format_args!("Map{ty}{{"));
  let multiline = len > 1;
  if multiline {
    indent.ln();
  } else {
    print!(" ")
  }
  indent.ind();
  for i in 0 .. len {
    indent.print("");
    dump(indent, rd)?;
    print!(": ");
    dump(indent, rd)?;
    if multiline {
      print!(",");
      indent.ln();
    } else if i + 1 != len {
      print!(", ")
    }
  }
  indent.out();
  indent.print(format_args!("}}"));
  Ok(())
}

fn dump_array(
  indent: &mut Indent,
  ty: u8,
  len: u32,
  rd: &mut &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
  indent.print(format_args!("Array{ty}["));
  let multiline = len > 1;
  if multiline {
    indent.ln();
  } else {
    print!(" ")
  }
  indent.ind();
  for i in 0 .. len {
    indent.print("");
    dump(indent, rd)?;
    if multiline {
      print!(",");
      indent.ln();
    } else if i + 1 != len {
      print!(", ")
    }
  }
  indent.out();
  indent.print("]");
  Ok(())
}

fn read_str_data<R: Read>(len: u32, rd: &mut R) -> Result<String, io::Error> {
  Ok(String::from_utf8_lossy(&read_bin_data(len, rd)?).into_owned())
}

fn read_bin_data<R: Read>(len: u32, rd: &mut R) -> Result<Vec<u8>, io::Error> {
  let mut buf = Vec::with_capacity(len.min(1 << 16) as usize);
  let bytes_read = rd.take(u64::from(len)).read_to_end(&mut buf)?;
  if bytes_read != len as usize {
    return Err(io::ErrorKind::UnexpectedEof.into());
  }
  Ok(buf)
}

struct Indent {
  i: u16,
  start: bool,
}
impl Indent {
  fn print(&mut self, args: impl fmt::Display) {
    print!(
      "{:w$}{args}",
      "",
      w = if self.start { (self.i as usize) * 2 } else { 0 }
    );
    self.start = false;
  }

  pub const fn ind(&mut self) {
    self.i += 1;
  }

  pub fn ln(&mut self) {
    println!();
    self.start = true;
  }

  pub const fn out(&mut self) {
    self.i -= 1;
  }
}

struct HexDump<'a>(&'a [u8]);
impl fmt::Display for HexDump<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let truncate = self.0.len() > 50;
    if truncate {
      f.write_fmt(format_args!("{}B ", self.0.len()))?;
    }

    for &b in &self.0[0 .. (if truncate { 50 } else { self.0.len() })] {
      f.write_fmt(format_args!("{b:02x}"))?;
    }

    if truncate {
      f.write_str("…")?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn no_args_use_sample_file() {
    assert_eq!(parse_args([]).unwrap(), Action::Dump(default_sample_path()));
  }

  #[test]
  fn help_arg_prints_usage() {
    assert_eq!(
      parse_args([OsString::from("--help")]).unwrap(),
      Action::Help
    );
    assert_eq!(parse_args([OsString::from("-h")]).unwrap(), Action::Help);
  }

  #[test]
  fn path_arg_is_dumped() {
    assert_eq!(
      parse_args([OsString::from("data.msgpack")]).unwrap(),
      Action::Dump(PathBuf::from("data.msgpack"))
    );
  }

  #[test]
  fn extra_args_are_rejected() {
    assert_eq!(
      parse_args([OsString::from("data.msgpack"), OsString::from("extra")]),
      Err(CliError::TooManyArguments {
        unexpected: OsString::from("extra")
      })
    );
  }
}
