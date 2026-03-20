use tch::{nn, nn::Module, nn::ModuleT, nn::OptimizerConfig, Device, Kind, Tensor};

/// Sample a batch of real data: 2D mixture of two Gaussians.
fn sample_real(batch: i64, device: Device) -> Tensor {
    let half = batch / 2;
    let std = 0.5_f64; // use f64 for scalar ops

    // Means as tensors (Float32 is fine; broadcasting will work)
    let mean1 = Tensor::f_from_slice(&[-2.0f32, 0.0])
        .unwrap()
        .to_device(device)
        .view([1, 2]);
    let mean2 = Tensor::f_from_slice(&[2.0f32, 0.0])
        .unwrap()
        .to_device(device)
        .view([1, 2]);

    let x1 = Tensor::randn([half, 2], (Kind::Float, device)) * std + &mean1; // (half,2)
    let x2 = Tensor::randn([batch - half, 2], (Kind::Float, device)) * std + &mean2; // (batch-half,2)
    Tensor::cat(&[x1, x2], 0) // (batch,2)
}

/// Generator: z_dim -> 64 -> 2
fn build_generator(vs: &nn::Path, z_dim: i64) -> nn::Sequential {
    nn::seq()
        .add(nn::linear(vs / "g1", z_dim, 64, Default::default()))
        .add_fn(|xs| xs.relu())
        .add(nn::linear(vs / "g2", 64, 2, Default::default()))
}

/// Discriminator: 2 -> 64 -> 1 (logit)
fn build_discriminator(vs: &nn::Path) -> nn::Sequential {
    nn::seq()
        .add(nn::linear(vs / "d1", 2, 64, Default::default()))
        .add_fn(|xs| xs.leaky_relu()) // no alpha argument in tch 0.18
        .add(nn::linear(vs / "d2", 64, 1, Default::default()))
}

fn main() {
    // Reproducibility + device
    tch::manual_seed(42);
    let device = Device::cuda_if_available();
    println!("Running on: {:?}", device);

    // Separate var stores (so G and D have separate optimizers)
    let vs_g = nn::VarStore::new(device);
    let vs_d = nn::VarStore::new(device);

    // Hyperparams
    let z_dim: i64 = 8;
    let batch: i64 = 128;
    let iters: i64 = 2000;
    let print_every: i64 = 100;

    // Models
    let g = build_generator(&vs_g.root(), z_dim);
    let d = build_discriminator(&vs_d.root());

    // Optimizers
    let mut opt_g = nn::Adam::default().build(&vs_g, 2e-4).unwrap();
    let mut opt_d = nn::Adam::default().build(&vs_d, 2e-4).unwrap();

    for step in 1..=iters {
        // -----------------------------
        // Train Discriminator
        // -----------------------------
        let x_real = sample_real(batch, device); // (B,2)
        let z = Tensor::randn([batch, z_dim], (Kind::Float, device));
        let x_fake = g.forward_t(&z, true); // (B,2)

        let d_real_logits = d.forward_t(&x_real, true); // (B,1)
        let d_fake_logits = d.forward_t(&x_fake.detach(), true); // (B,1)

        // BCE-with-logits (stable): -log σ(a) - log(1-σ(b)) = -logσ(a) - logσ(-b)
        let loss_d_real = -d_real_logits.log_sigmoid().mean(Kind::Float);
        let loss_d_fake = -(-&d_fake_logits).log_sigmoid().mean(Kind::Float);
        let loss_d = &loss_d_real + &loss_d_fake;

        opt_d.backward_step(&loss_d);

        // -----------------------------
        // Train Generator
        // -----------------------------
        let z2 = Tensor::randn([batch, z_dim], (Kind::Float, device));
        let x_fake2 = g.forward_t(&z2, true);
        let d_fake2_logits = d.forward_t(&x_fake2, true);

        // G wants D to predict "real" for fakes: minimize -log σ(D(G(z)))
        let loss_g = -d_fake2_logits.log_sigmoid().mean(Kind::Float);

        opt_g.backward_step(&loss_g);

        if step % print_every == 0 {
            let ld = loss_d.to_device(Device::Cpu).double_value(&[]);
            let lg = loss_g.to_device(Device::Cpu).double_value(&[]);
            println!("step {:4} | d_loss {:.4} | g_loss {:.4}", step, ld, lg);
        }
    }

    // Sample a few points from the trained generator
    let z = Tensor::randn([10, z_dim], (Kind::Float, device));
    let samples = g.forward_t(&z, false).to_device(Device::Cpu); // (10,2)
    let flat: Vec<f32> = samples.view([-1]).try_into().unwrap();
    println!("generated samples (x,y):");
    for i in 0..10 {
        let x = flat[2 * i];
        let y = flat[2 * i + 1];
        println!("  {:>2}: [{:.3}, {:.3}]", i, x, y);
    }
}
