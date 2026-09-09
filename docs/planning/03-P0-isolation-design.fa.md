<div style="direction:rtl;text-align:right;font-family:Tahoma">

# طراحی فنی فاز ۰ — ایزوله‌سازی واقعی پروژه

هدف فاز: **باگ آلودگی گراف هنگام سوییچ/افزودن پروژه دوم را به‌طور قطعی ببندیم** و با تست CI اثباتش کنیم.

معیار «انجام‌شده» (Definition of Done):
> پروژه A (وب Rust) و سپس پروژه B (CV پایتون) را در یک پروسه‌ی MCP باز کن. هر کوئری روی B باید **صفر فایل از A** برگرداند. CI این را اثبات کند (`cross_project_files == 0`).

---

## ۱. ریشه‌ی دقیق باگ (از روی کد baseline)

> **تصحیح نسبت به نسخه‌ی اول این سند.** ادعای اولیه این بود که «اگر پروژه‌ی دوم
> `graph.bin` نداشته باشد، گره‌های A و B در یک گراف قاطی می‌شوند». مطالعه‌ی
> دقیق‌تر مسیر ingest نشان داد این ادعا **درست نیست**: `reindex_incremental` →
> `ingest_scan_report` ابتدا `prune_absent_files(present)` را صدا می‌زند، و
> `ScanReport.present` شامل **همه‌ی مسیرهای نسبی نگه‌داشته‌شده‌ی پروژه‌ی جدید**
> است (نه فقط فایل‌های تغییرکرده). پس هر کلید `file_hashes` که در B نباشد
> حذف می‌شود ⟹ حجم عمده‌ی گره‌های A پاک می‌شوند.
>
> نتیجه‌ی درست: **ایزوله‌سازی فعلی «تصادفی» است، نه «تضمین‌شده»** — به
> حسابداری `file_hashes` و به تصادفی‌نبودن مسیرها/محتوا وابسته است، و سه مسیر
> نشتی واقعی باقی می‌ماند (A2، D، G در جدول زیر). این دقیقاً همان چیزی است که
> فاز ۰ باید به «تضمین‌شده» تبدیلش کند.

| # | محل | مشکل | شدت |
|---|---|---|---|
| A1 | `mcp/server.rs` → `adopt_workspace_from_initialize` | قبل از `load_persisted` + `reindex_incremental` `graph.clear()` صدا زده نمی‌شود. در عمل `prune_absent_files` جبرانش می‌کند، ولی این یک **اثر جانبی** است نه یک ضمانت. | متوسط |
| A2 | `graph.rs` → `ingest_file_keep` (early return) | `if file_hashes[rel] == new_hash { return }` — اگر A و B فایلی با **مسیر نسبی یکسان و محتوای بایت‌به‌بایت یکسان** داشته باشند (`LICENSE`، `.gitignore`، `__init__.py` خالی، `Cargo.toml` بویلرپلیت، `README` مشترک)، فایل B اصلاً ingest نمی‌شود و **گره‌های A با `project_id` خودِ A زنده می‌مانند**. | **نشتی واقعی** |
| B | `graph.rs` → `load_from` → `install_snapshot` | `mesh.load_lists` درست `clear` می‌کند — این مسیر سالم است. | ✅ |
| C | `core/types.rs` → `NodeId` | `NodeId` صرفاً `file:<path>` یا `sym:<path>:<symbol>` است، بدون پیشوند پروژه. `ContextNode.project_id` ذخیره می‌شود ولی **در هیچ کوئری فیلتر نمی‌شود** و جزو کلید `MeshStore.node_of` یا ایندکس‌های مشتق نیست. یعنی هیچ لایه‌ی دفاعی دومی وجود ندارد. | ساختاری |
| D | `server.rs` → گارد `same_workspace_path` | اگر تشخیص workspace برای A و B به یک مسیر برسد (monorepo، نبود `rootUri`، بایند به `$HOME`)، **زودخروج می‌کند و کل گراف A روی B سرو می‌شود**. هیچ pruning‌ای اجرا نمی‌شود چون هیچ reindex‌ای اجرا نمی‌شود. | **نشتی کامل** |
| E | `graph.rs` → `ingest_workspace_inner` | `if let Some((file, _)) = scanned.first() { set_workspace(infer_workspace_root(file)) }` — `workspace_root` که `reindex_incremental` تازه به‌درستی ست کرده بود، با حدسی از **اولین فایل بچ** بازنویسی می‌شود. (اصلاح: این «به‌ازای هر فایل در حلقه» نیست، فقط اولین فایل بچ است.) | متوسط |
| G | `mcp/server.rs:158` و `cli/main.rs:164` | `ProjectId::new(&p_name)` که `p_name = path.file_name()` — **شناسه‌ی پروژه از نام پوشه ساخته می‌شود**. دو چک‌اوت متفاوت با نام `app`/`api`/`backend` یک شناسه می‌گیرند. `project_id` روی گره‌ها را بی‌اثر و هر مقایسه‌ی «همان پروژه است؟» را غلط می‌کند. | **نشتی واقعی** — با #1 حل شد |
| F | `mcp/tools.rs` → `McpToolHandler` یک `Arc<NeuralProjectGraph>` | یک گراف زنده در کل عمر پروسه؛ سرو هم‌زمان چند پروژه ممکن نیست. | محدودیت |

