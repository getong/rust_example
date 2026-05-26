use libmdbx::*;

type Database = libmdbx::Database<NoWriteMap>;

fn opt_to_str(v: &Option<Vec<u8>>) -> String {
  match v {
    Some(b) => format!("\"{}\"", String::from_utf8_lossy(b)),
    None => "None".to_string(),
  }
}

fn show_compare(write: &str, read: &str, actual: &Option<Vec<u8>>, expected: Option<&[u8]>) {
  let expected = expected.map(|v| v.to_vec());
  let status = if actual == &expected { "OK" } else { "FAILED" };

  println!("  write   {write}");
  println!("  read    {read} = {}", opt_to_str(actual));
  println!("  expect  {read} = {}", opt_to_str(&expected));
  println!("  compare {status}");
}

fn main() -> Result<()> {
  let dir = tempfile::tempdir().map_err(|e| Error::DecodeError(Box::new(e)))?;
  let db = Database::open(&dir)?;

  // ── Atomicity: 事务要么全部提交,要么全部回滚 ─────────────────────
  println!("=== Atomicity: 事务原子性 ===");

  // 写入数据但不 commit, 让 Drop 自动 abort
  {
    let txn = db.begin_rw_txn()?;
    let t = txn.open_table(None)?;
    txn.put(&t, b"a", b"1", WriteFlags::UPSERT)?;
    txn.put(&t, b"b", b"2", WriteFlags::UPSERT)?;
    // 作用域结束 → Drop → mdbx_txn_abort
  }

  // 验证: abort 后的数据不可见
  {
    let txn = db.begin_ro_txn()?;
    let t = txn.open_table(None)?;
    let val_a: Option<Vec<u8>> = txn.get(&t, b"a")?;
    let val_b: Option<Vec<u8>> = txn.get(&t, b"b")?;
    show_compare("put(a, \"1\") then abort", "a", &val_a, None);
    show_compare("put(b, \"2\") then abort", "b", &val_b, None);
    assert_eq!(val_a, None);
    assert_eq!(val_b, None);
    txn.commit()?;
  }

  // 显式 abort
  {
    let txn = db.begin_rw_txn()?;
    let t = txn.open_table(None)?;
    txn.put(&t, b"x", b"should_not_exist", WriteFlags::UPSERT)?;
    std::mem::drop(txn); // 显式 abort
  }

  {
    let txn = db.begin_ro_txn()?;
    let t = txn.open_table(None)?;
    let val_x: Option<Vec<u8>> = txn.get(&t, b"x")?;
    show_compare("put(x, \"should_not_exist\") then abort", "x", &val_x, None);
    assert_eq!(val_x, None);
    txn.commit()?;
  }

  // ── Atomicity: commit 的数据真正可见 ──────────────────────────
  println!("  --- commit 作对比 ---");
  {
    let txn = db.begin_rw_txn()?;
    let t = txn.open_table(None)?;
    txn.put(&t, b"committed", b"visible", WriteFlags::UPSERT)?;
    txn.commit()?;
  }
  {
    let txn = db.begin_ro_txn()?;
    let t = txn.open_table(None)?;
    let val: Option<Vec<u8>> = txn.get(&t, b"committed")?;
    show_compare(
      "put(committed, \"visible\") then commit",
      "committed",
      &val,
      Some(b"visible"),
    );
    assert_eq!(val, Some(b"visible".to_vec()));
    txn.commit()?;
  }

  // ── Consistency: 事务内多次读取结果一致 ──────────────────────────
  println!("\n=== Consistency: 事务内一致性 ===");
  {
    let txn = db.begin_rw_txn()?;
    let t = txn.open_table(None)?;
    txn.put(&t, b"c", b"consistent_value", WriteFlags::UPSERT)?;

    // 同一个事务内反复读,结果始终相同
    let r1: Option<Vec<u8>> = txn.get(&t, b"c")?;
    let r2: Option<Vec<u8>> = txn.get(&t, b"c")?;
    let r3: Option<Vec<u8>> = txn.get(&t, b"c")?;
    show_compare(
      "put(c, \"consistent_value\")",
      "第 1 次 read c",
      &r1,
      Some(b"consistent_value"),
    );
    show_compare(
      "put(c, \"consistent_value\")",
      "第 2 次 read c",
      &r2,
      Some(b"consistent_value"),
    );
    show_compare(
      "put(c, \"consistent_value\")",
      "第 3 次 read c",
      &r3,
      Some(b"consistent_value"),
    );
    assert_eq!(r1, r2);
    assert_eq!(r2, r3);
    assert_eq!(r1, Some(b"consistent_value".to_vec()));
    txn.commit()?;
  }

  // ── Isolation (MVCC): 旧读事务不受并发写事务影响 ─────────────────
  println!("\n=== Isolation (MVCC): 读写隔离 ===");
  {
    // 步骤1: 开启一个长读事务 (获取数据库快照)
    let ro_txn = db.begin_ro_txn()?;
    let t_ro = ro_txn.open_table(None)?;

    // 步骤2: 写事务提交新数据
    {
      let rw_txn = db.begin_rw_txn()?;
      let t_rw = rw_txn.open_table(None)?;
      rw_txn.put(&t_rw, b"iso_key", b"after_write", WriteFlags::UPSERT)?;
      rw_txn.commit()?;
    }
    println!("  → 写事务已提交 iso_key = \"after_write\"");

    // 步骤3: 旧的读事务仍看到旧状态 (MVCC snapshot isolation)
    let old_read: Option<Vec<u8>> = ro_txn.get(&t_ro, b"iso_key")?;
    show_compare(
      "put(iso_key, \"after_write\") then commit",
      "旧读事务 read iso_key",
      &old_read,
      None,
    );
    println!("  → 旧读事务是在写提交前打开的, 所以仍读取 MVCC 快照");
    assert_eq!(old_read, None);
    ro_txn.commit()?;

    // 步骤4: 新读事务看到已提交的新状态
    {
      let ro_txn2 = db.begin_ro_txn()?;
      let t_ro2 = ro_txn2.open_table(None)?;
      let new_read: Option<Vec<u8>> = ro_txn2.get(&t_ro2, b"iso_key")?;
      show_compare(
        "put(iso_key, \"after_write\") then commit",
        "新读事务 read iso_key",
        &new_read,
        Some(b"after_write"),
      );
      assert_eq!(new_read, Some(b"after_write".to_vec()));
      ro_txn2.commit()?;
    }
  }

  // ── Durability: 关闭并重新打开后数据持久化 ─────────────────────
  println!("\n=== Durability: 持久性 ===");
  let path = dir.path().to_path_buf();
  {
    let txn = db.begin_rw_txn()?;
    let t = txn.open_table(None)?;
    txn.put(&t, b"durable", b"still_here", WriteFlags::UPSERT)?;
    txn.commit()?;
  }
  println!("  已提交 durable = \"still_here\", 关闭数据库...");
  std::mem::drop(db); // 关闭数据库

  // 重新打开数据库, 验证持久化
  let db = Database::open(&path)?;
  {
    let txn = db.begin_ro_txn()?;
    let t = txn.open_table(None)?;
    let val: Option<Vec<u8>> = txn.get(&t, b"durable")?;
    show_compare(
      "put(durable, \"still_here\") then commit",
      "重新打开后 read durable",
      &val,
      Some(b"still_here"),
    );
    assert_eq!(val, Some(b"still_here".to_vec()));
    println!("  → 已提交数据在 close/reopen 后完整保留");
    txn.commit()?;
  }

  // ── Isolation 进阶: 并发读写互不阻塞 ──────────────────────────
  println!("\n=== Isolation: 并发读写不阻塞 ===");
  {
    let dir2 = tempfile::tempdir().map_err(|e| Error::DecodeError(Box::new(e)))?;
    let db_arc = std::sync::Arc::new(Database::open(&dir2)?);
    let db_clone = db_arc.clone();

    let h1 = std::thread::spawn(move || -> Result<()> {
      let txn = db_clone.begin_rw_txn()?;
      let t = txn.open_table(None)?;
      for i in 0 .. 100 {
        txn.put(
          &t,
          format!("concur_{i}").as_bytes(),
          b"data",
          WriteFlags::UPSERT,
        )?;
      }
      txn.commit()?;
      println!("  [写线程] 100 条数据写入完成");
      Ok(())
    });

    let h2 = std::thread::spawn(move || -> Result<()> {
      let txn = db_arc.begin_ro_txn()?;
      let t = txn.open_table(None)?;
      // 读事务完全不被写阻塞, 且看不到未提交数据
      for i in 0 .. 100 {
        assert_eq!(txn.get::<()>(&t, format!("concur_{i}").as_bytes())?, None);
      }
      println!("  [读线程] 100 条均不可见 (与写事务互不阻塞)");
      txn.commit()?;
      Ok(())
    });

    h1.join().unwrap()?;
    h2.join().unwrap()?;
  }

  println!("\n✅ ACID 全部通过.");
  Ok(())
}
