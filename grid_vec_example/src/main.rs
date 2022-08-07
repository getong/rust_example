use grid::*;

fn main() {
    // println!("Hello, world!");

    let mut grid = grid![[1,2,3]
                     [4,5,6]];
    assert_eq!(grid, Grid::from_vec(vec![1, 2, 3, 4, 5, 6], 3));
    assert_eq!(grid.get(0, 2), Some(&3));
    assert_eq!(grid[1][1], 5);
    assert_eq!(grid.size(), (2, 3));
    grid.push_row(vec![7, 8, 9]);
    assert_eq!(grid, grid![[1,2,3][4,5,6][7,8,9]])
}
