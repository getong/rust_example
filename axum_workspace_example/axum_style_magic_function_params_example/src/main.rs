use magic::{trigger, Context, Id, Param};

mod magic;

fn print_id(id: Id) {
    println!("id is {}", id.0);
}

fn print_param(Param(param): Param) {
    println!("param is {param}");
}

fn print_all(Param(param): Param, Id(id): Id) {
    println!("param is {param}, id is {id}");
}

fn print_all_switched(Id(id): Id, Param(param): Param) {
    println!("param is {param}, id is {id}");
}

fn print_3_arguments(Id(id): Id, c: Context, Param(param): Param) {
    println!("param is {param}, id is {id}, {:?}", c);
}

pub fn main() {
    let context = Context::new("magic".into(), 33);

    println!("context.call:");
    context.call(print_id);
    context.call(print_param);
    context.call(print_all);
    context.call(print_all_switched);
    context.call(print_3_arguments);

    let context = &context;
    println!("\ntrigger:");
    trigger(context, print_id);
    trigger(context, print_param);
    trigger(context, print_all);
    trigger(context, print_all_switched);
    trigger(context,print_3_arguments);
}
