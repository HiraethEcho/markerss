//! SQLite storage — items + read state (rusqlite, bundled).
//! SQLite 存储 — 条目与已读状态（rusqlite，内置编译 SQLite）。
//!
//! Subscriptions stay in the `urls` file (newsboat format, live-editable);
//! the DB holds feed items and their content. Located at
//! `$XDG_STATE_HOME/markerss/markerss.db`.
//! 订阅列表存在 urls 文件（newsboat 格式，可直接编辑）；
//! 数据库只存文章条目及其内容。位置：
//! `$XDG_STATE_HOME/markerss/markerss.db`。
//!
//! Rust concepts introduced here:
//! 本文件涉及的 Rust 概念：
//! - prepared statements: `prepare(sql)` compiles SQL once, then `execute()`
//!   runs it many times with different bound values. Exactly
//!   `sqlite3_prepare_v2` + `sqlite3_bind_*` from C — same idea, but rusqlite
//!   wraps it so binding errors are type-checked at compile time.
//!   预编译语句：prepare 编译一次 SQL，execute 配合不同参数多次执行。
//!   对应 C 的 sqlite3_prepare_v2 + sqlite3_bind_*，但 rusqlite
//!   把参数绑定做成编译期类型检查。
//! - `params![]` / `[values]`: bind arguments to `?1 ?2 ...` placeholders.
//!   Never build SQL by concatenating strings — that's how injection happens.
//!   params![] / [值]：把参数绑到 ?1 ?2 占位符上。绝不拼接字符串构造 SQL，防注入。
//! - closures as row mappers: `query_map(..., |row| Ok(...))` converts each
//!   DB row into a Rust value via a lambda you supply.
//!   闭包做行映射：query_map 用你提供的 lambda 把每行转成 Rust 值。
//! - transactions: group many writes so they all succeed or all roll back.
//!   事务：批量写入要么全成功要么全部回滚。
//! - `&self` vs `&mut self`: reads take an immutable borrow (any number
//!   allowed), writes need exclusive mutable borrow — compiler-enforced.
//!   &self 与 &mut self：读用不可变借用（可多个），写需独占可变借用，编译器强制。

use std::path::Path;

use rusqlite::Connection;

use crate::model::Item;

/// Thin wrapper around a rusqlite connection — owns it outright.
/// rusqlite 连接的薄封装 — 完全拥有连接。
pub struct Db {
    // Owning field: when a Db is dropped, Connection's Drop impl closes the
    // SQLite handle automatically. C: you must remember sqlite3_close();
    // Rust: RAII does it for you, no leak possible.
    // 自有字段：Db 被销毁时，Connection 的 Drop 自动关闭 SQLite 句柄。
    // C 需手动 sqlite3_close()；Rust 的 RAII 自动完成，不会泄漏。
    conn: Connection,
}

