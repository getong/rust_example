use optimization::{Func, GradientDescent, Minimizer, NumericalDifferentiation};

fn main() {
    // println!("Hello, world!");
    // numeric version of the Rosenbrock function
    let function = NumericalDifferentiation::new(Func(|x: &[f64]| {
        (1.0 - x[0]).powi(2) + 100.0 * (x[1] - x[0].powi(2)).powi(2)
    }));

    // we use a simple gradient descent scheme
    let minimizer = GradientDescent::new();

    // perform the actual minimization, depending on the task this may
    // take some time, it may be useful to install a log sink to see
    // what's going on
    let solution = minimizer.minimize(&function, vec![-3.0, -4.0]);

    println!(
        "Found solution for Rosenbrock function at f({:?}) = {:?}",
        solution.position, solution.value
    );
}
