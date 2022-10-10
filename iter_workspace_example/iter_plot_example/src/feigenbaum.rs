use std::{sync::mpsc, thread};

pub fn plot(
    a: f64,
    b: f64,
    dark_iters: usize,
    final_iters: usize,
    delta: f64,
    init_size: usize,
) -> (Vec<f64>, Vec<f64>) {
    let mut res = (vec![], vec![]);
    let cpus = num_cpus::get();
    let interval = (b - a) / cpus as f64;
    let (tx, rx) = mpsc::channel();
    for i in 0..cpus {
        let tx_copy = tx.clone();
        thread::spawn(move || {
            tx_copy
                .send(simulate(
                    a + i as f64 * interval,
                    a + (i + 1) as f64 * interval,
                    dark_iters,
                    final_iters,
                    delta,
                    init_size,
                ))
                .unwrap();
        });
    }
    drop(tx);
    for mut received in rx {
        res.0.append(&mut received.0);
        res.1.append(&mut received.1);
    }
    res
}

fn simulate(
    a: f64,
    b: f64,
    dark_iters: usize,
    final_iters: usize,
    delta: f64,
    init_size: usize,
) -> (Vec<f64>, Vec<f64>) {
    let mut res = (vec![], vec![]);
    let mut r = a;
    while r <= b {
        for x in iterate_for_r(r, dark_iters, final_iters, init_size) {
            res.0.push(r);
            res.1.push(x);
        }
        r += delta;
    }
    res
}

fn iterate_for_r(r: f64, dark_iters: usize, final_iters: usize, init_size: usize) -> Vec<f64> {
    iterate(r, dark_iters, final_iters, create_random_values(init_size))
}

fn create_random_values(size: usize) -> Vec<f64> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen()).collect()
}

fn iterate(r: f64, dark_iters: usize, final_iters: usize, x_inits: Vec<f64>) -> Vec<f64> {
    x_inits
        .iter()
        .map(|x_0| {
            let mut x = *x_0;
            for _ in 0..dark_iters {
                x = logistic(r, x);
            }
            let mut res = vec![];
            for _ in 0..final_iters {
                x = logistic(r, x);
                res.push(x);
            }
            res
        })
        .flatten()
        .collect()
}

fn logistic(r: f64, x: f64) -> f64 {
    r * x * (1.0 - x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iterate() {
        let result = iterate(3.1, 10, 5, vec![0.3, 0.75]);
        println!("{:?}", result);
    }

    #[test]
    fn test_create_random_values() {
        println!("{:?}", create_random_values(10));
    }

    #[test]
    fn test_plot() {
        println!("{:?}", plot(0.0, 1.0, 10, 10, 0.2, 2));
    }
}
