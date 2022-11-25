use fftconvolve::fftconvolve;

use ndarray::prelude::*;

use ndarray_linalg::assert_aclose;

fn main() {
    // println!("Hello, world!");
    let standard = Array2::from_shape_vec((3, 3), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]).unwrap();
    let mut standard = standard.into_iter().collect::<Vec<_>>();
    standard.reverse();
    // reverse axes
    let reversed = Array2::from_shape_vec((3, 3), standard).unwrap();
    let expected = Array2::from_shape_vec((3, 3), vec![9, 8, 7, 6, 5, 4, 3, 2, 1]).unwrap();
    assert_eq!(reversed, expected);

    let mat =
        Array2::from_shape_vec((3, 3), vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let kernel = Array2::from_shape_vec((3, 3), vec![1., 2., 3., 4., 5., 6., 7., 8., 9.]).unwrap();
    let output = fftconvolve(&mat, &kernel).unwrap();
    let expected = Array2::from_shape_vec(
        (5, 5),
        vec![
            0., 0., 0., 0., 0., 0., 1., 2., 3., 0., 0., 4., 5., 6., 0., 0., 7., 8., 9., 0., 0., 0.,
            0., 0., 0.,
        ],
    )
    .unwrap();
    output
        .iter()
        .zip(expected.iter())
        .for_each(|(a, b)| assert_aclose!(*a, *b, 1e-6));
}