**پیامد برای طراحی:** چون A2 و D سناریوهای باریک ولی واقعی‌اند، تست نشتی (P0-8)
نباید صرفاً «سوییچ کن و امیدوار باش» باشد؛ باید **عمداً** این حالت‌ها را بسازد:
فایل‌های هم‌مسیر و هم‌محتوا بین دو فیکسچر، و سناریوی سوییچِ رد‌شده.
و ارزش P0-3 (گارد ناوردا) بالاتر می‌رود: ایزوله‌سازی تصادفی را با استفاده از
`project_id` که **از قبل روی هر گره ذخیره است** به ایزوله‌سازی تضمین‌شده تبدیل می‌کند.

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

نکته‌ی کلیدی: `ContextNode.file_path` **نسبی** است (`src/main.rs`)، پس چک
«زیر `workspace_root` است؟» برایش کار نمی‌کند. ولی `ContextNode.project_id`
**از قبل روی هر گره ذخیره می‌شود** — پس گارد هم دقیق است و هم رایگان (`O(n)`
بدون I/O):

```rust
impl NeuralProjectGraph {
    /// هر گره‌ای که project_id آن با پروژه‌ی فعلی گراف فرق دارد = نشتی.
    /// خروجی: فهرست (project_id بیگانه، مسیر) برای گزارش.
    pub fn foreign_nodes(&self) -> Vec<(String, String)> { … }

    pub fn assert_single_project(&self) -> std::result::Result<(), Vec<(String, String)>> {
        let foreign = self.foreign_nodes();
        if foreign.is_empty() { Ok(()) } else { Err(foreign) }
    }

    /// حذف هر گره‌ی بیگانه؛ تعداد حذف‌شده را برمی‌گرداند. مسیر self-heal.
    pub fn evict_foreign_nodes(&self) -> usize { … }
}
```

- در build دیباگ: `debug_assert!` بعد از هر `reindex_*`.
- در release: اگر گارد شکست → `tracing::error!` با تعداد و نمونه‌ها +
  `evict_foreign_nodes()` (self-heal بدون از دست دادن کل ایندکس).
- این دقیقاً سناریوی A2 (فایل هم‌مسیر و هم‌محتوا) را می‌گیرد، چون آن گره‌ها
  `project_id` پروژه‌ی قبلی را با خود دارند.
- `path_is_within` (در `neuromesh-core::project_id`) برای مقایسه‌ی مسیرهای
  **مطلق** می‌ماند — تشخیص workspace در P0-6.

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
