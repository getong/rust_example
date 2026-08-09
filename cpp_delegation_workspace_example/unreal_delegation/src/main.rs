use std::{
  cell::RefCell,
  collections::HashMap,
  rc::{Rc, Weak},
};

// 模拟一个 UObject (比如游戏里的 Actor)
struct MyActor {
  name: String,
}

impl MyActor {
  fn take_damage(&self, amount: i32) {
    println!("{} 受到了 {} 点伤害！", self.name, amount);
  }
}

// 模拟 UE 的单播委托
struct FSingleCastDelegate {
  // 使用 Weak 智能指针模拟 UE 的弱引用绑定（防止 Actor 被销毁后发生野指针崩溃）
  binding: Option<(Weak<RefCell<MyActor>>, fn(&MyActor, i32))>,
}

impl FSingleCastDelegate {
  fn new() -> Self {
    Self { binding: None }
  }

  // 对应 UE 的 BindUObject()
  fn bind_uobject(&mut self, object: &Rc<RefCell<MyActor>>, func: fn(&MyActor, i32)) {
    self.binding = Some((Rc::downgrade(object), func));
  }

  // 对应 UE 的 ExecuteIfBound()
  fn execute_if_bound(&self, amount: i32) {
    if let Some((ref weak_obj, func)) = self.binding {
      // 尝试提升为强引用，如果升级失败说明对象已被销毁 (GC了)
      if let Some(strong_obj) = weak_obj.upgrade() {
        func(&strong_obj.borrow(), amount);
      } else {
        println!("委托执行失败：目标对象已被销毁（模拟GC）");
      }
    }
  }
}

// 模拟 UE 的多播委托
struct FMultiCastDelegate {
  // 存储一个回调数组
  listeners: Vec<Box<dyn Fn(i32)>>,
}

impl FMultiCastDelegate {
  fn new() -> Self {
    Self {
      listeners: Vec::new(),
    }
  }

  // 对应 UE 的 .AddUObject() 或 .AddLambda()
  fn add_lambda<F>(&mut self, callback: F)
  where
    F: Fn(i32) + 'static,
  {
    self.listeners.push(Box::new(callback));
  }

  // 对应 UE 的 .Broadcast()
  fn broadcast(&self, value: i32) {
    for listener in &self.listeners {
      listener(value); // 依次调用所有监听者
    }
  }
}

// 模拟具有反射能力的类
struct BlueprintObject {
  // 存储该类身上可以通过名字调用的“蓝图函数”
  functions: HashMap<String, Box<dyn Fn(String)>>,
}

// 模拟动态单播委托 (对应 DECLARE_DYNAMIC_DELEGATE)
// 结合了"单播"和"动态"两个特性：只能绑一个目标，且按函数名反射调用
struct FDynamicDelegate {
  // 单个绑定（Option 模拟未绑定状态），存的是 对象 + 函数名字符串
  binding: Option<(Rc<RefCell<BlueprintObject>>, String)>,
}

impl FDynamicDelegate {
  fn new() -> Self {
    Self { binding: None }
  }

  // 对应 UE 的 BindDynamic() 宏 / BindUFunction()
  // 注意：绑定的是函数"名字"，不是函数指针，所以 UE 要求目标必须是 UFUNCTION()
  fn bind_dynamic(&mut self, obj: Rc<RefCell<BlueprintObject>>, func_name: &str) {
    // 单播语义：再次绑定会覆盖之前的绑定
    self.binding = Some((obj, func_name.to_string()));
  }

  // 对应 UE 的 ExecuteIfBound()
  fn execute_if_bound(&self, message: String) {
    if let Some((ref obj, ref func_name)) = self.binding {
      let obj_borrow = obj.borrow();
      // 运行时通过字符串名字在反射表中查找函数
      if let Some(func) = obj_borrow.functions.get(func_name) {
        func(message);
      } else {
        println!(
          "委托执行失败：函数 '{}' 未在反射表中注册（非 UFUNCTION）",
          func_name
        );
      }
    }
  }
}

// 模拟动态多播委托 (例如：BlueprintAssignable)
struct FDynamicMulticastDelegate {
  // 存储的是目标对象的引用 + 目标函数的名字（字符串）
  bound_functions: Vec<(Rc<RefCell<BlueprintObject>>, String)>,
}

impl FDynamicMulticastDelegate {
  fn new() -> Self {
    Self {
      bound_functions: Vec::new(),
    }
  }

  // 对应 UE 的 AddDynamic() 宏
  fn add_dynamic(&mut self, obj: Rc<RefCell<BlueprintObject>>, func_name: &str) {
    self.bound_functions.push((obj, func_name.to_string()));
  }

