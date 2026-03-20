use std::f64;
use tch::{nn, Device, Kind, Tensor};

struct MHSA {
    w_q: nn::Linear,
    w_k: nn::Linear,
    w_v: nn::Linear,
    w_o: nn::Linear,
    n_heads: i64,
    d_model: i64,
    d_head: i64,
}

impl MHSA {
    fn new(vs: &nn::Path, d_model: i64, n_heads: i64) -> Self {
        assert!(
            d_model % n_heads == 0,
            "d_model must be divisible by n_heads"
        );
        let d_head = d_model / n_heads;

        let linear_cfg = nn::LinearConfig {
            bias: true,
            ..Default::default()
        };

        let w_q = nn::linear(vs / "w_q", d_model, d_model, linear_cfg);
        let w_k = nn::linear(vs / "w_k", d_model, d_model, linear_cfg);
        let w_v = nn::linear(vs / "w_v", d_model, d_model, linear_cfg);
        let w_o = nn::linear(vs / "w_o", d_model, d_model, linear_cfg);

        Self {
            w_q,
            w_k,
            w_v,
            w_o,
            n_heads,
            d_model,
            d_head,
        }
    }

    // xs: (B, T, d_model) -> output: (B, T, d_model)
    fn forward_t(&self, xs: &Tensor, train: bool) -> Tensor {
        // 1) Linear projections
        let q = xs.apply_t(&self.w_q, train); // (B,T,d)
        let k = xs.apply_t(&self.w_k, train); // (B,T,d)
        let v = xs.apply_t(&self.w_v, train); // (B,T,d)

        // 2) Reshape + split heads: (B,T,d) -> (B, nH, T, dH)
        let (b, t, _d) = (xs.size()[0], xs.size()[1], xs.size()[2]);
        let q = self.split_heads(&q, b, t); // (B,nH,T,dH)
        let k = self.split_heads(&k, b, t); // (B,nH,T,dH)
        let v = self.split_heads(&v, b, t); // (B,nH,T,dH)

        // 3) Scaled dot-product attention
        // scores: (B,nH,T,T) = (B,nH,T,dH) x (B,nH,dH,T)
        let scale = (self.d_head as f64).sqrt();
        let scores = q.matmul(&k.transpose(-2, -1)) / scale;
        let attn = scores.softmax(-1, Kind::Float); // softmax over keys (last dim)

        // 4) Weighted sum of values: (B,nH,T,dH)
        let context = attn.matmul(&v);

        // 5) Concatenate heads: (B,T,nH*dH=d_model)
        let concat = self.combine_heads(&context, b, t);

        // 6) Output projection: (B,T,d_model)
        concat.apply_t(&self.w_o, train)
    }

    // (B,T,d) -> (B,nH,T,dH)
    fn split_heads(&self, x: &Tensor, b: i64, t: i64) -> Tensor {
        x.view([b, t, self.n_heads, self.d_head]) // (B,T,nH,dH)
            .transpose(1, 2) // (B,nH,T,dH)
    }

    // (B,nH,T,dH) -> (B,T,d_model)
    fn combine_heads(&self, x: &Tensor, b: i64, t: i64) -> Tensor {
        x.transpose(1, 2) // (B,T,nH,dH)
            .contiguous()
            .view([b, t, self.n_heads * self.d_head]) // (B,T,d)
    }
}

fn main() -> tch::Result<()> {
    let vs = nn::VarStore::new(Device::Cpu);
    let root = &vs.root();

    let d_model = 64;
    let n_heads = 8;
    let mhsa = MHSA::new(&(root / "mhsa"), d_model, n_heads);

    // Dummy batch: B=2, T=5, d_model=64
    let xs = Tensor::randn([2, 5, d_model], (Kind::Float, Device::Cpu));

    let out = mhsa.forward_t(&xs, /*train=*/ true);
    println!("out shape: {:?}", out.size()); // should be [2, 5, 64]
    Ok(())
}
