use std::io::{BufWriter, Read, Result};
use std::path::Path;

use bitstream_io::{BitWriter, LittleEndian};
use image::*;

pub struct ByteReader {
    input: Option<RgbaImage>,
    x: u32,
    y: u32,
    c: usize,
}

impl ByteReader {
    pub fn new(input_file: &str) -> Self {
        ByteReader::of_file(Path::new(input_file))
    }
}

impl ByteReader {
    pub fn of_file(input_file: &Path) -> Self {
        ByteReader::of_image(image::open(input_file)
            .expect("Input image is not readable.")
            .to_rgba())
    }
}

impl ByteReader {
    pub fn of_image(image: RgbaImage) -> Self {
        ByteReader {
            input: Some(image),
            x: 0,
            y: 0,
            c: 0,
        }
    }
}

impl Read for ByteReader {
    fn read(&mut self, b: &mut [u8]) -> Result<usize> {
        #[inline]
        #[cfg(debug_assertions)]
        fn update_progress(total_progress: u32, progress: &mut u8, x: u32, y: u32) {
            let p = ((x * y * 100) / total_progress) as u8;
            if p > *progress {
                *progress = p;
                print!("\rProgress: {}%", p);
                if p == 99 {
                    println!("\rDone                    ");
                }
            }
        }
        #[inline]
        #[cfg(not(debug_assertions))]
        fn update_progress(total_progress: u32, progress: &mut u8, x: u32, y: u32) {
            let p = ((x * y * 100) / total_progress) as u8;
            if p > *progress {
                *progress = p;
            }
        }

        let source_image = self.input.as_ref().unwrap();
        let (width, height) = source_image.dimensions();
        let bytes_to_read = b.len();
        let total_progress = width * height;
        let mut buf_writer = BufWriter::new(b);

        let mut bit_buffer = BitWriter::endian(
            buf_writer,
            LittleEndian,
        );

        let mut progress: u8 = ((self.x * self.y * 100) / total_progress) as u8;
        let mut bits_read = 0;
        let mut bytes_read = 0;
        for x in self.x..width {
            for y in self.y..height {
                let image::Rgba(rgba) = source_image.get_pixel(x, y);
                for c in self.c..3 {
                    if bytes_read >= bytes_to_read {
                        self.x = x;
                        self.y = y;
                        self.c = c;
                        return Ok(bytes_read);
                    }
                    let bit = rgba[c] & 0x01;
                    bit_buffer
                        .write_bit(bit > 0)
                        .unwrap_or_else(|_| panic!("Color {} on Pixel({}, {})", c, x, y));
                    bits_read += 1;

                    if bits_read % 8 == 0 {
                        bytes_read = (bits_read / 8) as usize;
                        update_progress(total_progress, &mut progress, x, y);
                    }
                }
                if self.c > 0 {
                    self.c = 0;
                }
            }
            if self.y > 0 {
                self.y = 0;
            }
        };
        self.x = width;
        if !bit_buffer.byte_aligned() {
            bit_buffer.byte_align();
        }

        return Ok(bytes_read);
    }
}


#[cfg(test)]
mod tests {
    use bitstream_io::{BitWriter, LittleEndian};

    use super::*;

    const H: u8 = b'H';
    const E: u8 = b'e';
    const L: u8 = b'l';
    const O: u8 = b'o';
    const HELLO_WORLD_PNG: &str = "resources/with_text/hello_world.png";
    const CARGO_ZIP_PNG: &str = "resources/with_attachment/contains_one_file.png";
    const TWO_FILES_ZIP_PNG: &str = "resources/with_attachment/contains_two_files.png";

    #[test]
    fn test_read_trait_behaviour_for_read_once() {
        let mut dec = ByteReader::new(HELLO_WORLD_PNG);

        let mut buf = [0 as u8; 13];
        let r = dec.read(&mut buf).unwrap();
        assert_eq!(r, 13, "bytes should have been read");
        assert_eq!(buf[0], 0x1, "1st byte does not match");
        assert_eq!(buf[1], H, "2nd byte is not a 'H'");
        assert_eq!(buf[2], E, "3rd byte is not a 'e'");
        assert_eq!(buf[3], L, "4th byte is not a 'l'");

        println!("{}", std::str::from_utf8(&buf).unwrap());
        assert_eq!(std::str::from_utf8(&buf).unwrap(), "\u{1}Hello World!");
    }

