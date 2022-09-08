// use string_obfuscation_example::xor_string;

use rand::prelude::*;
use std::time::Duration;
// mod string_obfuscation_example;

extern crate proc_macro;
extern crate proc_macro2;
extern crate quote;
use proc_macro::{TokenStream, TokenTree};
use proc_macro2::Literal;
use quote::quote;

#[proc_macro]
pub fn xor_string(tokens: TokenStream) -> TokenStream {
    let mut something = String::from("");
    for tok in tokens {
        something = match tok {
            TokenTree::Literal(lit) => lit.to_string(),
            _ => "<&#1>".to_owned(),
        }
    }
    something = String::from(&something[1..something.len() - 1]);
    let mut rng = rand::thread_rng();
    let random_bytes: Vec<u8> = (0..something.as_bytes().len())
        .map(|_| rng.gen::<u8>())
        .collect();
    let obfuscated: Vec<u8> = something
        .as_bytes()
        .iter()
        .zip(&random_bytes)
        .map(|(&a, b)| a ^ b)
        .collect();

    let xor_key = Literal::byte_string(&random_bytes);
    let obfuscated = Literal::byte_string(&obfuscated);
    let result = quote! {
        String::from_utf8(#obfuscated.iter().zip(#xor_key).map(|(&a, b)| a ^ b).collect()).unwrap()
    };
    result.into()
}

fn main() {
    println!("Hello, world!");
    std::thread::sleep(Duration::from_millis(10_000));
    println!("Content: {}", crate::xor_string!("Super_Top_Secret"));
}
