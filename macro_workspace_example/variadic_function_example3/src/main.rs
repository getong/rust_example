pub struct Ui {}

pub trait Component<Params, Content> {
    fn call(&self, ui: &mut Ui, params: Params, content: Content);
}

macro_rules! impl_component {
    ($($P:ident),*) => {
        impl<F, $($P,)* Content> $crate::Component<( $($P,)* ), Content> for F
            where F: Fn(&mut Ui, $($P,)* Content),
                  $( $P: ::std::cmp::PartialEq + ::std::clone::Clone + 'static, )*
                  Content: ::std::ops::FnOnce(&mut Ui),
        {

            fn call(&self, ui: &mut Ui, params: ( $($P,)* ), content: Content) {
                #[allow(non_snake_case)]
                let ($($P,)*) = params;
                self(ui, $($P,)* content)
            }
        }
    };
}

impl_component!();
impl_component!(P1);
impl_component!(P1, P2);
impl_component!(P1, P2, P3);
impl_component!(P1, P2, P3, P4);

pub fn memoize<Params, Content, Comp>(
    ui: &mut Ui,
    component: Comp,
    params: Params,
    content: Content,
) where
    Params: PartialEq + Clone + 'static,
    Content: FnOnce(&mut Ui),
    Comp: Component<Params, Content>,
{
    component.call(ui, params, content);
}

fn comp1(ui: &mut Ui, a: u8, f: impl FnOnce(&mut Ui)) {
    println!("a:{:?}", a);
    f(ui);
}
fn comp_(ui: &mut Ui, a: &str, f: impl FnOnce(&mut Ui)) {
    println!("a:{:?}", a);
    f(ui);
}
fn comp2(ui: &mut Ui, a: u8, b: u32, f: impl FnOnce(&mut Ui)) {
    println!("a:{:?}, b: {:?}", a, b);
    f(ui);
}
fn comp3(ui: &mut Ui, a: u8, b: u32, c: u64, f: impl FnOnce(&mut Ui)) {
    println!("a:{:?}, b: {:?}, c: {:?}", a, b, c);
    f(ui);
}
fn comp4(ui: &mut Ui, a: u8, b: u32, c: u64, d: usize, f: impl FnOnce(&mut Ui)) {
    println!("a:{:?}, b: {:?}, c: {:?}, d: {:?}", a, b, c, d);
    f(ui);
}

fn main() {
    let mut ui = Ui {};
    memoize(&mut ui, comp1, (1,), |_| {});
    memoize(&mut ui, comp_, ("",), |_| {});
    memoize(&mut ui, comp2, (2, 3), |_| {});
    memoize(&mut ui, comp3, (1, 2, 3), |_| {});
    memoize(&mut ui, comp4, (0, 1, 2, 3), |_| {});
}
