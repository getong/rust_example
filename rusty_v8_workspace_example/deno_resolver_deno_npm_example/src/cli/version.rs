pub struct DenoVersionInfo {
    pub deno: &'static str,
}

pub static DENO_VERSION_INFO: DenoVersionInfo = DenoVersionInfo {
    deno: "1.0.0",
};