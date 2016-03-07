extern crate gif;

use std::{ffi, borrow, iter, cmp, fs, io, error, path, fmt};
use std::io::Write;

#[derive(Debug)]
enum Err {
    Io(io::Error),
    Gif(gif::DecodingError),
    Usage
}
impl error::Error for Err {
    fn description(&self) -> &str { "welp" }
    fn cause(&self) -> Option<&error::Error> { None }
}
impl From<io::Error> for Err {
    fn from(e: io::Error) -> Err { Err::Io(e) }
}
impl From<gif::DecodingError> for Err {
    fn from(e: gif::DecodingError) -> Err { Err::Gif(e) }
}
impl fmt::Display for Err {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            Err::Io(ref e)
            | Err::Gif(gif::DecodingError::Io(ref e)) => e.fmt(f),
            Err::Gif(gif::DecodingError::Format(s))
            | Err::Gif(gif::DecodingError::Internal(s)) =>
                write!(f, "gif decoding error: {}", s),
            Err::Usage => write!(f, "usage: pass one file.gif, read stdout")
        }
    }
}

fn main() {
    if let Err(e) = go() {
        let _ = writeln!(io::stderr(), "{}", e);
        std::process::exit(-1);
    }

    fn go() -> Result<(), Err> {
        let mut args = std::env::args_os().skip(1);
        let arg = match (args.next(), args.next()) {
            (Some(arg), None) => arg,
            _ => return Err(Err::Usage)
        };

        let path: &path::Path = arg.as_ref();

        let file_name = path::Path::new(
            try!(path.file_name().ok_or(Err::Usage)));

        let ext = file_name.extension().and_then(ffi::OsStr::to_str);
        if ext != Some("gif") { return Err(Err::Usage); }

        try!(process(path));

        Ok(())
    }
}

fn gcd(mut a: u16, mut b: u16) -> u16 {
    loop {
        if b == 0 { return a; }
        let r = a % b;
        a = b;
        b = r;
        if a < b { std::mem::swap(&mut a, &mut b); }
    }
}


fn swap_transparent_palette(bg: usize, palette: &mut [u8]) {
    let p = bg * 3;
    if palette.len() > p + 2 {
        palette.swap(0, p);
        palette.swap(1, p + 1);
        palette.swap(2, p + 2);
    }
}

fn swap<T: PartialEq>(val: T, v1: T, v2:T) -> T {
    if val == v1 {
        v2
    } else if val == v2 {
        v1
    } else {
        val
    }
}

fn swap_transparent(mut frame: gif::Frame) -> gif::Frame {
    let bg = frame.transparent.unwrap_or(0);
    if bg == 0 { return frame; }

    if let Some(palette) = frame.palette.as_mut() {
        swap_transparent_palette(bg as usize, palette);
    }

    for c in frame.buffer.to_mut().iter_mut() {
        *c = swap(*c, bg, 0);
    }

    if let Some(r) = frame.transparent.as_mut() { *r = 0; }

    frame
}

fn process<P: AsRef<path::Path>>(from: P) -> Result<bool, Err> {
    const MIN_DELAY: u16 = 2;
    const ZERO_DELAY: u16 = 10;

    let mut decoder = try!(gif::Decoder::new(
        try!(fs::File::open(from))).read_info());

    let mut frames = Vec::new();

    let mut delay;
    let mut any_different = false;

    if let Some(first_frame) = try!(decoder.read_next_frame()) {
        delay = first_frame.delay;
        frames.push(swap_transparent(first_frame.clone()));
    } else {
        return Ok(false);
    }

    while let Some(frame) = try!(decoder.read_next_frame()) {
        if delay != frame.delay {
            delay = gcd(delay, cmp::max(frame.delay, MIN_DELAY));
            any_different = true;
        }
        frames.push(swap_transparent(frame.clone()));
    }

    if !any_different { return Ok(false); }

    let global_bg = decoder.bg_color().unwrap_or(0);
    let mut global_palette_swapped;
    let global_palette = decoder.global_palette().unwrap_or(&[]);
    let global_palette =
        if global_bg == 0 {
            global_palette
        } else {
            global_palette_swapped = global_palette.to_owned();
            swap_transparent_palette(global_bg, &mut global_palette_swapped);
            global_palette_swapped.as_slice()
        };

    let mut encoder = try!(gif::Encoder::new(io::stdout(),
        decoder.width(), decoder.height(), global_palette));

    try!(gif::SetParameter::set(&mut encoder, gif::Repeat::Infinite));

    if delay < MIN_DELAY { delay = MIN_DELAY; }

    let empty_buf = [0];
    let empty_frame = gif::Frame {
        delay: delay,
        width: 1,
        height: 1,
        transparent: Some(0),
        buffer: borrow::Cow::Borrowed(&empty_buf),
        .. Default::default()
    };

    for mut frame in frames {
        let n;
        if frame.delay < 2 {
            n = (ZERO_DELAY + delay - 1) / delay;
        } else {
            n = (frame.delay + delay - 1) / delay;
        }
        let n = n as usize;
        frame.delay = delay;

        let first_frame;
        let mut i1;
        let mut i2;
        let mut i3;
        let frames: &mut Iterator<Item = &gif::Frame>;
        frames = if n < 3 {
            i1 = iter::repeat(&frame).take(n);
            &mut i1
        } else {
            use gif::DisposalMethod::*;

            match frame.dispose {
                Any | Keep => {
                    i2 = iter::once(&frame)
                        .chain(iter::repeat(&empty_frame))
                        .take(n);
                    &mut i2
                }
                Background => {
                    first_frame = gif::Frame {
                        dispose: Keep,
                        .. frame.clone()
                    };
                    i3 = iter::once(&first_frame)
                        .chain(iter::repeat(&empty_frame))
                        .take(n - 1)
                        .chain(iter::once(&frame));
                    &mut i3
                }
                Previous => {
                    i1 = iter::repeat(&frame).take(n);
                    &mut i1
                }
            }
        };

        for frame in frames {
            try!(encoder.write_frame(frame));
        }
    }

    Ok(true)
}