impl Db {
    /// Open (or create) the database file and set up schema + migrations.
    /// 打开（或新建）数据库文件，建表并执行迁移。
    pub fn open(path: &Path) -> rusqlite::Result<Db> {
        // if let Some(dir): path may have no parent (bare filename); skip mkdir then.
        // path 可能没有父目录（纯文件名），此时跳过建目录。
        // `.ok()`: discard the Result deliberately (best effort — the open()
        // below will fail loudly if the dir truly couldn't be made).
        // .ok()：故意丢弃 Result（尽力而为；真失败时下面 open 会报错）。
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        // The `?` propagates any open failure to the caller immediately.
        // ? 把打开失败立刻传给调用方。
        let conn = Connection::open(path)?;
        // WAL journaling: readers don't block the writer; synchronous NORMAL is
        // the standard durability/perf pairing with WAL.
        // WAL 日志模式：读写不互相阻塞；synchronous NORMAL 是 WAL 标准搭配。
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS items (
                feed_url TEXT NOT NULL,
                guid     TEXT NOT NULL,
                title    TEXT NOT NULL DEFAULT '',
                url      TEXT NOT NULL DEFAULT '',
                summary  TEXT NOT NULL DEFAULT '',
                content  TEXT NOT NULL DEFAULT '',
                date     TEXT NOT NULL DEFAULT '',
                author   TEXT NOT NULL DEFAULT '',
                read     INTEGER NOT NULL DEFAULT 0,
                read_later INTEGER NOT NULL DEFAULT 0,
                saved    INTEGER NOT NULL DEFAULT 0,
                fetched_at TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (feed_url, guid)
            );
            CREATE INDEX IF NOT EXISTS idx_items_feed ON items(feed_url, read);
            ",
        )?;
        // Schema migration for older DB files: add columns that didn't exist yet.
        // Iterate over an ARRAY of tuples — a tiny data-driven migration table.
        // 旧库迁移：补齐缺失列。遍历元组数组 — 数据驱动的迁移表。
        for (col, def) in [
            ("read_later", "INTEGER NOT NULL DEFAULT 0"),
            ("saved", "INTEGER NOT NULL DEFAULT 0"),
            ("author", "TEXT NOT NULL DEFAULT ''"),
        ] {
            // prepare(): compile the SQL once (≈ sqlite3_prepare_v2).
            // prepare()：编译一次 SQL。
            let has: bool = conn
                .prepare("SELECT 1 FROM pragma_table_info('items') WHERE name = ?1")?
                // query_row(params, mapper): bind params, map the single row through
                // the closure. Missing column → Err(QueryReturnedNoRows);
                // unwrap_or(false) converts any Err into false.
                // query_row：绑定参数并用闭包映射单行结果。列不存在时返回 Err，
                // unwrap_or(false) 把任何 Err 变成 false。
                .query_row([col], |_| Ok(true))
                .unwrap_or(false);
            if !has {
                // format! here is SAFE despite dynamic SQL: col/def come from the
                // hardcoded array above, never from user input. Identifiers (table/
                // column names) can't be bound via ?, so string building is the norm.
                // 这里的 format! 拼接是安全的：col/def 来自写死的数组，不含用户输入。
                // 标识符（表名/列名）无法用 ? 绑定，只能拼接。
                conn.execute(&format!("ALTER TABLE items ADD COLUMN {col} {def}"), [])?;
            }
        }
        // flag indexes after migration (old DBs lack the columns)
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_items_later ON items(read_later);
             CREATE INDEX IF NOT EXISTS idx_items_saved ON items(saved);",
        )?;
        Ok(Db { conn })
    }

    /// Fetch-mode upsert: insert new items, update metadata of existing ones,
    /// preserve read/content/flags. Returns the guids of newly added items.
    /// 抓取模式的 upsert：新条目插入，已有条目只更新元数据，
    /// 保留已读/内容/标记。返回新增条目的 guid 列表。
    pub fn upsert_fetch(&mut self, feed_url: &str, items: &[Item]) -> rusqlite::Result<Vec<String>> {
        let mut added = Vec::new();
        // transaction(): BEGIN a transaction. Everything prepared/executed on
        // `tx` below is grouped; commit() makes it permanent, dropping tx rolls back.
        // transaction()：开启事务。下面基于 tx 的操作要么随 commit 永久生效，
        // 要么在丢弃 tx 时全部回滚。
        let tx = self.conn.transaction()?;
        {
            // Braced block: limits the borrow of `tx` by the statements — both
            // are dropped (unborrowed) before commit() runs.
            // 花括号块：限定语句对 tx 的借用，commit 前两条语句已被释放。
            let mut ins = tx.prepare(
                "INSERT OR IGNORE INTO items
                 (feed_url, guid, title, url, summary, content, date, author, read, read_later, saved, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 0, 0, ?9)",
            )?;
            let mut upd = tx.prepare(
                "UPDATE items SET title = ?3, url = ?4, summary = ?5, date = ?6, author = ?7,
                        content = CASE WHEN content = '' THEN ?8 ELSE content END,
                        fetched_at = CASE WHEN content = '' THEN ?9 ELSE fetched_at END
                 WHERE feed_url = ?1 AND guid = ?2",
            )?;
            let now = chrono::Utc::now().to_rfc3339();
            for i in items {
                // params![] macro binds the tuple to ?1..?9 positionally —
                // type errors surface at compile time, values are escaped safely.
                // params![] 宏按位置绑定到 ?1..?9，类型编译期检查，值自动安全转义。
                // execute returns how many rows changed: 0 means INSERT OR IGNORE
                // skipped a duplicate guid.
                // execute 返回受影响行数：0 表示 guid 重复被 INSERT OR IGNORE 跳过。
                let n = ins.execute(rusqlite::params![
                    feed_url,
                    i.guid,
                    i.title,
                    i.url,
                    i.summary,
                    i.content,
                    i.date,
                    i.author,
                    now,
                ])?;
                if n > 0 {
                    // clone(): we keep a copy in `added` while `i` stays borrowed.
                    // clone()：added 留一份拷贝，i 本身仍是借用。
                    added.push(i.guid.clone());
                }
                // The UPDATE only touches metadata + empty-content rows, so read
                // flags and previously fetched articles survive untouched.
                // UPDATE 只改元数据和空内容行，已读标记与旧文章内容不受影响。
                upd.execute(rusqlite::params![
                    feed_url, i.guid, i.title, i.url, i.summary, i.date, i.author, i.content, now
                ])?;
            }
        }
        tx.commit()?;
        Ok(added)
    }

    /// Keep `guid`-keyed read flags across a refresh (delete+insert would lose them).
    pub fn replace_feed_items_preserving_read(
        &mut self,
        feed_url: &str,
        items: &[Item],
    ) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        {
            // capture existing read flags + fetched content BEFORE deleting
            // 删除之前先抢救出现有的已读标记与已抓取内容。
            // HashSet<String> / HashMap<String, String>: hash containers ≈
            // a C hash table you didn't have to write. Default::default() = empty.
            // HashSet/HashMap：现成的哈希容器，Default::default() 取空实例。
            let mut read_guids: std::collections::HashSet<String> = Default::default();
            let mut content_map: std::collections::HashMap<String, String> = Default::default();
            let mut fetched_map: std::collections::HashMap<String, String> = Default::default();
            let mut later_map: std::collections::HashSet<String> = Default::default();
            let mut saved_map: std::collections::HashSet<String> = Default::default();
            // full rows of flagged items — kept even when the feed drops them
            // 被标记条目的完整行 — 源已不再提供时也要保住。
            let mut keep_rows: Vec<Item> = Vec::new();
            {
                let mut q = tx.prepare(
                    "SELECT guid, title, url, summary, content, date, author, read, read_later, saved, fetched_at FROM items WHERE feed_url = ?1",
                )?;
                let rows = q.query_map([feed_url], |r| {
                    // The closure maps ONE row to a value; rusqlite calls it per row.
                    // r.get::<_, T>(i): read column i as type T (type-checked).
                    // 闭包把单行映射成一个值；rusqlite 对每行调用一次。
                    // r.get::<_, T>(i)：按类型读取第 i 列（编译期检查）。
                    Ok((
                        r.get::<_, String>(0)?, // guid
                        r.get::<_, String>(1)?, // title
                        r.get::<_, String>(2)?, // url
                        r.get::<_, String>(3)?, // summary
                        r.get::<_, String>(4)?, // content
                        r.get::<_, String>(5)?, // date
                        r.get::<_, String>(6)?, // author
                        r.get::<_, i64>(7)?,   // read
                        r.get::<_, i64>(8)?,   // read_later
                        r.get::<_, i64>(9)?,   // saved
                        r.get::<_, String>(10)?, // fetched_at
                    ))
                })?;
                // rows is an iterator of Result<tuple>; .flatten() skips Err rows.
                // rows 是 Result 元组的迭代器；.flatten() 跳过出错的行。
                for row in rows.flatten() {
                    // row.N accesses tuple fields by position (row is a tuple,
                    // not a struct). SQLite booleans are INTEGER 0/1 → compare != 0.
                    // row.N 按位置取元组字段。SQLite 布尔即 INTEGER 0/1 → 用 != 0 比较。
                    if row.7 != 0 {
                        // clone() into the set: the tuple keeps owning its Strings;
                        // we only need guid copies here.
                        // clone() 进集合：元组仍拥有自己的 String，这里只需 guid 的副本。
                        read_guids.insert(row.0.clone());
                    }
                    if !row.4.is_empty() {
                        content_map.insert(row.0.clone(), row.4.clone());
                        fetched_map.insert(row.0.clone(), row.10);
                    }
                    if row.8 != 0 {
                        later_map.insert(row.0.clone());
                    }
                    if row.9 != 0 {
                        saved_map.insert(row.0.clone());
                    }
                    if row.8 != 0 || row.9 != 0 {
                        keep_rows.push(Item {
                            guid: row.0,
                            title: row.1,
                            url: row.2,
                            summary: row.3,
                            content: row.4,
                            date: row.5,
                            author: row.6,
                            read: row.7 != 0,
                            read_later: row.8 != 0,
                            saved: row.9 != 0,
                        });
                    }
                }
            }
            let mut del = tx.prepare("DELETE FROM items WHERE feed_url = ?1")?;
            // [feed_url]: slice-of-params form for a single ?1 placeholder.
            // [feed_url]：单占位符的参数切片写法。
            del.execute([feed_url])?;
            let now = chrono::Utc::now().to_rfc3339();
            let mut ins = tx.prepare(
                "INSERT OR REPLACE INTO items
                 (feed_url, guid, title, url, summary, content, date, author, read, read_later, saved, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )?;
            for i in items {
                // Rebuild each flag from the maps captured before the delete:
                // contains() on a HashSet is O(1) average — like a hash lookup.
                // 从删除前抢救出的 map 重建各标记：HashSet::contains 平均 O(1)。
                let read = if read_guids.contains(&i.guid) { 1 } else { 0 };
                let later = if later_map.contains(&i.guid) { 1 } else { 0 };
                let saved = if saved_map.contains(&i.guid) { 1 } else { 0 };
                // keep previously fetched content; otherwise use the feed content
                // 已有抓取内容则保留，否则用源里的内容。
                // Option::cloned(): Option<&String> -> Option<String>;
                // unwrap_or_else(lazy closure): compute fallback ONLY when None.
                // cloned() 把 Option<&String> 变成 Option<String>；
                // unwrap_or_else(闭包)：仅 None 时才执行闭包计算后备值。
                let content = content_map
                    .get(&i.guid)
                    .cloned()
                    .unwrap_or_else(|| i.content.clone());
                let fetched_at = if content_map.contains_key(&i.guid) {
                    fetched_map.get(&i.guid).cloned().unwrap_or_else(|| now.clone())
                } else {
                    now.clone()
                };
                ins.execute(rusqlite::params![
                    feed_url,
                    i.guid,
                    i.title,
                    i.url,
                    i.summary,
                    content,
                    i.date,
                    i.author,
                    read,
                    later,
                    saved,
                    fetched_at,
                ])?;
            }
            // re-insert flagged items the feed no longer carries (saved /
            // read-later must survive a full refresh)
            for k in &keep_rows {
                if items.iter().any(|i| i.guid == k.guid) {
                    continue;
                }
                ins.execute(rusqlite::params![
                    feed_url,
                    k.guid,
                    k.title,
                    k.url,
                    k.summary,
                    k.content,
                    k.date,
                    k.author,
                    1,
                    k.read_later as i64,
                    k.saved as i64,
                    now,
                ])?;
            }
        }
        tx.commit()
    }

    /// Update an item's content (fetched full article) — preserves read flag.
    /// 更新条目全文内容 — 不动已读标记。
    pub fn update_item_content(&mut self, feed_url: &str, guid: &str, content: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE items SET content = ?3, fetched_at = ?4 WHERE feed_url = ?1 AND guid = ?2",
            rusqlite::params![feed_url, guid, content, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// All items of one feed, oldest DB rows mapped to `Item` structs.
    /// 取某源的全部条目：数据库行映射为 Item 结构体。
    pub fn items_for_feed(&self, feed_url: &str) -> rusqlite::Result<Vec<Item>> {
        // &self: pure read — no mutation, so an immutable borrow suffices and
        // callers may hold several such queries at once.
        // &self：纯读取，不可变借用即可，多个查询可同时存在。
        let mut stmt = self.conn.prepare(
            "SELECT guid, title, url, summary, content, date, author, read, read_later, saved
             FROM items WHERE feed_url = ?1",
        )?;
        let rows = stmt.query_map([feed_url], |r| {
            // Build the struct directly from columns. `r.get(0)?` infers its type
            // (String here) from the field it feeds into — no annotation needed.
            // 直接从列构造结构体。r.get(0)? 的类型由目标字段推断，无需标注。
            Ok(Item {
                guid: r.get(0)?,
                title: r.get(1)?,
                url: r.get(2)?,
                summary: r.get(3)?,
                content: r.get(4)?,
                date: r.get(5)?,
                author: r.get(6)?,
                read: r.get::<_, i64>(7)? != 0,
                read_later: r.get::<_, i64>(8)? != 0,
                saved: r.get::<_, i64>(9)? != 0,
            })
        })?;
        // collect() into Result<Vec<Item>>: if ANY row failed to map, the whole
        // thing is Err — a neat "all-or-nothing" fold over the iterator.
        // collect() 收集成 Result<Vec<Item>>：任一行出错整体变 Err —
        // 迭代器上的「全有或全无」折叠。
        rows.collect()
    }

    /// All (feed_url, item) pairs with a flag set — for virtual nodes.
    /// 所有设置了标记的 (源 URL, 条目) 对 — 供虚拟节点使用。
    pub fn items_with_flag(&self, flag: &str) -> rusqlite::Result<Vec<(String, Item)>> {
        // Whitelist gate: only these two column names ever reach the SQL below.
        // 白名单校验：只有这两个列名能进入下面的 SQL。
        let col = match flag {
            "read_later" => "read_later",
            "saved" => "saved",
            _ => return Ok(Vec::new()),
        };
        let sql = format!(
            "SELECT feed_url, guid, title, url, summary, content, date, author, read, read_later, saved
             FROM items WHERE {col} = 1"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        // [] = no parameters to bind this time.
        // [] 表示本次没有参数要绑定。
        let rows = stmt.query_map([], |r| {
            // Return type is a TUPLE (String, Item): first element is the feed
            // URL key, second the item itself.
            // 返回元组 (String, Item)：第一个是源 URL，第二个是条目。
            Ok((
                r.get(0)?,
                Item {
                    guid: r.get(1)?,
                    title: r.get(2)?,
                    url: r.get(3)?,
                    summary: r.get(4)?,
                    content: r.get(5)?,
                    date: r.get(6)?,
                    author: r.get(7)?,
                    read: r.get::<_, i64>(8)? != 0,
                    read_later: r.get::<_, i64>(9)? != 0,
                    saved: r.get::<_, i64>(10)? != 0,
                },
            ))
        })?;
        rows.collect()
    }

    /// COUNT of items with a flag set — for nav node badges (no row loads).
    /// 统计设置了标记的条目数 — 供导航节点角标（不加载行数据）。
    pub fn flag_count(&self, flag: &str) -> rusqlite::Result<usize> {
        // Same whitelist pattern; note "read_later" | "saved" OR-patterns
        // match either string in one arm.
        // 同样的白名单；"read_later" | "saved" 是或模式，一个分支匹配两个值。
        let col = match flag {
            "read_later" | "saved" => flag,
            _ => return Ok(0),
        };
        // query_row: exactly one expected result row. [] = no bound params.
        // query_row：期望恰好一行结果。[] 表示无参数。
        let n: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM items WHERE {col} = 1"),
            [],
            |r| r.get(0),
        )?;
        Ok(n as usize) // widen i64 -> usize for the Rust-side count type
                      // i64 转成 Rust 侧的 usize 计数类型
    }

    /// Per-feed unread counts in one grouped query (avoids N lookups per feed).
    /// 一条 GROUP BY 查询拿到每个源的未读数（避免逐源查询）。
    pub fn unread_counts(&self) -> rusqlite::Result<std::collections::HashMap<String, usize>> {
        use std::collections::HashMap;
        let mut m = HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT feed_url, COUNT(*) FROM items WHERE read = 0 GROUP BY feed_url",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize))
        })?;
        for r in rows.flatten() {
            m.insert(r.0, r.1); // tuple fields straight into the map: key, value
                                // 元组字段直接入 map：键、值
        }
        Ok(m)
    }

    /// Set read_later/saved on one item; unknown flag names are ignored.
    /// 设置某条目的 read_later/saved；未知标记名直接忽略。
    pub fn set_flag(&mut self, feed_url: &str, guid: &str, flag: &str, on: bool) -> rusqlite::Result<()> {
        // Whitelist again — `flag` is user-controlled text and must never be
        // spliced into SQL unchecked.
        // 再次白名单 — flag 是外部传入文本，绝不能未检查就拼进 SQL。
        let col = match flag {
            "read_later" | "saved" => flag,
            _ => return Ok(()),
        };
        let sql = format!("UPDATE items SET {col} = ?3 WHERE feed_url = ?1 AND guid = ?2");
        // `on as i64`: bool -> 0/1, matching SQLite's INTEGER booleans.
        // on as i64：bool 转 0/1，对应 SQLite 的整数布尔。
        self.conn.execute(&sql, rusqlite::params![feed_url, guid, on as i64])?;
        Ok(())
    }

    /// Flip read_later/saved and report the NEW value (false if item vanished).
    /// 翻转 read_later/saved，返回新值（条目不存在则 false）。
    pub fn toggle_flag(&mut self, feed_url: &str, guid: &str, flag: &str) -> rusqlite::Result<bool> {
        let col = match flag {
            "read_later" | "saved" => flag,
            _ => return Ok(false),
        };
        let sql = format!("SELECT {col} FROM items WHERE feed_url = ?1 AND guid = ?2");
        // .ok(): Result<i64,_> -> Option<i64> — we don't care WHY it failed.
        // .ok()：把 Result 变成 Option，只关心成败不关心错误原因。
        let cur: Option<i64> = self
            .conn
            .query_row(&sql, rusqlite::params![feed_url, guid], |r| r.get(0))
            .ok();
        // let-else: bind when Some, otherwise the else branch must EXIT the fn.
        // Newer alternative to `match`/`if let` + early return.
        // let-else：Some 则绑定变量，否则进入 else 分支并必须退出函数。
        // 是 match/if let 加提前返回的更新写法。
        let Some(cur) = cur else {
            return Ok(false); // row gone — nothing to toggle
                              // 行已不存在 — 无可翻转
        };
        // cur == 0: was unread/off → new value true (on). XOR semantics.
        // cur == 0：原来是关 → 新值为开。异或语义。
        self.set_flag(feed_url, guid, flag, cur == 0)?;
        Ok(cur == 0)
    }

    /// Is this specific item marked read?
    /// 该条目是否已读？
    pub fn is_read(&self, feed_url: &str, guid: &str) -> rusqlite::Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT read FROM items WHERE feed_url = ?1 AND guid = ?2",
            rusqlite::params![feed_url, guid],
            |r| r.get(0),
        )?;
        Ok(n != 0)
    }

    /// Set the read flag on one item (`read as i64`: bool -> 0/1).
    /// 设置单条已读标记（read as i64：bool 转 0/1）。
    pub fn set_read(&mut self, feed_url: &str, guid: &str, read: bool) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE items SET read = ?3 WHERE feed_url = ?1 AND guid = ?2",
            rusqlite::params![feed_url, guid, read as i64],
        )?;
        Ok(())
    }

    /// Toggle read, returning the new value. Two tiny queries instead of a
    /// clever single SQL statement — clarity over micro-optimization.
    /// 翻转已读并返回新值。用两条小查询代替单条巧妙 SQL — 清晰优先。
    pub fn toggle_read(&mut self, feed_url: &str, guid: &str) -> rusqlite::Result<bool> {
        let read = self.is_read(feed_url, guid)?;
        self.set_read(feed_url, guid, !read)?;
        Ok(!read)
    }

    /// Mark every item of one feed as read (single UPDATE, no loop).
    /// 把某源全部条目标为已读（单条 UPDATE，无需循环）。
    pub fn mark_all_read(&mut self, feed_url: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE items SET read = 1 WHERE feed_url = ?1", [feed_url])?;
        Ok(())
    }

    /// Global unread count across all feeds.
    /// 全部源的总未读数。
    pub fn total_unread(&self) -> rusqlite::Result<usize> {
        let n: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM items WHERE read = 0", [], |r| r.get(0))?;
        Ok(n as usize)
    }

    /// Delete all items of a removed feed.
    /// 删除某源的（被移除后残留的）全部条目。
    pub fn remove_feed_items(&mut self, feed_url: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM items WHERE feed_url = ?1", [feed_url])?;
        Ok(())
    }

    /// Purge fetched article bodies older than ttl_days (keep item metadata;
    /// saved items exempt). ttl_days = 0 disables cleanup entirely.
    /// 清理超过 ttl_days 的已抓取正文（保留条目元数据；收藏条目豁免）。
    /// ttl_days 为 0 表示完全不清理。
    pub fn cleanup_content(&mut self, ttl_days: u64) -> rusqlite::Result<()> {
        if ttl_days == 0 {
            return Ok(());
        }
        // now minus N days; `as i64` widens u64 for chrono::Duration::days.
        // 当前时间减去 N 天；as i64 把 u64 扩宽供 Duration::days 使用。
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(ttl_days as i64))
            .to_rfc3339();
        // Content is blanked, rows kept — titles/dates still render in lists.
        // 只清空内容不删行 — 列表里标题日期仍在。
        self.conn.execute(
            "UPDATE items SET content = '' WHERE fetched_at != '' AND fetched_at < ?1 AND saved = 0",
            [cutoff],
        )?;
        Ok(())
    }
}

