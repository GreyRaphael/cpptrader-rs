# CppTrader Rust 移植 —— 设计方案对比分析

> **分析日期**: 2026-06-02
> **场景**: 10,000 个金融标的，NASDAQ ITCH 数据流

---

## 1. 数据结构方案对比

### 方案 A: BTreeMap + VecDeque（原方案）

```
Order → level_price + level_bucket → BTreeMap::get(price) → LevelData
LevelData.order_queue: VecDeque<OrderId>
best_bid: BTreeMap::iter().next_back()
```

### 方案 B: SlotMap + 侵入式链表（新方案）

```
Order → level_idx → SlotMap::get(idx) → LevelData
LevelData: head/tail OrderId 缓存
best_bid: 缓存的 Option<LevelIdx>
```

### 对比表

| 维度 | 方案 A: BTreeMap | 方案 B: SlotMap |
|------|------------------|-----------------|
| 热路径复杂度 | O(log n) ~7-10 次比较 | O(1) ~1-2 次内存访问 |
| 代码复杂度 | 低，大量使用标准库 | 中，需要手写侵入式链表 |
| 内存开销/订单 | 低，无额外索引字段 | +16 bytes (prev/next) |
| 缓存局部性 | 中，BTree 节点内连续 | 高，SlotMap 连续分配 |
| Rust 惯用性 | 高，纯标准库 + hashbrown | 中，侵入式链表需要小心 |
| unsafe 代码 | 几乎不需要 | 侵入式链表可能需要少量 |
| 可维护性 | 高，逻辑清晰 | 中，指针管理复杂 |
| 10,000 标的单线程 | ~900-1500ns/msg | ~400-600ns/msg |
| 10,000 标的 8 核并行 | ~160-270ns/msg | ~70-110ns/msg |
| 相对 C++ 单线程(300ns) | 慢 3-5x / 快 1.2-1.9x | 慢 30-50% / 快 2-3x |

**最终选择: 方案 A (BTreeMap)**
- 代码量少 30-40%
- 几乎不需要 unsafe
- 并行后已经比 C++ 快
- 如果后续发现瓶颈，可以局部替换为 SlotMap

---

## 2. 线程模型对比

### C++ 单线程模型

```cpp
// market_manager.h: Not thread-safe.
// 所有操作在调用线程中同步执行，无锁、无原子操作。
```

### Rust 单线程（Phase 1）

```rust
// MarketManager 不实现 Send/Sync
impl !Send for MarketManager {}
impl !Sync for MarketManager {}
```

### Rust 并行（Phase 2，可选扩展）

```rust
// 按标的分片：不同标的的 OrderBook 可以并行处理
// 同一标的的撮合仍然串行
fn parallel_match(books: &mut [Option<OrderBook>]) {
    books.par_iter_mut()  // rayon 并行迭代
        .filter_map(|ob| ob.as_mut())
        .for_each(|book| book.match_orders());
}
```

### 并行可行性

ITCH 协议中，不同标的（StockLocate）的订单事件天然独立，无需跨标的同步。

**陷阱**：真实 ITCH 数据中，头部股票（AAPL/MSFT/TSLA）交易量远大于尾部，简单按标的分片会导致负载不均衡。需要 work-stealing 或动态负载均衡。

---

## 3. 性能模型

### C++ 基准数据

```
ITCH 处理: ~41.5M msg/s (24ns/msg)
Market Manager: ~3.2M msg/s (309ns/msg), ~7.2M upd/s
优化版: ~8.3M msg/s (120ns/msg)
激进版: ~9.75M msg/s (102ns/msg)
```

### Rust 估算

| 场景 | 原方案 BTreeMap | 新方案 SlotMap |
|------|-----------------|----------------|
| 单线程 | ~0.7-1.1M msg/s | ~1.7-2.5M msg/s |
| 8 核并行 | ~3.7-6.2M msg/s | ~5-9M msg/s |
| 相对 C++ | 并行后快 1.2-1.9x | 并行后快 2-3x |

### 为什么 BTreeMap 比指针慢

```
C++ 指针解引用:
  order->Level = 1-4ns (L1 cache hit)
  _best_bid->Price = 1-4ns

Rust BTreeMap 查找:
  BTreeMap::get(price) = 15-30ns (O(log n), n~100 时 ~7 次比较)
  BTreeMap::iter().next_back() = 15-30ns
```

BTreeMap 节点在堆上分配，不如连续内存池缓存友好。

### 为什么并行能弥补差距

```
单线程 Rust 慢 3-5x → 并行 8 核加速 5-6 倍 → 净效果快 1.2-1.9x
```

即使单线程慢，并行化带来的吞吐量提升足以弥补。

---

## 4. 最终决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 数据结构 | BTreeMap + VecDeque | 简洁、安全、并行后足够快 |
| 线程模型 | Phase 1 单线程，Phase 2 可选并行 | 渐进式优化 |
| HashMap 实现 | hashbrown (SwissTable) | 比 std 更快 |
| 错误处理 | Result<T, ErrorCode> | Rust 惯用模式 |
| 事件系统 | trait + dyn dispatch | 对应 C++ 虚函数 |
| unsafe 使用 | 尽量避免 | 仅在性能瓶颈处考虑 |

---

## 5. 未来优化路径

```
Phase 1: BTreeMap 单线程
  ↓ benchmark 发现瓶颈
Phase 2: 并行化（rayon 按标的分片）
  ↓ benchmark 仍然不够
Phase 3: 热路径 SlotMap 优化
  ↓ 极致性能
Phase 4: unsafe 侵入式链表 + SIMD
```

每一步都是渐进式的，不会一开始就引入不必要的复杂度。
