use display_tree::{AsTree, CharSet, DisplayTree, StyleBuilder};

// A tree representing a numerical expression.
#[derive(DisplayTree)]
enum Expr {
    Int(i32),
    BinOp {
        #[node_label]
        op: char,
        #[tree]
        left: Box<Self>,
        #[tree]
        right: Box<Self>,
    },
    UnaryOp {
        #[node_label]
        op: char,
        #[tree]
        arg: Box<Self>,
    },
}

fn main() {
    let expr: Expr = Expr::BinOp {
        op: '+',
        left: Box::new(Expr::UnaryOp {
            op: '-',
            arg: Box::new(Expr::Int(2)),
        }),
        right: Box::new(Expr::Int(7)),
    };

    assert_eq!(
        format!(
            "{}",
            AsTree::new(&expr)
                .indentation(1)
                .char_set(CharSet::DOUBLE_LINE)
        ),
        concat!(
            "+\n",
            "╠═ -\n",
            "║  ╚═ Int\n",
            "║     ╚═ 2\n",
            "╚═ Int\n",
            "   ╚═ 7",
        ),
    );
    println!("Hello, world!");
    let expr = format!(
        "{}",
        AsTree::new(&expr)
            .indentation(1)
            .char_set(CharSet::DOUBLE_LINE)
    );

    println!("expr: {}", expr);
}
