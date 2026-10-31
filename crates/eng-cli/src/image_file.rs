//! PNG and animated PNG files for `shot` and `video` (plan difference 5),
//! made with the `png` crate. The files are deterministic: the same pixels
//! give the same bytes. Each holds 8-bit RGBA as the output stores it, an
//! `sRGB` chunk saying so, and nothing else: no time, text or path.
//!
//! An animated PNG plays once per loop, forever; every frame replaces the
//! whole image and shows for the same time, a whole number of ticks.

use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;

use eng_gpu::Image;

/// What every file shares: 8-bit RGBA, sRGB-encoded, and fixed compression
/// settings, so a newer default in `png` cannot change the bytes.
fn encoder<W: Write>(out: W, width: u32, height: u32) -> png::Encoder<'static, W> {
    let mut encoder = png::Encoder::new(out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    encoder.set_compression(png::Compression::Balanced);
    encoder
}

/// A video's errors (`[E-VIDEO]`); a still image's are `[E-IMAGE]`.
fn png_error(error: png::EncodingError) -> String {
    format!("[E-VIDEO] {error}")
}

/// A still image as a PNG file.
pub(crate) fn encode_png(image: &Image) -> Result<Vec<u8>, String> {
    let still = |error: png::EncodingError| format!("[E-IMAGE] {error}");
    let mut out = Vec::new();
    let mut writer = encoder(&mut out, image.width, image.height)
        .write_header()
        .map_err(still)?;
    writer.write_image_data(&image.rgba8).map_err(still)?;
    writer.finish().map_err(still)?;
    Ok(out)
}

/// How long a frame shows when it lasts `ticks` ticks at `tick_rate`, as
/// APNG's fraction of a second (16-bit numerator and denominator), reduced.
pub(crate) fn frame_delay(ticks: u64, tick_rate: u32) -> Result<(u16, u16), String> {
    let (mut a, mut b) = (ticks, u64::from(tick_rate));
    while b != 0 {
        (a, b) = (b, a % b);
    }
    let divisor = a.max(1);
    let (num, den) = (ticks / divisor, u64::from(tick_rate) / divisor);
    match (u16::try_from(num), u16::try_from(den)) {
        (Ok(num), Ok(den)) if num > 0 && den > 0 => Ok((num, den)),
        _ => Err(format!(
            "a frame delay of {ticks} ticks at {tick_rate} Hz does not fit APNG's \
             16-bit fraction of a second"
        )),
    }
}

