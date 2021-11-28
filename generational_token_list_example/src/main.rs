use generational_token_list::GenerationalTokenList;

fn main() {
    let mut list = GenerationalTokenList::<i32>::new();
    let _item1 = list.push_back(10);
    let _item2 = list.push_back(20);
    let _item3 = list.push_back(30);

    let data = list.into_iter().collect::<Vec<_>>();
    assert_eq!(data, vec![10, 20, 30]);
}
