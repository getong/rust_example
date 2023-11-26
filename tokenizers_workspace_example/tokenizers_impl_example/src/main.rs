use tokenizers::models::bpe::BpeTrainer;
use tokenizers::models::bpe::BPE;
use tokenizers::pre_tokenizers::whitespace::Whitespace;
use tokenizers::AddedToken;
use tokenizers::DecoderWrapper;
use tokenizers::NormalizerWrapper;
use tokenizers::PostProcessorWrapper;
use tokenizers::PreTokenizerWrapper;
use tokenizers::Tokenizer;
use tokenizers::TokenizerImpl;


fn main() -> tokenizers::Result<()> {
    // println!("Hello, world!");
    let mut tokenizer: TokenizerImpl<
        BPE,
        NormalizerWrapper,
        PreTokenizerWrapper,
        PostProcessorWrapper,
        DecoderWrapper,
    > = TokenizerImpl::new(
        BPE::builder()
            .unk_token("[UNK]".to_string())
            .build()
            .unwrap(),
    );

    let mut trainer = BpeTrainer::builder()
        .special_tokens(vec![
            AddedToken::from("[UNK]", true),
            AddedToken::from("[CLS]", true),
            AddedToken::from("[SEP]", true),
            AddedToken::from("[PAD]", true),
            AddedToken::from("[MASK]", true),
        ])
        .build();

    tokenizer.with_pre_tokenizer(Whitespace {});
    let files = vec![
        "wikitext-103-raw/wiki.train.raw".into(),
        "wikitext-103-raw/wiki.test.raw".into(),
        "wikitext-103-raw/wiki.valid.raw".into(),
    ];
    tokenizer.train_from_files(&mut trainer, files)?;
    tokenizer.save("tokenizer-wiki.json", false)?;
    let tokenizer = Tokenizer::from_file("tokenizer-wiki.json")?;
    let output = tokenizer.encode("Hello, y'all! How are you 😁 ?", true)?;

    println!("{:?}", output.get_tokens());
    // ["Hello", ",", "y", "'", "all", "!", "How", "are", "you", "[UNK]", "?",]

    println!("{:?}", output.get_ids());
    // [27253, 16, 93, 11, 5097, 5, 7961, 5112, 6218, 0, 35]

    println!("{:?}", output.get_offsets()[9]);
    // (26, 30)

    let sentence = "Hello, y'all! How are you 😁 ?";
    println!("{}", &sentence[26..30]);
    // "😁"

    println!("{}", tokenizer.token_to_id("[SEP]").unwrap());
    // 2
    Ok(())
}
