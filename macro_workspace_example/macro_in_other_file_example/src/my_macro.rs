#[macro_export]
macro_rules! square {
  ($x:expr) => {
    $x * $x
  };
}

#[macro_export]
macro_rules! make_forces_torques_zero {
    (($($dest:ident), *)) => {
        $(
            $dest = 0.0;
        )*
    };
}
