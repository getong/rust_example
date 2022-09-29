#[derive(Clone, Debug)]
pub struct Context {
    param: String,
    id: u32,
}

impl Context {
    pub fn new(param: String, id: u32) -> Self {
        Context { param, id }
    }
    pub fn call<T, H>(&self, handler: H)
    where
        H: Handler<T>,
    {
        handler.call(self)
    }
}

pub struct Param(pub String);

pub struct Id(pub u32);

pub trait FromContext {
    fn from_context(context: &Context) -> Self;
}

impl FromContext for Param {
    fn from_context(context: &Context) -> Self {
        Param(context.param.clone())
    }
}

impl FromContext for Id {
    fn from_context(context: &Context) -> Self {
        Id(context.id)
    }
}

impl FromContext for Context {
    fn from_context(context: &Context) -> Self {
        context.clone()
    }
}

pub trait Handler<T> {
    fn call(self, context: &Context);
}

#[allow(unused_parens)]
impl<T1, F> Handler<(T1)> for F
where
    F: Fn(T1),
    T1: FromContext,
{
    fn call(self, context: &Context) {
        self(T1::from_context(context));
    }
}

impl<T1, T2, F> Handler<(T1, T2)> for F
where
    F: Fn(T1, T2),
    T1: FromContext,
    T2: FromContext,
{
    fn call(self, context: &Context) {
        self(T1::from_context(context), T2::from_context(context));
    }
}

impl<T1, T2, T3, F> Handler<(T1, T2, T3)> for F
where
    F: Fn(T1, T2, T3),
    T1: FromContext,
    T2: FromContext,
    T3: FromContext,
{
    fn call(self, context: &Context) {
        self(
            T1::from_context(context),
            T2::from_context(context),
            T3::from_context(context),
        );
    }
}

pub fn trigger<T, H>(context: &Context, handler: H)
where
    H: Handler<T>,
{
    handler.call(context);
}
