// copy from https://stackoverflow.com/questions/39482131/is-it-possible-to-use-impl-trait-as-a-functions-return-type-in-a-trait-defini

// 1.67.0-nightly (2022-11-13 e631891f7ad40eac3ef5)
#![feature(type_alias_impl_trait)]
#![feature(return_position_impl_trait_in_trait)]

trait FromTheFuture {
    type Iter: Iterator<Item = u8>;

    fn returns_associated_type(&self) -> Self::Iter;

    // Needs `return_position_impl_trait_in_trait`
    fn returns_impl_trait(&self) -> impl Iterator<Item = u16>;
}

impl FromTheFuture for u8 {
    // Needs `type_alias_impl_trait`
    type Iter = impl Iterator<Item = u8>;

    fn returns_associated_type(&self) -> Self::Iter {
        std::iter::repeat(*self).take(*self as usize)
    }

    fn returns_impl_trait(&self) -> impl Iterator<Item = u16> {
        Some((*self).into()).into_iter()
    }
}

fn main() {
    for v in 7.returns_associated_type() {
        println!("type_alias_impl_trait: {v}");
    }

    for v in 7.returns_impl_trait() {
        println!("return_position_impl_trait_in_trait: {v}");
    }
}
