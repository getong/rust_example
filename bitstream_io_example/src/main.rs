use bitstream_io::{BigEndian, BitRead, BitReader};
use bitstream_io::{BitWrite, BitWriter};
use std::io::Write;
use std::io::{Cursor, Read};

fn main() {
    // println!("Hello, world!");

    let flac: Vec<u8> = vec![
        0x66, 0x4C, 0x61, 0x43, 0x00, 0x00, 0x00, 0x22, 0x10, 0x00, 0x10, 0x00, 0x00, 0x06, 0x06,
        0x00, 0x21, 0x62, 0x0A, 0xC4, 0x42, 0xF0, 0x00, 0x04, 0xA6, 0xCC, 0xFA, 0xF2, 0x69, 0x2F,
        0xFD, 0xEC, 0x2D, 0x5B, 0x30, 0x01, 0x76, 0xB4, 0x62, 0x88, 0x7D, 0x92,
    ];

    let mut cursor = Cursor::new(&flac);
    {
        let mut reader = BitReader::endian(&mut cursor, BigEndian);

        // stream marker
        let mut file_header: [u8; 4] = [0, 0, 0, 0];
        reader.read_bytes(&mut file_header).unwrap();
        assert_eq!(&file_header, b"fLaC");

        // metadata block header
        let last_block: bool = reader.read_bit().unwrap();
        let block_type: u8 = reader.read(7).unwrap();
        let block_size: u32 = reader.read(24).unwrap();
        assert_eq!(last_block, false);
        assert_eq!(block_type, 0);
        assert_eq!(block_size, 34);

        // STREAMINFO block
        let minimum_block_size: u16 = reader.read(16).unwrap();
        let maximum_block_size: u16 = reader.read(16).unwrap();
        let minimum_frame_size: u32 = reader.read(24).unwrap();
        let maximum_frame_size: u32 = reader.read(24).unwrap();
        let sample_rate: u32 = reader.read(20).unwrap();
        let channels = reader.read::<u8>(3).unwrap() + 1;
        let bits_per_sample = reader.read::<u8>(5).unwrap() + 1;
        let total_samples: u64 = reader.read(36).unwrap();
        assert_eq!(minimum_block_size, 4096);
        assert_eq!(maximum_block_size, 4096);
        assert_eq!(minimum_frame_size, 1542);
        assert_eq!(maximum_frame_size, 8546);
        assert_eq!(sample_rate, 44100);
        assert_eq!(channels, 2);
        assert_eq!(bits_per_sample, 16);
        assert_eq!(total_samples, 304844);
    }

    // STREAMINFO's MD5 sum

    // Note that the wrapped reader can be used once bitstream reading
    // is finished at exactly the position one would expect.

    let mut md5 = [0; 16];
    cursor.read_exact(&mut md5).unwrap();
    assert_eq!(
        &md5,
        b"\xFA\xF2\x69\x2F\xFD\xEC\x2D\x5B\x30\x01\x76\xB4\x62\x88\x7D\x92"
    );

    let mut flac: Vec<u8> = Vec::new();
    {
        let mut writer = BitWriter::endian(&mut flac, BigEndian);

        // stream marker
        writer.write_bytes(b"fLaC").unwrap();

        // metadata block header
        let last_block: bool = false;
        let block_type: u8 = 0;
        let block_size: u32 = 34;
        writer.write_bit(last_block).unwrap();
        writer.write(7, block_type).unwrap();
        writer.write(24, block_size).unwrap();

        // STREAMINFO block
        let minimum_block_size: u16 = 4096;
        let maximum_block_size: u16 = 4096;
        let minimum_frame_size: u32 = 1542;
        let maximum_frame_size: u32 = 8546;
        let sample_rate: u32 = 44100;
        let channels: u8 = 2;
        let bits_per_sample: u8 = 16;
        let total_samples: u64 = 304844;
        writer.write(16, minimum_block_size).unwrap();
        writer.write(16, maximum_block_size).unwrap();
        writer.write(24, minimum_frame_size).unwrap();
        writer.write(24, maximum_frame_size).unwrap();
        writer.write(20, sample_rate).unwrap();
        writer.write(3, channels - 1).unwrap();
        writer.write(5, bits_per_sample - 1).unwrap();
        writer.write(36, total_samples).unwrap();
    }

    // STREAMINFO's MD5 sum

    // Note that the wrapped writer can be used once bitstream writing
    // is finished at exactly the position one would expect.

    flac.write_all(b"\xFA\xF2\x69\x2F\xFD\xEC\x2D\x5B\x30\x01\x76\xB4\x62\x88\x7D\x92")
        .unwrap();

    assert_eq!(
        flac,
        vec![
            0x66, 0x4C, 0x61, 0x43, 0x00, 0x00, 0x00, 0x22, 0x10, 0x00, 0x10, 0x00, 0x00, 0x06,
            0x06, 0x00, 0x21, 0x62, 0x0A, 0xC4, 0x42, 0xF0, 0x00, 0x04, 0xA6, 0xCC, 0xFA, 0xF2,
            0x69, 0x2F, 0xFD, 0xEC, 0x2D, 0x5B, 0x30, 0x01, 0x76, 0xB4, 0x62, 0x88, 0x7D, 0x92
        ]
    );
}
