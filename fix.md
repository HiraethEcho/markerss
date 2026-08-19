when scroll down to end of list and go on, the go up, the list keep rolling and it stay at bottom. i want it to keep still until i hit top of the list.
when no 正文 of rss, and fetch the content, in article pane i can not scroll to bottom, it does not show whole content.

## 优化

检查 Rust 程序的优化程度并找到瓶颈，有一套成熟的方法论。简单来说，分为三步：**测量（基准测试）→ 定位（性能剖析）→ 优化（代码与编译调优）**。

下面我为你详细拆解这套方法论，并结合 Ratatui TUI 应用的特点给出具体建议。

---

### 第一步：测量 (Benchmarking) —— 建立性能基线

在优化之前，首先要知道程序“现在有多快”。这能帮你量化优化的效果，避免“凭感觉”优化。

- **推荐工具：`criterion.rs`**
  这是 Rust 生态中最流行的**微基准测试**框架。它能提供具有统计意义的测量结果（如均值、置信区间），帮你精确捕获代码变更带来的性能变化。
  - **用法**：在项目根目录下创建 `benches/` 文件夹，编写基准测试代码，然后运行 `cargo bench`。
  - **注意**：`cargo bench` 默认采用 `bench` 优化配置，它与 `release` 配置类似，都开启了优化。

- **备选工具：Rust 内置的 `#[bench]`**
  这是 Rust 标准库提供的基准测试功能，但功能相对基础，需要你在 nightly 版本下使用。

---

### 第二步：定位 (Profiling) —— 找到性能瓶颈

基准测试能告诉你“慢了多少”，而性能分析（Profiling）能告诉你“**慢在哪里**”。

#### 1. CPU 性能分析 (CPU Profiling)
这是最常用的分析方式，用于找出消耗 CPU 时间最多的热点函数。

- **`cargo-flamegraph` (推荐，最简单)**
  这是生成火焰图最方便的工具。火焰图能直观地展示 CPU 时间在函数调用栈上的分布，宽度越宽的函数，消耗的 CPU 时间越多。
  - **安装**：`cargo install flamegraph`
  - **使用**：`cargo flamegraph --bin your_program`。它会运行你的程序并生成一个漂亮的 SVG 火焰图。

- **`perf` (Linux, 最详细)**
  Linux 系统级的性能分析工具，能采集 CPU 周期、缓存命中率、函数调用栈等底层数据。
  - **使用**：`sudo perf record -g target/release/your_program` 然后 `sudo perf report` 查看报告。也可以将 `perf` 的数据转化为火焰图。

- **`hotpath-rs` (多功能)**
  一个较新的性能分析库，提供 Live TUI 仪表盘，可以实时监控 CPU、内存、异步任务、I/O 等多项指标。
  - **亮点**：能区分函数是因为等待 I/O 慢还是因为 CPU 计算慢，并支持 Tokio 运行时监控。

#### 2. 内存性能分析 (Memory Profiling)
如果你的程序内存占用过高或存在泄漏，可以使用以下工具：

- **`dhat`**：用于分析堆内存的分配次数、对象生命周期和热点分配点，指导你“减少堆分配、预分配容量、复用缓冲”等优化。
- **`Valgrind` / `heaptrack`**：经典的 Linux 工具，可以发现内存泄漏和重复分配等问题。

---

### 第三步：优化 (Optimization) —— 方法论与实践

找到瓶颈后，就可以有针对性地进行优化了。

#### 通用 Rust 优化方法论

1.  **编译优化 (Compile-time Optimizations)**
    在 `Cargo.toml` 中配置 `[profile.release]` 以启用更激进的优化：
    - **`opt-level = 3`**：最高级别的优化。
    - **`lto = true`**：开启**链接时优化**，能进行跨模块内联，显著提升性能。
    - **`codegen-units = 1`**：告诉编译器使用单个代码生成单元，以获得更充分的优化（但会增加编译时间）。
    - **`RUSTFLAGS="-C target-cpu=native"`**：为你的**当前机器**的 CPU 特性（如 AVX2）进行优化，能榨干硬件性能。

2.  **代码优化 (Code-level Optimizations)**
    - **优先使用迭代器**：Rust 的迭代器是“零成本抽象”，编译后通常比手写循环更高效。
    - **减少堆分配**：
      - 使用 `Vec::with_capacity()` 和 `String::with_capacity()` 预分配容量，避免动态扩容。
      - 在热路径（Hot Path）中避免使用 `format!` 等产生临时字符串分配的操作。
      - 能使用引用 (`&str`) 就尽量不用 `String`。
    - **选择合适的数据结构**：优先选用 `HashMap` 或 `BTreeMap` 等高效数据结构，并关注缓存局部性。

---

### 针对 Ratatui TUI 应用的特定优化技巧

TUI 应用通常有一个主循环，每秒刷新多次（如 60 FPS）。优化目标是让每帧的计算和渲染足够快。

1.  **分离“冷”、“热”路径 (The "Preparable" Pattern)**
    这是最关键的优化思想。将**昂贵的、只需执行一次**的准备工作（如文本的 Unicode 宽度计算）与**每帧都要执行的、轻量的**布局/渲染工作分离开。
    - **实践**：可以参考 `ratatui-ppalla` 库，它实现了这个模式。通过缓存文本的宽度等信息，将布局速度提升了 **21 倍**（从 2.83ms 降至 134µs）。
    - **收益**：这使得在 60fps（每帧 16.67ms）的预算下，复杂的多面板 TUI 也能流畅运行。

2.  **利用 Ratatui 的 Diff 渲染**
    Ratatui 的终端渲染本身已经很高效，它只会更新变化的单元格。但是，如果你的应用逻辑每帧都强制**重新创建整个 UI 树**，依然会造成不必要的 CPU 开销。
    - **优化**：只在实际状态发生变化（如用户输入、新数据到达）时才触发重绘，而不是每个“tick”都重绘。
    - **场景**：对于静态内容（如侧边栏、状态栏），可以跳过重绘。

3.  **避免每帧进行昂贵的计算**
    - **布局计算**：布局计算（如 `Layout` 的分割）如果每帧都做，开销不小。考虑缓存布局结果，仅在窗口大小变化时重新计算。
    - **字符串操作**：在热路径中，避免对长文本进行全量扫描或克隆。只处理可见窗口内的文本。

### 💎 总结：优化的完整工作流

1.  **写基准**：用 `criterion.rs` 为关键路径（如启动、渲染、响应事件）编写基准测试。
2.  **测性能**：运行 `cargo bench` 获取基准数据。
3.  **找瓶颈**：使用 `cargo flamegraph` 生成火焰图，或使用 `hotpath-rs` 进行实时监控，定位 CPU 或内存热点。
4.  **做优化**：
    - **通用**：应用编译优化（`lto`, `codegen-units`）和代码优化（预分配、迭代器）。
    - **TUI 特定**：应用“冷/热路径”分离模式，利用 Ratatui 的 diff 渲染，避免不必要的重绘和计算。
5.  **验证效果**：再次运行基准测试，对比优化前后的数据，确保优化有效。

通过这套流程，你可以系统性地提升 Rust 程序的性能，让你的 Ratatui 应用运行得更加流畅。
