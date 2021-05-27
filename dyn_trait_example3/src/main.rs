trait Animal {
    fn speak(&self);
}
struct Dog;
impl Animal for Dog {
    fn speak(&self) {
        println!("旺旺.....");
    }
}
struct Cat;
impl Animal for Cat {
    fn speak(&self) {
        println!("喵喵.....");
    }
}

//fn animal_speak<T: Animal>(animal: T) {
//    animal.speak();
//}
//
//fn main() {
//    let dog = Dog;
//    let cat = Cat;
//
//    animal_speak(dog);
//    animal_speak(cat);
//}

fn animal_speak(animal: &dyn Animal) {
    animal.speak();
}

fn main() {
    let dog = Dog;
    let cat = Cat;

    animal_speak(&dog);
    animal_speak(&cat);
}