  // 对应 UE 的 Broadcast()
  fn broadcast(&self, message: String) {
    for (obj, func_name) in &self.bound_functions {
      let obj_borrow = obj.borrow();
      // 在运行时通过字符串名字动态查找函数并执行
      if let Some(func) = obj_borrow.functions.get(func_name) {
        func(message.clone());
      }
    }
  }
}

fn main() {
  // ========== 1. 单播委托：Bind -> ExecuteIfBound ==========
  println!("=== 单播委托 FSingleCastDelegate ===");

  // 创建一个 Actor（Rc<RefCell<T>> 模拟 UE 中由 GC 管理的 UObject）
  let actor = Rc::new(RefCell::new(MyActor {
    name: "敌人小兵".to_string(),
  }));

  let mut on_hit = FSingleCastDelegate::new();
  // 对应 UE: OnHit.BindUObject(Actor, &AMyActor::TakeDamage);
  on_hit.bind_uobject(&actor, MyActor::take_damage);

  // 对应 UE: OnHit.ExecuteIfBound(50); —— 对象存活，正常调用
  on_hit.execute_if_bound(50);

  // 销毁 Actor（drop 最后一个强引用，模拟 UObject 被 GC 回收）
  drop(actor);
  // 再次调用：Weak::upgrade() 失败，委托安全跳过，不会野指针崩溃
  on_hit.execute_if_bound(50);

  // ========== 2. 多播委托：AddLambda -> Broadcast ==========
  println!("\n=== 多播委托 FMultiCastDelegate ===");

  let mut on_health_changed = FMultiCastDelegate::new();
  // 对应 UE: OnHealthChanged.AddLambda(...); 多个系统各自订阅同一事件
  on_health_changed.add_lambda(|hp| println!("[UI系统] 血条更新为 {}", hp));
  on_health_changed.add_lambda(|hp| println!("[音效系统] 播放受击音效 (HP={})", hp));
  on_health_changed.add_lambda(|hp| {
    if hp <= 0 {
      println!("[成就系统] 解锁成就：击败敌人");
    }
  });

  // 对应 UE: OnHealthChanged.Broadcast(30); —— 一次广播，所有监听者依次执行
  on_health_changed.broadcast(30);
  on_health_changed.broadcast(0);

  // 构造一个带"反射表"的对象，相当于 UHT 为 UFUNCTION 生成的名字->函数映射
  // （第 3、4 种委托共用这个对象）
  let mut functions: HashMap<String, Box<dyn Fn(String)>> = HashMap::new();
  functions.insert(
    "OnGameOver".to_string(),
    Box::new(|msg| println!("[蓝图事件 OnGameOver] 收到消息: {}", msg)),
  );
  functions.insert(
    "OnTimelineUpdate".to_string(),
    Box::new(|msg| println!("[蓝图事件 OnTimelineUpdate] 收到消息: {}", msg)),
  );
  let blueprint_obj = Rc::new(RefCell::new(BlueprintObject { functions }));

  // ========== 3. 动态单播委托：BindDynamic -> ExecuteIfBound（按名字反射调用，只绑一个）
  // ==========
  println!("\n=== 动态单播委托 FDynamicDelegate ===");

  let mut on_timeline = FDynamicDelegate::new();
  // 对应 UE: OnTimeline.BindDynamic(this, &UMyClass::OnTimelineUpdate);
  on_timeline.bind_dynamic(Rc::clone(&blueprint_obj), "OnTimelineUpdate");
  on_timeline.execute_if_bound("进度 50%".to_string());

  // 单播语义：重新绑定会覆盖旧绑定（而不是像多播那样追加）
  on_timeline.bind_dynamic(Rc::clone(&blueprint_obj), "OnGameOver");
  on_timeline.execute_if_bound("绑定已被覆盖".to_string());

  // 绑定一个未注册的名字：反射表查不到，安全跳过
  on_timeline.bind_dynamic(Rc::clone(&blueprint_obj), "NotAUFunction");
  on_timeline.execute_if_bound("这条不会被打印".to_string());

  // ========== 4. 动态多播委托：AddDynamic -> Broadcast（按名字反射调用） ==========
  println!("\n=== 动态多播委托 FDynamicMulticastDelegate ===");

  let mut on_game_over = FDynamicMulticastDelegate::new();
  // 对应 UE: OnGameOver.AddDynamic(this, &UMyWidget::OnGameOver);
  // 注意存的是函数"名字"，运行时才去反射表里查找
  on_game_over.add_dynamic(Rc::clone(&blueprint_obj), "OnGameOver");
  // 绑定一个反射表里不存在的名字：Broadcast 时查不到，静默跳过
  on_game_over.add_dynamic(Rc::clone(&blueprint_obj), "NotRegistered");

  // 对应 UE: OnGameOver.Broadcast(TEXT("胜利！"));
  on_game_over.broadcast("胜利！".to_string());
}