// Unit tests — run with `cargo test`. Helpers below show two idioms:
// - AtomicU64 static counter: unique temp file names even across parallel tests.
// - unwrap() everywhere: in tests a panic IS the failure report.
// 单元测试 — cargo test 运行。下方辅助函数展示两个惯用法：
// - AtomicU64 静态计数器：并行测试也能生成唯一临时文件名。
// - 全部用 unwrap()：测试里 panic 就是失败报告。
#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Db {
        use std::sync::atomic::{AtomicU64, Ordering};
        // static + AtomicU64: a mutable counter without &mut — atomics allow
        // shared mutation safely (fetch_add = atomic i++).
        // static + AtomicU64：无需 &mut 的可变计数器，fetch_add 即原子自增。
        static N: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "markerss-db-{}-{}.db",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_file(&path);
        Db::open(&path).unwrap()
    }

    fn item(guid: &str) -> Item {
        Item {
            guid: guid.into(),
            title: format!("t-{guid}"),
            url: format!("https://x.com/{guid}"),
            summary: String::new(),
            content: String::new(),
            date: String::new(),
            read: false,
            author: String::new(),
            read_later: false,
            saved: false,
        }
    }

    #[test]
    fn replace_and_read_back() {
        let mut db = test_db();
        db.replace_feed_items_preserving_read("https://f.com", &[item("a"), item("b")]).unwrap();
        let items = db.items_for_feed("https://f.com").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(db.total_unread().unwrap(), 2);
    }

    #[test]
    fn refresh_preserves_read() {
        let mut db = test_db();
        db.replace_feed_items_preserving_read("https://f.com", &[item("a")]).unwrap();
        db.set_read("https://f.com", "a", true).unwrap();
        db.replace_feed_items_preserving_read("https://f.com", &[item("a"), item("b")]).unwrap();
        assert!(db.is_read("https://f.com", "a").unwrap());
        assert!(!db.is_read("https://f.com", "b").unwrap());
    }

    #[test]
    fn toggle_and_mark_all() {
        let mut db = test_db();
        db.replace_feed_items_preserving_read("https://f.com", &[item("a"), item("b")]).unwrap();
        assert!(db.toggle_read("https://f.com", "a").unwrap());
        assert!(!db.toggle_read("https://f.com", "a").unwrap());
        db.mark_all_read("https://f.com").unwrap();
        assert_eq!(db.total_unread().unwrap(), 0);
    }

    #[test]
    fn update_content_keeps_read() {
        let mut db = test_db();
        db.replace_feed_items_preserving_read("https://f.com", &[item("a")]).unwrap();
        db.set_read("https://f.com", "a", true).unwrap();
        db.update_item_content("https://f.com", "a", "<p>full</p>").unwrap();
        let items = db.items_for_feed("https://f.com").unwrap();
        assert_eq!(items[0].content, "<p>full</p>");
        assert!(db.is_read("https://f.com", "a").unwrap());
    }
}

