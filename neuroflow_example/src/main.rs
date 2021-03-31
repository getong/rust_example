use neuroflow::activators::Type::Tanh;
use neuroflow::data::DataSet;
use neuroflow::io;
use neuroflow::FeedForward;
use rand::Rng;

fn main() {
    let mut rand_generator = rand::thread_rng();
    // defines the number of layers and the number of neurons in each layer
    // first value is the number of neurons in the input layer
    // last value is the number of neurons in the output layer
    let mut neural_net = FeedForward::new(&[1, 7, 8, 8, 7, 1]);
    let mut data = DataSet::new();
    for _x in 1..15000 {
        let val1: f64 = rand_generator.gen();
        let val2: f64 = rand_generator.gen();
        data.push(&[val1], &[val2]);
    }
    neural_net
        .activation(Tanh)
        .learning_rate(0.01)
        .train(&data, 5000);
    let new_val: f64 = rand_generator.gen();
    let check_val = neural_net.calc(&[new_val])[0];
    println!("Calculated value: {}", check_val);
    io::save(&neural_net, "fakecorrelation.flow").unwrap();
    // let mut new_neural: FeedForward = io::load("fakecorrelation.flow").unwrap()
}