    #[test]
    fn test_read_trait_behaviour_for_read_multiple_times() {
        let mut dec = ByteReader::new(HELLO_WORLD_PNG);

        let mut buf = [0 as u8; 3];
        let r = dec.read(&mut buf).unwrap();
        assert_eq!(r, 3, "bytes should have been read");
        assert_eq!(buf[0], 0x1, "1st byte does not match");
        assert_eq!(buf[1], H, "2nd byte is not a 'H'");
        assert_eq!(buf[2], E, "3rd byte is not a 'e'");
        assert_eq!(std::str::from_utf8(&buf).unwrap(), "\u{1}He");

        let r = dec.read(&mut buf).unwrap();
        assert_eq!(r, 3, "bytes should have been read");
        assert_eq!(buf[0], L, "4th byte is not a 'l'");
        assert_eq!(buf[1], L, "5th byte is not a 'l'");
        assert_eq!(buf[2], O, "6th byte is not a 'o'");
        assert_eq!(std::str::from_utf8(&buf).unwrap(), "llo");
    }

    #[test]
    fn test_read_trait_behaviour_for_read_all() {
        let mut dec = ByteReader::new(HELLO_WORLD_PNG);
        let expected_bytes = ((515 * 443 * 3) / 8) as usize;

        let mut buf = Vec::new();
        let r = dec.read_to_end(&mut buf).unwrap();
        assert_eq!(r, expected_bytes, "bytes should have been read"); // filesize
        assert_eq!(buf[0], 0x1, "1st byte does not match");
        assert_eq!(buf[1], H, "2nd byte is not a 'H'");
        assert_eq!(buf[2], E, "3rd byte is not a 'e'");
    }

    #[test]
    fn should_not_contain_noise_bytes() {
        let mut dec = ByteReader::new(CARGO_ZIP_PNG);
        let expected_bytes = ((515 * 443 * 3) / 8) as usize;
        let zip_file_size = 337;

        let mut buf = Vec::new();
        let r = dec.read_to_end(&mut buf).unwrap();
        assert_eq!(r, expected_bytes, "bytes should have been read"); // filesize

//        use std::fs::File;
//        let mut target = File::create("/tmp/contains_one_file.png.zip")
//            .expect("temp file was not created");
//        target.write_all(&buf[1..]);
//        target.flush();

//        let mut reader = std::io::Cursor::new(&buf[1..]);
//        let mut zip = zip::ZipArchive::new(reader)
//            .expect("zip archive was not readable");
//        for i in 0..zip.len() {
//            let mut file = zip.by_index(i).unwrap();
//            println!("Filename: {}", file.name());
//            let first_byte = file.bytes().next().unwrap()
//                .expect("not able to read next byte");
//            println!("{}", first_byte);
//        }
    }

    #[test]
    fn test_bit_writer() {
        let b = vec![0b0100_1000, 0b0110_0001, 0b0110_1100];
        let mut buf = Vec::with_capacity(3);

        {
            let mut buf_writer = BufWriter::new(&mut buf);
            let mut bit_buffer = BitWriter::endian(
                &mut buf_writer,
                LittleEndian,
            );

            bit_buffer.write_bit((0 & 1) == 1).expect("1 failed");
            bit_buffer.write_bit((0 & 1) == 1).expect("2 failed");
            bit_buffer.write_bit((0 & 1) == 1).expect("3 failed");
            bit_buffer.write_bit((1 & 1) == 1).expect("4 failed");
            bit_buffer.write_bit((0 & 1) == 1).expect("5 failed");
            bit_buffer.write_bit((0 & 1) == 1).expect("6 failed");
            bit_buffer.write_bit((1 & 1) == 1).expect("7 failed");
            bit_buffer.write_bit((0 & 1) == 1).expect("8 failed");
        }

        assert_eq!(*buf.first().unwrap(), H);
    }
}