#[cfg(test)]
// Flag persistence tests: read_later/saved must survive refreshes.
// 标记持久化测试：read_later/saved 必须在刷新后保留。
mod flag_tests {
    use super::*;

    fn db() -> Db {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!("markerss-flags-{}-{}.db", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_file(&path);
        Db::open(&path).unwrap()
    }

    fn item(guid: &str) -> Item {
        Item { guid: guid.into(), title: String::new(), url: String::new(), summary: String::new(), content: String::new(), date: String::new(), author: String::new(), read: false, read_later: false, saved: false }
    }

    #[test]
    fn flags_toggle_and_persist() {
        let mut d = db();
        d.replace_feed_items_preserving_read("f", &[item("a"), item("b")]).unwrap();
        assert!(d.toggle_flag("f", "a", "read_later").unwrap());
        assert!(d.toggle_flag("f", "b", "saved").unwrap());
        let later = d.items_with_flag("read_later").unwrap();
        assert_eq!(later.len(), 1);
        assert_eq!(later[0].0, "f");
        assert!(later[0].1.read_later);
        let saved = d.items_with_flag("saved").unwrap();
        assert_eq!(saved.len(), 1);
        // refresh preserves flags
        d.replace_feed_items_preserving_read("f", &[item("a"), item("b"), item("c")]).unwrap();
        assert_eq!(d.items_with_flag("read_later").unwrap().len(), 1);
        assert_eq!(d.items_with_flag("saved").unwrap().len(), 1);
        // toggle off
        assert!(!d.toggle_flag("f", "a", "read_later").unwrap());
        assert_eq!(d.items_with_flag("read_later").unwrap().len(), 0);
    }

    #[test]
    fn old_db_migrates_columns() {
        // create a pre-flag db, reopen → columns added
        let path = std::env::temp_dir().join(format!("markerss-migrate-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let c = rusqlite::Connection::open(&path).unwrap();
            c.execute_batch("CREATE TABLE items (feed_url TEXT NOT NULL, guid TEXT NOT NULL, title TEXT NOT NULL DEFAULT '', url TEXT NOT NULL DEFAULT '', summary TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '', date TEXT NOT NULL DEFAULT '', read INTEGER NOT NULL DEFAULT 0, fetched_at TEXT NOT NULL DEFAULT '', PRIMARY KEY (feed_url, guid));").unwrap();
        }
        let mut d = Db::open(&path).unwrap();
        d.replace_feed_items_preserving_read("f", &[item("x")]).unwrap();
        assert_eq!(d.items_with_flag("saved").unwrap().len(), 0);
        std::fs::remove_file(&path).ok();
    }
}

#[cfg(test)]
// Fetch-upsert semantics tests (new items added, metadata updated, flags kept).
// 抓取 upsert 语义测试（新增条目、更新元数据、保留标记）。
mod upsert_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);

