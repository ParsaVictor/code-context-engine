<div style="direction:rtl;text-align:right;font-family:Tahoma">

# طراحی فنی فاز ۰ — ایزوله‌سازی واقعی پروژه

هدف فاز: **باگ آلودگی گراف هنگام سوییچ/افزودن پروژه دوم را به‌طور قطعی ببندیم** و با تست CI اثباتش کنیم.

معیار «انجام‌شده» (Definition of Done):
> پروژه A (وب Rust) و سپس پروژه B (CV پایتون) را در یک پروسه‌ی MCP باز کن. هر کوئری روی B باید **صفر فایل از A** برگرداند. CI این را اثبات کند (`cross_project_files == 0`).

---

## ۱. ریشه‌ی دقیق باگ (از روی کد baseline)

| # | محل | مشکل |
|---|---|---|
| A | `crates/neuromesh-mcp/src/server.rs` → `adopt_workspace_from_initialize` | قبل از `load_persisted` + `reindex_incremental` **`graph.clear()` صدا زده نمی‌شود**. اگر پروژه‌ی جدید `graph.bin` نداشته باشد، `load_from` مقدار `false` برمی‌گرداند و گراف پروژه‌ی قبلی دست‌نخورده می‌ماند؛ بعد `reindex_incremental` فقط فایل‌های جدید را روی همان گراف اضافه می‌کند. |
| B | `crates/neuromesh-graph/src/graph.rs` → `reindex_incremental` (حدود خط ۱۵۵۳) و `load_from`→`install_snapshot` | `install_snapshot` از `mesh.load_lists` استفاده می‌کند که `clear` می‌کند (خوب)، ولی فقط وقتی snapshot موجود باشد. `reindex_incremental` بر اساس `file_fingerprints` کار می‌کند و گره‌های «خارج از workspace فعلی» را حذف نمی‌کند. |
| C | `crates/neuromesh-core/src/types.rs` → `NodeId` | `NodeId` صرفاً `file:<path>` یا `sym:<path>:<symbol>` است. هیچ پیشوند پروژه ندارد. `ContextNode.project_id` هست ولی جزو کلید `MeshStore.node_of` (`HashMap<NodeId, u32>`) و هیچ‌کدام از ایندکس‌های مشتق (`name_to_nodes`, `file_to_nodes`, `token_to_nodes`, `path_index`, `impl_index`, `export_index`, `concept_index`) نیست. |
| D | `server.rs` → گارد `same_workspace_path` | اگر تشخیص workspace برای A و B به یک مسیر برسد (monorepo، نبود `rootUri`، بایند به `$HOME`)، زودخروج می‌کند و گراف A روی B سرو می‌شود. |
| E | `graph.rs` → `reindex_incremental` حلقه‌ی `infer_workspace_root(file)` + `set_workspace(&root)` | `workspace_root` در حین ایندکس روی هر فایل عوض می‌شود؛ در monorepo نوسان می‌کند. |
| F | `crates/neuromesh-mcp/src/tools.rs` → `McpToolHandler` یک `Arc<NeuralProjectGraph>` | یک گراف زنده در کل عمر پروسه؛ سرو هم‌زمان چند پروژه ممکن نیست. |

---

## ۲. استراتژی دو‌مرحله‌ای

### P0a — «پارتیشن سخت به‌ازای هر پروسه» (کوچک، کم‌ریسک، قابل‌ارسال به upstream)

تضمین می‌کنیم گرافِ هر پروسه **همیشه فقط یک پروژه** را دربردارد. بدون بازنویسی `NodeId`.

### P0b — «Namespace واقعی» (تهاجمی، فقط اگر لازم شد)

`MeshStore` و ایندکس‌های مشتق را project-scoped می‌کنیم تا سرو هم‌زمان چند پروژه ممکن شود.

**تصمیم:** اول P0a کامل + تست + (احتمالاً) PR به upstream. بعد P0b را فقط وقتی سراغش می‌رویم که واقعاً به multi-project هم‌زمان نیاز داشتیم (مثلاً برای demo یا federated retrieval در P2). دلیل: باگی که کاربر دیده با P0a کامل حل می‌شود، و P0a را yoosefap راحت‌تر merge می‌کند تا یک ری‌فکتور ۱۵‌فایلی.

---

## ۳. P0a — تغییرات دقیق

### ۳.۱ `ProjectId` قطعی و پایدار

**فایل جدید:** `crates/neuromesh-core/src/project_id.rs`

```rust
/// شناسه‌ی پایدار پروژه: blake3 روی مسیر canonical ریشه‌ی گیت (یا workspace اگر گیت نبود).
pub fn stable_project_id(workspace_root: &Path) -> ProjectId {
    // 1) اگر <ws>/.neuromesh/id هست، همان را بخوان (پایدار در برابر rename/clone)
    // 2) وگرنه: root = git_toplevel(workspace_root).unwrap_or(workspace_root)
    //    id = hex(blake3(normalize_workspace(root)))[:16]
    //    آن را در <ws>/.neuromesh/id بنویس
}
```