/// The bytes the encoder writes, shared so they can be taken back after
/// `finish` consumes the writer.
#[derive(Clone, Default)]
struct Shared(Rc<RefCell<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// An animated PNG built one frame at a time, in memory, so a run that
/// stops early leaves no partial file.
pub(crate) struct Apng {
    writer: png::Writer<Shared>,
    bytes: Shared,
    size: (u32, u32),
    frames: u32,
    left: u32,
}

impl Apng {
    /// A video of `frames` frames of `width` by `height` pixels, each shown
    /// for `delay` (seconds as a fraction, from [`frame_delay`]).
    pub fn new(width: u32, height: u32, frames: u32, delay: (u16, u16)) -> Result<Self, String> {
        let bytes = Shared::default();
        let mut encoder = encoder(bytes.clone(), width, height);
        encoder.set_animated(frames, 0).map_err(png_error)?;
        encoder
            .set_frame_delay(delay.0, delay.1)
            .map_err(png_error)?;
        encoder.validate_sequence(true);
        let writer = encoder.write_header().map_err(png_error)?;
        Ok(Self {
            writer,
            bytes,
            size: (width, height),
            frames,
            left: frames,
        })
    }

    /// Adds the next frame.
    pub fn push(&mut self, image: &Image) -> Result<(), String> {
        if (image.width, image.height) != self.size {
            return Err(format!(
                "[E-VIDEO] a {}x{} frame in a {}x{} video",
                image.width, image.height, self.size.0, self.size.1
            ));
        }
        if self.left == 0 {
            return Err("[E-VIDEO] more frames than the video was made for".into());
        }
        self.writer
            .write_image_data(&image.rgba8)
            .map_err(png_error)?;
        self.left -= 1;
        Ok(())
    }

    /// The file, once every frame is in.
    pub fn finish(self) -> Result<Vec<u8>, String> {
        if self.left > 0 {
            return Err(format!(
                "[E-VIDEO] {} of {} frames are missing",
                self.left, self.frames
            ));
        }
        self.writer.finish().map_err(png_error)?;
        Ok(self.bytes.0.take())
    }
}

#[cfg(test)]
mod tests {
    use eng_gpu::Image;

    use super::{Apng, encode_png, frame_delay};

    /// A 3x2 image whose every byte differs, so a swapped channel, row or
    /// column shows.
    fn image(seed: u8) -> Image {
        Image {
            width: 3,
            height: 2,
            rgba8: (0..24u8)
                .map(|i| i.wrapping_mul(11).wrapping_add(seed))
                .collect(),
        }
    }

    /// The chunk types in a PNG file, in order.
    fn chunks(bytes: &[u8]) -> Vec<String> {
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "the PNG signature");
        let mut at = 8;
        let mut names = Vec::new();
        while at < bytes.len() {
            let len = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
            names.push(String::from_utf8(bytes[at + 4..at + 8].to_vec()).unwrap());
            at += 12 + len;
        }
        assert_eq!(at, bytes.len(), "the last chunk ends the file");
        names
    }

    fn decode_png(bytes: &[u8]) -> Image {
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let mut rgba8 = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut rgba8).unwrap();
        assert_eq!(
            (info.color_type, info.bit_depth),
            (png::ColorType::Rgba, png::BitDepth::Eight)
        );
        rgba8.truncate(info.buffer_size());
        Image {
            width: info.width,
            height: info.height,
            rgba8,
        }
    }

    #[test]
    fn a_png_holds_the_pixels() {
        let bytes = encode_png(&image(1)).unwrap();
        assert_eq!(decode_png(&bytes), image(1));
    }

    #[test]
    fn the_same_pixels_give_the_same_bytes() {
        let first = encode_png(&image(5)).unwrap();
        assert_eq!(first, encode_png(&image(5)).unwrap());
        assert_ne!(first, encode_png(&image(6)).unwrap());
        // No time, text or other environment in the file: sRGB says how to
        // read the pixels, and nothing else is added.
        assert_eq!(chunks(&first), ["IHDR", "sRGB", "IDAT", "IEND"]);
    }

    #[test]
    fn an_apng_holds_every_frame_and_its_delay() {
        let frames = [image(1), image(2), image(3)];
        let mut apng = Apng::new(3, 2, 3, (1, 30)).unwrap();
        for frame in &frames {
            apng.push(frame).unwrap();
        }
        let bytes = apng.finish().unwrap();
        assert_eq!(
            chunks(&bytes),
            [
                "IHDR", "sRGB", "acTL", "fcTL", "IDAT", "fcTL", "fdAT", "fcTL", "fdAT", "IEND"
            ]
        );
        let mut reader = png::Decoder::new(std::io::Cursor::new(&bytes))
            .read_info()
            .unwrap();
        let control = reader.info().animation_control.unwrap();
        assert_eq!((control.num_frames, control.num_plays), (3, 0), "loops");
        let mut rgba8 = vec![0; reader.output_buffer_size().unwrap()];
        for frame in &frames {
            reader.next_frame(&mut rgba8).unwrap();
            let fctl = reader.info().frame_control.unwrap();
            assert_eq!((fctl.delay_num, fctl.delay_den), (1, 30));
            assert_eq!(
                (fctl.width, fctl.height, fctl.x_offset, fctl.y_offset),
                (3, 2, 0, 0)
            );
            assert_eq!(fctl.dispose_op, png::DisposeOp::None);
            assert_eq!(fctl.blend_op, png::BlendOp::Source);
            assert_eq!(rgba8, frame.rgba8);
        }
    }

    #[test]
    fn the_same_frames_give_the_same_apng() {
        let make = || {
            let mut apng = Apng::new(3, 2, 2, (1, 60)).unwrap();
            apng.push(&image(7)).unwrap();
            apng.push(&image(8)).unwrap();
            apng.finish().unwrap()
        };
        assert_eq!(make(), make());
    }

    #[test]
    fn an_apng_gets_exactly_its_frames() {
        let mut short = Apng::new(3, 2, 2, (1, 60)).unwrap();
        short.push(&image(1)).unwrap();
        let err = short.finish().unwrap_err();
        assert_eq!(err, "[E-VIDEO] 1 of 2 frames are missing");

        let mut long = Apng::new(3, 2, 1, (1, 60)).unwrap();
        long.push(&image(1)).unwrap();
        let err = long.push(&image(2)).unwrap_err();
        assert!(err.starts_with("[E-VIDEO] "), "{err}");

        let mut wrong = Apng::new(3, 2, 1, (1, 60)).unwrap();
        let other = Image {
            width: 2,
            height: 3,
            rgba8: vec![0; 24],
        };
        let err = wrong.push(&other).unwrap_err();
        assert!(err.starts_with("[E-VIDEO] "), "{err}");

        assert!(Apng::new(3, 2, 0, (1, 60)).is_err(), "no frames");
    }

    #[test]
    fn a_frame_lasts_its_ticks() {
        assert_eq!(frame_delay(1, 60), Ok((1, 60)));
        assert_eq!(frame_delay(2, 60), Ok((1, 30)));
        assert_eq!(frame_delay(90, 60), Ok((3, 2)));
        assert_eq!(frame_delay(70_000, 60), Ok((3_500, 3)));
        for (ticks, rate) in [(65_537, 60), (1, 65_537), (0, 60), (1, 0)] {
            let err = frame_delay(ticks, rate).unwrap_err();
            assert!(err.contains("frame delay"), "{ticks}/{rate}: {err}");
        }
    }
}