    fn db() -> Db {
        let p = std::env::temp_dir().join(format!("markerss-upsert-{}-{}.db", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_file(&p);
        Db::open(&p).unwrap()
    }

    fn item(guid: &str, title: &str) -> Item {
        Item { guid: guid.into(), title: title.into(), url: String::new(), summary: String::new(), content: String::new(), date: "2026-01-01".into(), author: String::new(), read: false, read_later: false, saved: false }
    }

    #[test]
    fn upsert_adds_new_keeps_existing() {
        let mut d = db();
        d.replace_feed_items_preserving_read("f", &[item("a", "old title")]).unwrap();
        d.set_read("f", "a", true).unwrap();
        d.set_flag("f", "a", "saved", true).unwrap();
        d.update_item_content("f", "a", "<p>content</p>").unwrap();
        // fetch: new guid b added, a updated metadata only
        let added = d.upsert_fetch("f", &[item("a", "new title"), item("b", "new")]).unwrap();
        assert_eq!(added, vec!["b"]);
        let items = d.items_for_feed("f").unwrap();
        assert_eq!(items.len(), 2);
        let a = items.iter().find(|i| i.guid == "a").unwrap();
        assert_eq!(a.title, "new title"); // metadata updated
        assert!(d.is_read("f", "a").unwrap()); // read preserved
        assert_eq!(d.items_with_flag("saved").unwrap().len(), 1); // flag preserved
        let a2 = items.iter().find(|i| i.guid == "a").unwrap();
        assert_eq!(a2.content, "<p>content</p>"); // content preserved
        let b = items.iter().find(|i| i.guid == "b").unwrap();
        assert!(!d.is_read("f", "b").unwrap()); // new item unread
    }

    #[test]
    fn ttl_purge_clears_only_old_non_saved_content() {
        let mut d = db();
        d.replace_feed_items_preserving_read("f", &[item("a", "A")]).unwrap();
        d.update_item_content("f", "a", "<p>fresh</p>").unwrap();
        d.upsert_fetch("f", &[item("b", "B")]).unwrap();
        d.update_item_content("f", "b", "<p>old</p>").unwrap();
        // age b's content beyond the TTL by backdating fetched_at
        let old = (chrono::Utc::now() - chrono::Duration::days(99)).to_rfc3339();
        d.conn.execute("UPDATE items SET fetched_at = ?1 WHERE guid = 'b'", [old]).unwrap();
        // saved items are exempt
        d.set_flag("f", "a", "saved", true).unwrap();
        d.cleanup_content(30).unwrap();
        let items = d.items_for_feed("f").unwrap();
        let a = items.iter().find(|i| i.guid == "a").unwrap();
        let b = items.iter().find(|i| i.guid == "b").unwrap();
        assert_eq!(a.content, "<p>fresh</p>"); // saved + fresh → kept
        assert_eq!(b.content, ""); // old non-saved → purged
        // item rows stay; only content is cleared
        assert_eq!(items.len(), 2);
    }
}