- `blake3` از قبل در `workspace.dependencies` هست.
- `normalize_workspace` از `paths.rs` استفاده شود (canonical + lowercase + strip `//?/`).
- `git_toplevel`: اجرای `git rev-parse --show-toplevel` یا خواندن `.git`؛ اگر نبود، خود مسیر.

### ۳.۲ رجیستری مرکزی پروژه‌ها

**فایل جدید:** `crates/neuromesh-core/src/registry.rs` — `~/.neuromesh/registry.json` (فعلاً JSON؛ SQLite در P1)

```rust
struct ProjectRecord {
    id: ProjectId,
    canonical_path: String,
    slot: String,             // همان project_slot_name فعلی
    lang_profile: Option<String>,
    created_at, last_indexed_at,
    schema_version: u32,
}
struct Registry { projects: Vec<ProjectRecord> }
```

- `Registry::resolve(workspace) -> ProjectRecord` (ثبت اگر نبود).
- `Registry::path_for(id) -> Option<PathBuf>` برای بررسی تطابق.

### ۳.۳ گارد ناوردا (Invariant Guard) در گراف

**فایل:** `crates/neuromesh-graph/src/graph.rs`

```rust
impl NeuralProjectGraph {
    /// هر گره‌ای که file_path آن زیر workspace_root فعلی نیست = نشتی.
    pub fn assert_single_project(&self) -> Result<(), Vec<String>> {
        let data = self.inner.read();
        let Some(root) = &data.workspace_root else { return Ok(()) };
        let bad: Vec<String> = data.mesh.nodes()
            .filter(|n| !path_is_within(&n.file_path, root))
            .map(|n| n.file_path.to_string_lossy().into())
            .collect();
        if bad.is_empty() { Ok(()) } else { Err(bad) }
    }
}
```

- در build دیباگ: `debug_assert!` بعد از هر `reindex_*`.
- در release: اگر گارد شکست → لاگ `error!` + `self.clear(Some(pid))` + reindex تمیز (self-heal).
- `path_is_within`: مقایسه‌ی canonical prefix.

### ۳.۴ سوییچ workspace تمیز

**فایل:** `crates/neuromesh-mcp/src/server.rs` → `adopt_workspace_from_initialize`

ترتیب جدید:
```
1. pid_new = stable_project_id(p_buf)
2. اگر pid_new == graph.project_id()  ➜ فقط reindex_incremental (همان پروژه، تغییرات فایل)
3. اگر فرق دارد:
      graph.clear(Some(pid_new))          ← ✅ اضافه‌شده
      graph.set_workspace(&p_buf)          ← یک‌بار، قفل
      registry.upsert(pid_new, p_buf)
      if !graph.load_persisted(&p_buf) { /* گراف خالی ماند، اوکی */ }
      spawn: graph.reindex_incremental(p_buf, pid_new, max_files)
      graph.assert_single_project()  ➜ لاگ اگر شکست
4. watcher قبلی را متوقف کن، watcher جدید برای p_buf
```

- گارد `same_workspace_path` ➜ جایگزین با مقایسه‌ی `pid`.
- **مشکل E:** در `reindex_incremental` حلقه‌ی `infer_workspace_root` + `set_workspace` حذف شود؛ `workspace_root` پارامتر ورودی و ثابت.

### ۳.۵ رد کردن workspaceهای ناامن / مبهم

**فایل:** `crates/neuromesh-index/src/mcp_workspace.rs` + `walker.rs`

- اگر resolved root یکی از این‌ها بود ➜ **ایندکس نکن، خطای واضح بده**: `$HOME`, ریشه‌ی درایو, `/`, `/tmp`, مسیر بدون هیچ marker.
- اگر چند marker زیر یک root (monorepo): لاگ هشدار + استفاده از `nm.config.json → project.sub_projects` اگر تعریف شده، وگرنه حالت «single project» با هشدار.

### ۳.۶ scope در فیدبک/یادگیری

**فایل:** `crates/neuromesh-mcp/src/learning.rs`, `crates/neuromesh-memory/*`

- `record_feedback` قبل از اعمال، چک کند `touched_nodes` واقعاً در گراف پروژه‌ی فعلی‌اند.
- `neuromesh.json` و `ProjectFact`ها از قبل per-slot‌اند (خوب) — فقط تست اضافه شود.

---

## ۴. تست نشتی (قلب فاز ۰)

**فایل جدید:** `crates/neuromesh-context/tests/cross_project_isolation.rs`

فیکسچرها: `tests/fixtures/iso-web-rust/` (چند فایل axum) و `tests/fixtures/iso-ml-python/` (چند فایل PyTorch: `dataset.py`, `model.py`, `train.py`).

