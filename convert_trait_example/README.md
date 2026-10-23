# Rust Trait Object 示例

本项目用最小可运行代码说明 Rust 的 trait object（`dyn Trait`）。运行：

```bash
cargo run
cargo test
```

## 1. Trait object 是什么

`dyn Trait` 是运行时多态的视图，不是一个具体类型。它通常由两部分组成：

- 指向实际数据的指针；
- 指向 vtable 的指针，vtable 中有该具体类型对应的方法实现。

因此 `&dyn Pet` 的大小固定，但它不能直接访问 `Dog::age` 这样的具体字段。
调用 trait 方法时会经过动态分发；如果类型在编译期已知，泛型通常可以使用静态分发。

## 2. 从 trait object 取出里面的数据

推荐顺序是：

1. **优先把需要的数据放进 trait 方法**，例如 `name()`；这保留了抽象边界。
2. 如果确实需要知道具体类型，让 trait 继承 `Any`，提供 `as_any()`，再使用：

```rust
let dog = pet.as_any().downcast_ref::<Dog>();
let dog = pet.as_any_mut().downcast_mut::<Dog>();
```

`downcast_ref`/`downcast_mut` 会检查运行时真实类型，失败时返回 `None`，不会凭空把
`Cat` 解释成 `Dog`。这要求目标类型满足 `Any` 的 `'static` 限制；包含非 `'static`
借用的数据不能用这个方式恢复。

代码位置：`src/main.rs` 的 `demonstrate_recovering_data`。

## 3. 转换为另一个 trait object

### 3.1 向上转型：超 trait

如果有：

```rust
trait Animal: Pet { /* ... */ }
```

那么 `&dyn Animal` 可以作为 `&dyn Pet` 使用：

```rust
let animal: &dyn Animal = &dog;
let pet: &dyn Pet = animal;
```

这是 trait upcasting：真实数据没有改变，只是可见的方法集合变少了。它只适用于
继承关系，不能用于两个无关 trait。

### 3.2 向下转型：恢复具体类型

`&dyn Pet -> &Dog` 不是普通的编译期转换，要通过 `Any::downcast_ref`，并接受失败的
`Option`。得到 `Dog` 后，可以再按需要借用为另一个 trait object。

### 3.3 无关 trait：使用适配器

`Pet` 和 `Named` 没有继承关系，不能写成 `pet as &dyn Named`。可以包一层适配器：

```rust
struct NamedAdapter<'a> { pet: &'a dyn Pet }

impl Named for NamedAdapter<'_> { /* 转发或组合行为 */ }
```

这不是改变原对象类型，而是创建了一个实现目标 trait 的新视图对象。示例见
`NamedAdapter`。

## 4. 转为不同范式的 trait object

下面这些类型的动态对象都可以指向同一个具体类型，但所有权和生命周期不同：

| 形式 | 含义 | 适用场景 |
| --- | --- | --- |
| `&dyn Pet` | 临时不可变借用 | 调用方只读、不取得所有权 |
| `&mut dyn Pet` | 临时独占可变借用 | 需要通过 trait 修改对象 |
| `Box<dyn Pet>` | 堆上独占所有权 | 返回异构对象、放进 `Vec` |
| `Rc<dyn Pet>` | 单线程共享所有权 | 多个单线程持有者 |
| `Arc<dyn Pet + Send + Sync>` | 多线程共享所有权 | 在线程之间共享 |

例如：

```rust
let borrowed: &dyn Pet = &dog;
let owned: Box<dyn Pet> = Box::new(dog);
let shared: Rc<dyn Pet> = Rc::new(Cat { name: "咪咪".into() });
let threaded: Arc<dyn Pet + Send + Sync> = Arc::new(dog);
```

这些转换通常发生在构造容器时，或通过 `as_ref()`/`as_mut()` 借出视图；它们不等于
把 `Dog` 的内存布局转换成另一个类型。`Rc` 不能跨线程，`Arc` 中的对象还必须满足
`Send + Sync`。

## 5. trait object 的限制

一个 trait 要能写成 `dyn Trait`，必须满足 dyn-compatible（旧称 object-safe）规则：

- trait 不能要求 `Self: Sized`；
- 不能有返回 `Self` 的普通方法；
- 不能有泛型方法；
- 不能有依赖具体 `Self` 大小的关联常量；
- 不能把 `Self` 用在不允许动态分发的位置，例如某些参数或返回类型；
- 关联类型本身不是绝对禁止，但使用 trait object 时通常必须指定它。

需要保留这些 API 时，可以给单个方法增加 `where Self: Sized`，这样该方法只能通过
具体类型调用，不能通过 `dyn Trait` 调用：

```rust
trait Factory {
    fn make() -> Self where Self: Sized;
    fn generic<T>(&self, value: T) where Self: Sized;
}
```

如果 trait 的核心 API 天然依赖泛型或返回 `Self`，应使用泛型/`impl Trait`，或者把
不同实现收敛到一个 enum，而不是强行使用 trait object。

另外，trait object 还会受到普通 Rust 约束影响：需要线程安全时添加 `Send + Sync`，
需要 `'static` 时显式写出生命周期；`Box<dyn Pet>` 默认通常意味着对象至少满足
容器要求的生命周期，而借用对象应写成 `Box<dyn Pet + 'a>` 等明确形式。

## 6. 选择建议

- 类型在编译期确定、关注性能：优先泛型 `T: Trait`。
- 需要 `Vec` 中放不同具体类型：使用 `Vec<Box<dyn Trait>>`。
- 只需要短暂读取：使用 `&dyn Trait`。
- 需要恢复具体类型：谨慎使用 `Any`，并考虑是否应该把行为放回 trait。
- 只有少数固定类型：enum 往往比 trait object 更直接，也不需要 vtable。

所有完整代码和断言都在 `src/main.rs`。
