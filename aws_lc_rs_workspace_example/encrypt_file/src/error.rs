use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
  #[error("I/O 错误：{0}")]
  Io(#[from] std::io::Error),
  #[error("密码不能为空；无密码密钥请显式使用 --no-password")]
  PasswordRequired,
  #[error("两次密码不一致")]
  PasswordMismatch,
  #[error("密码最多 1024 字节")]
  PasswordTooLong,
  #[error("密码输入已结束或取消")]
  InputCancelled,
  #[error("密码输入失败：{0}")]
  Input(String),
  #[error("密钥密码错误或密钥文件被修改")]
  KeyUnlockFailed,
  #[error("解密认证失败：密钥不匹配或密文被修改")]
  AuthenticationFailed,
  #[error("发送方签名无效或验签公钥不匹配")]
  SignatureInvalid,
  #[error("需要发送方签名与可信验签公钥；旧无签名文件须显式使用 --allow-unsigned-legacy")]
  SignatureRequired,
  #[error("不支持的格式版本：{0}")]
  UnsupportedVersion(u8),
  #[error("不支持的算法套件：{0}")]
  UnsupportedSuite(u8),
  #[error("文件格式损坏、长度错误或密钥类型不匹配")]
  Corrupt,
  #[error("Argon2id 参数超出支持范围")]
  KdfParameters,
  #[error("输入必须是大小不超过限制的普通文件，且读取期间不能改变长度")]
  InputLimit,
  #[error("输出路径必须互不相同且不存在；不会覆盖已有文件")]
  OutputExists,
  #[error("密码派生失败：{0}")]
  Argon(#[from] argon2::Error),
  #[error("密码学后端操作失败")]
  Crypto(#[from] aws_lc_rs::error::Unspecified),
  #[error("密钥内容无效")]
  InvalidKey(#[from] aws_lc_rs::error::KeyRejected),
  #[error("{0}")]
  Usage(&'static str),
}