```rust
#[test]
fn switching_projects_does_not_leak_nodes() {
    let g = NeuralProjectGraph::new(ProjectId::new("boot"));
    // پروژه A
    index_fixture(&g, "iso-web-rust");
    let pkt_a = get_context_packet(&g, "how does the router register routes");
    assert!(pkt_a.files.iter().all(|f| f.path.contains("iso-web-rust")));

    // سوییچ به B (بدون graph.bin از قبل — همان سناریوی باگ کاربر)
    switch_workspace(&g, "iso-ml-python");   // معادل adopt_workspace_from_initialize
    let pkt_b = get_context_packet(&g, "why did the model mAP drop");

    let leaked: Vec<_> = pkt_b.files.iter()
        .filter(|f| f.path.contains("iso-web-rust")).collect();
    assert_eq!(leaked.len(), 0, "cross-project leak: {leaked:?}");
    assert!(g.assert_single_project().is_ok());
}
```

سناریوهای دیگر (همه در CI):
1. سوییچ A→B وقتی B قبلاً `neuromesh index` **نشده** ← سناریوی اصلی باگ.
2. سوییچ A→B وقتی B `graph.bin` **دارد**.
3. monorepo: root شامل دو زیرپکیج؛ کوئری زیرپکیج X نباید فایل زیرپکیج Y بدهد (اگر sub_projects تعریف شده).
4. نبود `rootUri`: سرور نباید به `$HOME` بایند شود؛ باید خطای واضح بدهد.
5. فیدبک: `record_feedback` روی گره A بعد از سوییچ به B ← رد شود.

**اسکریپت e2e:** `scripts/leakage_smoke.sh` — دو ریپوی واقعی کوچک، اجرای MCP، بررسی پکت.

---

## ۵. P0b — Namespace واقعی (خلاصه، برای بعد)

اگر به multi-project هم‌زمان نیاز شد:

- `NodeId` موقع ساخت پیشوند بگیرد: `NodeId::scoped(pid, "file:<path>")` → `Arc<str>` = `"<pid>\u{1}file:<path>"` (جداکننده‌ی کنترلی).
- یا `MeshStore.node_of: HashMap<ScopedId, u32>` که `ScopedId = (ProjectId, NodeId)`.
- همه‌ی `BTreeMap<String, Vec<NodeId>>`‌ها → `HashMap<ProjectId, BTreeMap<String, Vec<NodeId>>>`.
- `RuntimeState { graphs: LruCache<ProjectId, Arc<NeuralProjectGraph>>, cap: 3 }` در `neuromesh-mcp`.
- هر ابزار MCP `project_id` را از `initialize`/context جاری resolve کند، نه از state سراسری.
- تخمین: ~۱۵ فایل، ~۳–۴ هفته، ریسک merge بالا با upstream ➜ فقط با دلیل موجه.

---

## ۶. ترتیب کار فاز ۰ (Issueها)

| # | Issue | فایل‌ها | پیش‌نیاز |
|---|---|---|---|
| P0-1 | `stable_project_id` + `.neuromesh/id` + تست واحد | `core/project_id.rs` | — |
| P0-2 | `Registry` (JSON) + resolve/upsert + تست | `core/registry.rs` | P0-1 |
| P0-3 | `assert_single_project` + `path_is_within` + `debug_assert` در reindex | `graph/graph.rs` | — |
| P0-4 | سوییچ تمیز در `adopt_workspace_from_initialize` (clear + قفل workspace + pid compare) | `mcp/server.rs`, `graph/graph.rs` | P0-1، P0-3 |
| P0-5 | حذف نوسان `workspace_root` در `reindex_incremental` | `graph/graph.rs` | — |
| P0-6 | رد workspace ناامن/مبهم + هشدار monorepo + `sub_projects` در کانفیگ | `index/mcp_workspace.rs`, `core/config.rs` | — |
| P0-7 | scope فیدبک/حافظه + تست | `mcp/learning.rs`, `memory/*` | P0-1 |
| P0-8 | **تست نشتی** (۵ سناریو) + فیکسچرها + `scripts/leakage_smoke.sh` + گِیت CI | `context/tests/`, `.github/workflows/ci.yml` | P0-3، P0-4 |
| P0-9 | مستندسازی: `docs/isolation.md` + به‌روزرسانی `docs/mcp.md` (حذف هشدار قدیمی) | `docs/` | همه |

خروجی: تگ `p0-complete`، و یک PR تمیز به `pinoox/neuromesh` شامل P0-3/4/5/8.

---

## ۷. بلاکر فعلی — محیط build

روی این ماشین **Rust و کامپایلر C نصب نیست**. برای توسعه‌ی واقعی لازم است:

- `rustup` (stable) — `winget install Rustlang.Rustup` یا از rustup.rs
- **Visual Studio Build Tools 2022** با workload «Desktop development with C++» (برای گرامرهای tree-sitter و ONNX Runtime) — ~۳–۷ گیگ
- (احتمالاً) `cmake`

گزینه‌ها:
1. **نصب محلی** (توصیه‌شده برای dev واقعی) — بعدش `cargo build` و `cargo test --all`.
2. **CI به‌عنوان حلقه‌ی verify** — تا وقتی محیط محلی آماده نیست، هر تغییر روی برنچ push شود و از GitHub Actions (`ubuntu-latest`) نتیجه بگیریم. کندتر ولی کارراه‌انداز.
3. **WSL2 + Ubuntu** — نزدیک به محیط CI، نصب سبک‌تر از MSVC.

</div>
