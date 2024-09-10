use ethers_core::types::U256;

pub fn u256_hex(a: &U256) -> String {
    let mut bytes = [0u8; 32];
    a.to_big_endian(&mut bytes);
    hex::encode(bytes)
}

pub fn hex_u256(a: &str) -> U256 {
    let bytes = hex::decode(a).unwrap_or(vec![0u8; 32]);
    U256::from_big_endian(&bytes)
}


fn main() {

    let u256 = hex_u256("4bd6dacf2d5a5c93f410964569cddab068bb038ce976e53818cc032181cb8373");
    println!("u256 is {}", u256);



    let u256 = u256_hex(&U256::from_dec_str("115569590093573465686975422602080333992787915355521131868309842096538601843211").unwrap());
    // "ff8211e81337a50f55a0243002bf8dbe9448c0b60cdcd9f8e59660694726ea0b"
    println!("u256 is {:#?}", u256);


    println!("u256 is {:#?}", hex::decode("115569590093573465686975422602080333992787915355521131868309842096538601843211"));

    // 4bd6dacf2d5a5c93f410964569cddab068bb038ce976e53818cc032181cb8373
    let new_u256 = u256_hex(&U256::from("4bd6dacf2d5a5c93f410964569cddab068bb038ce976e53818cc032181cb8373"));


    println!("new u256 is {:#?}", new_u256);
}
