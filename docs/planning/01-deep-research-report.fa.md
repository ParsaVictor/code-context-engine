


<div style="direction:rtl;text-align:right;font-family:B Lotus, B Nazanin, Tahoma">


# گزارش تحقیق عمیق: NeuroMesh و نقشه راه NeuroMesh 2.0




نسخه گزارش: ۱.۰ — تاریخ: ۲۰۲۶/۰۹/۰۹
منبع اصلی بررسی: کلون کامل مخزن `pinoox/neuromesh` روی شاخه `main` (کامیت `fe65e18`، نسخه `v0.9.0`)
فورک کاری شما: `github.com/ParsaVictor/neuromesh`

---

## ۰. خلاصه اجرایی (Executive Summary)

**۱. سازنده پروژه قطعی است.** NeuroMesh را **یوسف علی‌پور** (`yoosefap` در گیت‌هاب، هم‌بنیان‌گذار و مدیرعامل Pinoox، مقیم بوشهر) استارت زده و تقریباً تک‌نفره توسعه داده است: از **۱۹۰ کامیت**، همه به‌جز چند مورد جزئی زیر دو نام گیت (`yoosefap` و `yoosef alipour`) اما یک ایمیل (`yoosefalipour@gmail.com`) ثبت شده‌اند. کل پروژه در یک اسپرینت فشرده **۲۲ تا ۳۱ اوت ۲۰۲۶** (حدود ۱۰ روز) نوشته شده. پس گزارش دوم شما که او را «فقط maintainer فعال» می‌دانست بیش از حد محتاط بود؛ او **بنیان‌گذار و تنها نویسنده اصلی** است.

**۲. نقص «تک‌پروژه‌ای» که پیدا کردید واقعی است، اما نه به آن شکلی که تصور می‌شد.** برخلاف تصور اولیه، NeuroMesh از نسخه `v0.6.0` یک **Managed Store** دارد که هر پروژه را زیر `~/.neuromesh/projects/<slug>-<sha256[:8]>` جدا می‌کند (دقیقاً همان ایده «هش مسیر پروژه» که در تحقیق‌های شما پیشنهاد شده بود — از قبل پیاده شده). نقص واقعی در **سه لایه دیگر** است:
   - **رانتایم MCP فقط یک گراف زنده در RAM دارد** و هنگام سوییچ پروژه آن را «جهش» می‌دهد، نه Hot-Swap تمیز.
   - در تابع `adopt_workspace_from_initialize` **قبل از ایندکس مجدد `graph.clear()` صدا زده نمی‌شود**؛ اگر پروژه دوم قبلاً `neuromesh index` نشده باشد یا تشخیص workspace اشتباه باشد، گره‌های پروژه A با پروژه B **در یک گراف قاطی می‌شوند** (Graph Contamination واقعی).
   - **`NodeId` و `EdgeId` هیچ namespace پروژه‌ای ندارند** (`file:src/main.rs`, `sym:path:symbol`). فیلد `project_id` روی `ContextNode`/`ContextEdge` هست ولی در **هویت گره و در فیلترهای کوئری استفاده نمی‌شود**. این دقیقاً همان چیزی است که گزارش دوم شما درست تشخیص داده بود: ساختارهای symbol/name/token/path در عمل corpus-wide هستند.

**۳. «Universal» بودن هنوز فاصله دارد.** NeuroMesh از قبل tree-sitter دارد (پس «مهاجرت به tree-sitter» که در تحقیق‌ها آمده، لازم نیست — قبلاً انجام شده). ولی:
   - پوشش گرامر محدود است: ~۱۲ زبان. **C/C++ در enum هست ولی گرامر ندارد**؛ R، Julia، Scala، MATLAB، Lua اصلاً نیستند.
   - **نوت‌بوک `.ipynb` پارس نمی‌شود.**
   - **لایه Config→Code وجود ندارد** (YAML/Hydra هایپرپارامترها به کد آموزش وصل نمی‌شوند).
   - `NodeType`/`EdgeType` مفاهیم ML ندارند (Dataset، Model، Checkpoint، Experiment، Metric، Pipeline stage / produces / consumes / trained_by).
   - همه Overlayهای فریم‌ورک وب‌محور‌اند (Laravel، Django، Next، Vue، Axum، Rails، Flutter) — هیچ Overlay برای PyTorch/Lightning/HF Transformers/sklearn نیست.

**۴. عدد «۹۰٪ کاهش توکن» را نباید هدف بگذاریم.** خود پروژه در README دو نمونه `97.3%` و `99.4%` را گزارش کرده که **اعداد داخلی و per-task هستند، نه بنچمارک مستقل**. پژوهش‌های ۲۰۲۵–۲۰۲۶ (Chroma Context Rot، Lost-in-the-Middle، CodeRAG-Bench، «When Retrieval Hurts Code Completion») نشان می‌دهند **context بیشتر ⟵⟶ نتیجه بهتر نیست** و retrieval آلوده نتیجه را بدتر می‌کند. هدف حرفه‌ای‌تر: **Maximum Task Success per Token** با معیار `Cross-Project Leakage = 0`.

**۵. اولویت پیشنهادی (هم‌راستا با گزارش دوم شما):**
   - **P0 — اول جلوی خطا را بگیر:** Project Isolation واقعی (namespace در NodeId، فیلتر کوئری، clear روی سوییچ، Registry، تست نشتی).
   - **P1 — بعد دانش را زیاد کن:** Universal Artifact Graph + پارسر نوت‌بوک/کانفیگ + Hybrid Retrieval + Reranker.
   - **P2 — بعد فشرده‌سازی را عمیق‌تر کن:** LLMLingua-2 اختیاری، لایه دانش چندپروژه‌ای.

---

## ۱. کالبدشکافی NeuroMesh (وضعیت فعلی)

### ۱.۱ سازنده و تاریخچه

| مورد | یافته |
|---|---|
| بنیان‌گذار | یوسف علی‌پور (`yoosefap`) — Co-Founder & CEO شرکت Pinoox، بوشهر |
| سازمان میزبان | `pinoox` (همان تیم فریم‌ورک PHP HMVC به نام Pinoox) |
| شروع | ۲۲ اوت ۲۰۲۶ — کامیت `cd0bf4a` با پیام «Initial release of NeuroMesh v2.0 - Biomimetic MCP Context Engine & Visual Runtime» |
| حجم فعالیت | ۱۹۰ کامیت در بازه ۲۲–۳۱ اوت ۲۰۲۶ |
| نویسندگان | عملاً یک نفر (۱۵۸ + ۳۲ کامیت زیر یک ایمیل) |
| نسخه فعلی | `v0.9.0` (۳۰ اوت ۲۰۲۶) |
| زبان | Rust (workspace با ۱۷ crate) |
| مجوز | MIT OR Apache-2.0 |
| محبوبیت | ~۸۵ ستاره، ۸ فورک، ۰ ایشو باز (در زمان بررسی) |
| پروژه هم‌نام | یک `NeuroMesh-ai` جدا وجود دارد؛ ربطی به این ندارد. |

نکته: تگ‌ها از `v0.3.0` شروع می‌شوند و «v2.0» در پیام کامیت اول احتمالاً به بازنویسی داخلی یک نمونه قبلی اشاره دارد، نه یک نسخه عمومی منتشرشده.

### ۱.۲ معماری فعلی

**۱۷ crate در یک workspace:**

| Crate | نقش | LOC تقریبی |
|---|---|---|
| `neuromesh-context` | هسته انتخاب کانتکست: retrieval لایه‌ای، fold registry، اسکلت‌سازی، بهینه‌ساز ژنتیک، gold harness | ~۱۶٬۸۰۰ |
| `neuromesh-parser` | رجیستری زبان، کوئری‌های tree-sitter، overlayهای فریم‌ورک، fallback رجکس | ~۹٬۷۰۰ |
| `neuromesh-graph` | گراف عصبی: ingest، search، trace، Physarum، STDP، sidecar امبدینگ | ~۹٬۲۰۰ |
| `neuromesh-cli` | دستورات `mcp`، `index`، `monitor`، `connect`، `doctor`، `eval` | ~۴٬۷۰۰ |
| `neuromesh-mcp` | سرور JSON-RPC 2.0 روی stdio | ~۴٬۶۰۰ |
| `neuromesh-core` | تایپ‌های مشترک: `ProjectId`، `NodeId`، `NodeType`، `EdgeType`، بودجه توکن | ~۲٬۹۰۰ |
| `neuromesh-api` | مانیتور HTTP/SSE محلی + UI «کهکشان عصبی سه‌بعدی» | ~۱٬۹۰۰ |
| `neuromesh-graph-proxy` | بک‌اند گراف خارجی (CBM) — اختیاری | ~۱٬۷۰۰ |
| `neuromesh-index` | Walker فایل‌ها، هش‌ها، تشخیص زبان از مسیر، `mcp_workspace` | ~۱٬۶۰۰ |
| `neuromesh-memory` | حقایق پروژه از manifestها و مستندات، حافظه اپیزودیک | ~۱٬۵۰۰ |
| `neuromesh-embed` | MiniLM از طریق `fastembed` + ONNX Runtime، کش کوئری، LRU معنایی | ~۱٬۱۰۰ |
| بقیه | `provider`، `observability`، `cache` (مایسلیوم/prefetch)، `task`، `router` (QualityGate)، `local-ai` | جمعاً ~۳٬۰۰۰ |

**خط لوله (Pipeline):**

```
Prompt (هر زبانی)
  → graph_backend: native (پیش‌فرض) یا proxy_cbm
  → retrieval.engine: fast (پیش‌فرض) | hybrid | deep
      fast  : QueryPlan + seedهای lexical/graph، بدون embedding
      hybrid: embed کوئری با MiniLM → ANN سلسله‌مراتبی (file → symbol تنبل)
      deep  : embed همه symbolها موقع rebuild → ANN مسطح
  → Tiered retrieval: L1 (seed) → L2 (الگوهای pattern) → L3 (بازیابی معنایی محدود)
  → seed files همیشه ارسال می‌شوند (اسکلتون‌شده)
  → پر کردن callee/usage/import زیر سقف توکن (fill_cap)
  → Evidence Packet (فولدشده) → کلاینت MCP
  → expand_fold بدنه‌ی جمع‌شده را از رجیستری برمی‌گرداند
```

**ابزارهای MCP:** `get_context_packet` (اصلی)، `neuromesh_expand_fold`، `neuromesh_search_symbols`، `neuromesh_trace`، `neuromesh_analyze_impact`، `neuromesh_get_dependencies`، `neuromesh_get_file_skeleton`، `neuromesh_get_architecture`، `neuromesh_get_project_memory`، `neuromesh_record_feedback`، `neuromesh_get_node_weights`، `neuromesh_expand_gap`، `neuromesh_explain_packet`، `neuromesh_get_stats`.

**ایده متمایزکننده — «Fold به‌جای Delete»:** به‌جای حذف کد اضافه، بدنه توابع غیرلازم به یک مارکر یک‌خطی برگشت‌پذیر جمع می‌شود:
```
/* [neuromesh:fold:fold_unused_helper_1 | 12 lines folded | fn unused_helper()] */
```
این نکته قوت اصلی پروژه نسبت به رقباست: مدل «شکل» فایل (امضاها، importها، همسایه‌ها) را می‌بیند بدون پرداخت هزینه هر helper خصوصی.

### ۱.۳ وضعیت واقعی ایزوله‌سازی پروژه — تحلیل کد

**چیزی که از قبل هست (خوب):**

- `crates/neuromesh-core/src/paths.rs`:
  - `enum ProjectStore { Managed (پیش‌فرض), Local (legacy) }`
  - `normalize_workspace(path)` → مسیر canonical و lowercase
  - `project_slot_name` = `<slug>-<sha256(normalized_path)[:8]>`
  - `project_data_dir` = `~/.neuromesh/projects/<slot>/` شامل `graph.bin`، `embeddings.bin`، `neuromesh.json`، `config.json`، `workspace.json`
  - مهاجرت خودکار از `.neuromesh/` داخل ریپو (legacy) به slot مدیریت‌شده
- `crates/neuromesh-index/src/mcp_workspace.rs`:
  - `resolve_mcp_startup_workspace()` از envهای IDE (`WORKSPACE_FOLDER_PATHS`, `VSCODE_CWD`, `CURSOR_WORKSPACE`, …) و بعد از cwd
  - `best_project_root()` اولین مسیری که marker دارد (`.git`, `Cargo.toml`, `package.json`, `pyproject.toml`, `manage.py`, `go.mod`, `composer.json`) را انتخاب می‌کند
- `crates/neuromesh-mcp/src/server.rs::adopt_workspace_from_initialize`:
  - از `rootUri`/`workspaceFolders` در پیام `initialize` workspace را «adopt» می‌کند
  - اگر مسیر جدید == مسیر فعلی بود، زودخروج (`same_workspace_path`)
  - `set_project_id` + `set_workspace` + `load_persisted` + `reindex_incremental` در پس‌زمینه + راه‌اندازی `WorkspaceWatcher`

**نقص‌های واقعی (این‌ها را باید حل کنیم):**

| # | نقص | محل کد | پیامد |
|---|---|---|---|
| A | **قبل از سوییچ workspace، `graph.clear()` صدا زده نمی‌شود.** فقط `load_persisted` (که اگر `graph.bin` وجود داشته باشد کل mesh را replace می‌کند) و بعد `reindex_incremental` (که فقط فایل‌های تغییر‌کرده را می‌سازد و گره‌های پروژه قبلی را حذف نمی‌کند). | `server.rs:160-175` | اگر پروژه دوم `graph.bin` نداشته باشد (هرگز `neuromesh index` نشده)، گره‌های A + B در یک گراف قاطی می‌شوند → **دقیقاً همان خرابی که شما دیدید**. |
| B | **`NodeId`/`EdgeId` بدون namespace پروژه.** `NodeId::from_file_path` = `file:<path>`، `NodeId::from_symbol` = `sym:<path>:<symbol>`. | `core/src/types.rs:29-60` | دو پروژه با فایل هم‌نام (`src/main.rs`, `README.md`, `utils.py`) در یک store مشترک برخورد می‌کنند. |
| C | **`project_id` روی node/edge ذخیره می‌شود ولی در کوئری فیلتر نمی‌شود.** search/trace/activation روی کل `data.mesh` کار می‌کنند. | `graph/src/graph.rs` (توابع `search`, `spreading_activation`, …) | حتی اگر گراف قاطی شود، هیچ لایه دفاعی دومی نیست. |
| D | **زودخروج `same_workspace_path`.** اگر تشخیص workspace هم برای A و هم B به یک مسیر برسد (مثلاً هر دو به `$HOME` یا ریشه monorepo)، هرگز سوییچ نمی‌کند و گراف A به B سرو می‌شود. | `server.rs:148-152` | در monorepo یا وقتی IDE `rootUri` نمی‌فرستد. |
| E | **`reindex_incremental` داخل حلقه `set_workspace` را عوض می‌کند** (`infer_workspace_root(file)` روی هر فایل). | `graph.rs:1613-1614` | در monorepo با markerهای تودرتو، workspace root در حین ایندکس نوسان می‌کند. |
| F | **رانتایم فقط یک `NeuralProjectGraph` زنده دارد.** سرو هم‌زمان چند پروژه ممکن نیست؛ هر سوییچ یعنی reindex کامل. | `mcp/src/tools.rs` (یک `Arc<McpToolHandler>`) | برای کاربری که بین چند ریپو جابه‌جا می‌شود، کند و مستعد خطا. |
| G | **هشدار مستندات خودشان:** «هرگز `neuromesh mcp` را بدون مسیر workspace اجرا نکنید… وگرنه سرور ممکن است به home directory بایند شود و پروژه‌های نامرتبط را ایندکس کند.» (`docs/mcp.md:7`) | — | پذیرش رسمی وجود مشکل. |

**جمع‌بندی لایه ایزوله‌سازی:** NeuroMesh یک ایزوله‌سازی **درشت، مبتنی بر مسیر فایل، و تک‌پروژه‌ی فعال** دارد. کم دارد: (۱) scoping سخت داخل ساختار داده و کوئری، (۲) سرو هم‌زمان چند پروژه، (۳) تشخیص مقاوم workspace برای monorepo/multi-root، (۴) مجموعه تست نشتی (leakage).

### ۱.۴ محدودیت‌های Universal بودن

| حوزه | وضعیت فعلی | فاصله |
|---|---|---|
| زبان‌ها | tree-sitter 0.24 + گرامرهای: Rust, TS/JS, Python, Go, Java, Kotlin, PHP, C#, Swift, Dart, Ruby | C/C++ (enum دارد، گرامر ندارد)، R، Julia، Scala، MATLAB، Lua، Bash، Nix، Zig نیستند |
| نوت‌بوک | — | `.ipynb` هیچ‌جا پارس نمی‌شود؛ برای ML/Colab حیاتی است |
| Config as graph | فقط `.env.example` overlay و خواندن manifest برای «حقایق پروژه» | YAML/Hydra/OmegaConf/TOML → آرگومان توابع Python وصل نمی‌شود |
| مفاهیم ML در گراف | `NodeType`: Project, Directory, File, Component, Class, Function, Symbol, Import, Dependency, Api, DbModel, Test, Config, Doc, Task, Decision, Memory, StyleToken | Dataset, DataLoader, Model, Layer, Loss, Optimizer, Checkpoint, Experiment, Run, Metric, Pipeline/Stage, Notebook, Cell نیست |
| Edgeها | `Imports, Calls, References, Contains, DependsOn, ModifiedWith, TestedBy, RelatedTo, UsedBy, PreviouslySuccessfulWith` | `produces, consumes, reads, writes, trains, evaluates, configures, checkpoints, logs_metric` نیست |
| Overlayهای فریم‌ورک | Laravel, Django, Next, Vue, Axum, Rails, Flutter, Spring, ASP.NET, SwiftUI | PyTorch, Lightning, HF Transformers/Datasets, sklearn Pipeline, TF/Keras, JAX/Flax, DVC, MLflow, W&B نیست |
| رزولوشن نام | lexical + الگوهای دستی + امبدینگ اختیاری | رزولوشن دقیق cross-file/cross-module (مثل SCIP یا stack-graphs) نیست → دقت روی کدبیس بزرگ محدود |

### ۱.۵ ارزیابی ادعای کاهش توکن

- README: «Savings are per task, after folding — not a marketing average.» — یعنی خودشان هم قبول دارند عدد میانگین نیست.
- دو نمونه گزارش‌شده روی یک monorepo ۶۵۰k توکنی: `97.3%` و `99.4%` صرفه‌جویی، `0` گرپ اضافه، ۲۲ و ۱۲ میلی‌ثانیه.
- **این‌ها اعداد خود پروژه‌اند، بدون بنچمارک مستقل عمومی.** برای اسپانسر/محصول باید بنچمارک تکرارپذیر روی SWE-bench / CodeRAG-Bench داشته باشیم.

---

## ۲. چشم‌انداز رقابتی (Competitive Landscape)

### ۲.۱ دسته‌بندی

| دسته | نمونه‌های شاخص | ایده کلیدی | قوت | ضعف | چه چیزی قابل استفاده مجدد است |
|---|---|---|---|---|---|
| **Repo Map (AST + رتبه‌بندی گراف)** | **Aider RepoMap** | tree-sitter → tagهای def/ref → گراف جهت‌دار فایل↔سمبل → **Personalized PageRank** با restart vector سمت سمبل‌های چت → binary-search روی بودجه توکن | دقیق در کاهش ۷۰–۹۰٪ بدون افت، ۱۳۰+ زبان (کوئری‌های `.scm`) | وابسته به ترمینال Aider، خروجی فقط «نقشه» است نه پکت فولدشده | **الگوریتم PageRank شخصی‌سازی‌شده + منطق بودجه** (Apache-2.0) — مستقیم قابل پیاده‌سازی در `neuromesh-graph` |
| **Code Graph MCP** | **CodeGraph** (colbymchenry), **code-graph-mcp** (sdsrss), **CodeGraphContext**, **Semble/Repowise** | tree-sitter → گراف call/dependency در **SQLite با FTS** → یک ابزار MCP که سورس + call chain + blast radius را یک‌جا برمی‌گرداند؛ file-watch برای آپدیت زنده | راه‌اندازی ساده، per-project index، اعداد مستقل (۴۴–۶۲٪ کاهش، ۵۸٪ کمتر tool call روی ۷ ریپو) | گراف ساده‌تر از NeuroMesh (بدون fold/skeleton، بدون یادگیری) | **الگوی per-project SQLite index + طرح ابزار MCP یکپارچه**؛ تأیید می‌کند مسیر «index جدا برای هر پروژه» درست است |
| **Semantic / LSP** | **Serena** (oraios) | به‌جای index ایستا، **Language Server واقعی (LSP)** را اجرا می‌کند: `find_symbol`, `find_referencing_symbols`, `insert_after_symbol`؛ ۳۰+ زبان | رزولوشن کامپایلر-دقیق، ادیت سمبلی اتمیک، رفکتور چندمرحله‌ای | نیاز به LSP نصب‌شده per language، کندتر برای کوئری سریع | **ایده Adapter LSP به‌عنوان یک backend رزولوشن** برای دقت بالا؛ MIT |
| **Precise Indexing / Name Resolution** | **Sourcegraph SCIP**, **GitHub stack-graphs / tree-sitter-stack-graphs** (Rust) | SCIP: فرمت index مبتنی بر پروتکل‌بافر برای ناوبری دقیق cross-repo. stack-graphs: قواعد رزولوشن نام به‌صورت **incremental و بدون build** | دقت فوق‌العاده، cross-repo، incremental | راه‌اندازی سنگین، هر زبان قواعد TSG جدا | **crate `tree-sitter-stack-graphs` + فرمت `scip`** — هر دو Rust/Apache-2.0. می‌توان indexهای SCIP موجود (rust-analyzer, scip-python, scip-typescript, scip-clang) را ingest کرد |
| **Context Bundlers** | **Repomix** (سابقاً Repopack) | کل ریپو → یک فایل متنی فشرده ساختاریافته (XML/MD) با شمارش توکن و فیلتر | ساده، همه زبان‌ها، بدون سرور | فشرده‌سازی فقط لغوی؛ کاهش توکن کمتر؛ وقتی ریپو در context جا نشود بی‌فایده | **الگوی شمارش توکن + فیلترهای include/ignore + خروجی ساختاریافته** |
| **Post-Retrieval Compression** | **Microsoft LLMLingua / LongLLMLingua / LLMLingua-2** | حذف توکن‌های کم‌اطلاعات با یک مدل کوچک (perplexity) یا طبقه‌بندی توکن (LLMLingua-2, task-agnostic، سریع)؛ تا ۲۰x فشرده‌سازی | بعد از retrieval اعمال می‌شود، مکمل fold است | برای کد باید محتاط بود (خراب‌کردن نحو)؛ نیاز به مدل جانبی | **LLMLingua-2 (MIT)** به‌عنوان مرحله اختیاری بعد از اسکلت‌سازی؛ ONNX یا سایدکار Python |
| **Hybrid Retrieval / GraphRAG** | **Microsoft GraphRAG**, lxDIG-MCP, semtree, **Potpie** (Neo4j، $2.2M pre-seed)، **blarify** | ترکیب Graph + BM25 + Vector + reranking؛ GraphRAG: community detection + خلاصه‌های سلسله‌مراتبی | درک ساختاری + معنایی هم‌زمان، مقیاس بالا (Potpie: کدبیس ۴۰M خط) | هزینه ایندکس اولیه، Neo4j سنگین | **الگوی community detection برای «لایه دانش»**؛ الگوی reranker دو مرحله‌ای |
| **Experience / Memory** | **SWE Context Bench**, CodeSage, GraphFlow | انتخاب حافظه/تجربه درست از خود حافظه مهم‌تر از داشتن حافظه است | جهت‌گیری علمی درست | نوپا | **CodeSage: precedent مستقیم per-project local index**؛ SWE Context Bench: معیار ارزیابی |

### ۲.۲ اجزای «آماده و اثبات‌شده» که مستقیم استفاده می‌کنیم (اولویت شما)

بر اساس درخواست شما که «اول از معماری و کدهای آماده صد‌درصد درست استفاده کنیم»:

| مؤلفه | منبع | مجوز | زبان | نقش در NeuroMesh 2.0 |
|---|---|---|---|---|
| `tree-sitter` + گرامرها | tree-sitter | MIT | Rust (binding) | پارس نحوی — **از قبل استفاده می‌شود**، فقط گرامر اضافه کنیم (C, C++, R, Julia, Scala, Lua, Bash, TSX، …) |
| `tree-sitter-stack-graphs` | github/stack-graphs | MIT/Apache-2.0 | Rust | رزولوشن نام دقیق و incremental — جایگزین منطق دستی `identifiers.rs`/`calls.rs` برای زبان‌های پشتیبانی‌شده |
| `scip` crate + SCIP indexers | Sourcegraph | Apache-2.0 | Rust | ingest indexهای دقیق موجود (rust-analyzer `--scip`, scip-python, scip-typescript, scip-clang) وقتی موجودند |
| الگوریتم RepoMap (PageRank شخصی‌سازی‌شده + بودجه) | Aider | Apache-2.0 | Python (پورت به Rust) | رتبه‌بندی سمبل‌ها برای seed؛ جایگزین/مکمل `physarum` و `spreading_activation` |
| `tantivy` | quickwit-oss | MIT | Rust | ایندکس BM25 برای retrieval لغوی (به‌جای پیاده‌سازی دستی fa lexical) |
| `fastembed-rs` | Qdrant | Apache-2.0 | Rust | امبدینگ محلی — **از قبل استفاده می‌شود** |
| ANN: `usearch` یا `hnsw_rs` | unum / rust-cv | Apache-2.0 / MIT | Rust/C++ | جایگزین ANN دستی SIMD فعلی (اختیاری، اگر benchmark ببازد) |
| `redb` یا SQLite (`rusqlite` + bundled) | cberner / SQLite | MIT / Public Domain | Rust | **ذخیره‌گاه per-project** به‌جای صرفاً `graph.bin` تک‌فایلی — تراکنش، کوئری، migration |
| `LLMLingua-2` | Microsoft | MIT | Python / ONNX | فشرده‌سازی اختیاری بعد از fold |
| `nbformat` schema + الگوی `jupytext` | Jupyter / mwouts | BSD / MIT | — | نرمال‌سازی `.ipynb` → سلول‌های ترتیبی + DEF-USE chain |
| الگوی community detection GraphRAG | Microsoft | MIT | Python (الگو) | «لایه دانش» و خلاصه‌های ماژول |

> نکته درباره **Kuzu** (که در تحقیق‌های شما به‌عنوان گزینه graph DB مطرح شده): اکتبر ۲۰۲۵ بعد از خرید توسط Apple **آرشیو شد**. توصیه: برای ذخیره‌گاه از **SQLite (bundled)** یا **redb** استفاده کنیم که پایدار و اثبات‌شده‌اند؛ گراف را در RAM نگه داریم و فقط snapshot/query را به DB بسپاریم.

---

## ۳. یافته‌های پژوهشی مرتبط و دلالت‌ها

| یافته | منبع | دلالت برای NeuroMesh 2.0 |
|---|---|---|
| **Lost in the Middle**: دقت مدل تابع U-شکل موقعیت اطلاعات است؛ افت >۳۰٪ وقتی اطلاعات وسط context باشد. تأییدشده روی ۶+ خانواده مدل. | Liu et al. 2023 + تکرارها ۲۰۲۵ | ترتیب‌دهی پکت مهم است: seedهای اصلی را **اول و آخر** بگذار، sidecarها وسط. |
| **Context Rot** (Chroma، ۲۰۲۵): ۱۸ مدل شامل GPT-4.1/Claude 4/Gemini 2.5 — همه با افزایش طول ورودی کم‌اعتمادتر می‌شوند؛ context مؤثر گاهی تا ۹۹٪ کمتر از حداکثر تبلیغ‌شده. | Chroma Research | «کمتر ولی دقیق‌تر» را به‌عنوان اصل طراحی تثبیت کن؛ coverage proof را جدی بگیر. |
| **When Retrieval Hurts Code Completion** (۲۰۲۶): context ریپوی کهنه/نامرتبط، completion را **بدتر** می‌کند. | arXiv 2605.14478 | فولد/سایدکار کهنه را invalidate کن؛ رتبه‌بندی edge بر اساس «اخیراً ویرایش‌شده». |
| **CodeRAG-Bench** (NAACL 2025): ارزیابی ۱۰ retriever × ۱۰ مدل؛ retrieval فقط در برخی سناریوها کمک می‌کند. | Zhang et al. | بنچمارک ما باید سناریو-محور باشد (repo-level vs. basic vs. open-domain). |
| **stack-graphs**: رزولوشن نام «در مقیاس»، incremental، بدون build. | Creager et al., GitHub | مسیر درست برای دقت روی کدبیس بزرگ بدون کندی. |
| **CodeSage / per-project index**: precedent برای «هر پروژه index جدا». | — | تأیید مستقیم اولویت P0 شما. |

**نتیجه‌گیری راهبردی:** هدف را از «۹۰٪ کاهش توکن» به این تغییر بده:

```
Task Success ↑   |   Retrieval Precision ↑   |   Coverage ↑
Token Cost ↓     |   Latency ↓               |   Cross-Project Leakage = 0
```

---

## ۴. معماری پیشنهادی NeuroMesh 2.0

### ۴.۱ اصل حاکم

> **یک Runtime، پروژه‌های کاملاً ایزوله.**
> `Graph(A) ⟂ Graph(B)`، `Embedding(A) ⟂ Embedding(B)`، `Memory/Feedback/Cache/Fold(A) ⟂ (B)` — ولی یک پروسه واحد همه را مدیریت می‌کند و بین‌شان Hot-Swap تمیز دارد.

```
                    MCP Gateway  (Cursor / Claude / Codex / …)
                              │
                     Project Resolver  ← rootUri, env, .neuromesh/id, git root
                              │
                   ┌──────────▼───────────┐
                   │   Project Registry   │   ~/.neuromesh/registry.sqlite
                   │  id ↔ path ↔ slot ↔  │   (فهرست همه پروژه‌های شناخته‌شده)
                   │  lang profile ↔ lock │
                   └──────────┬───────────┘
                              │  LRU از N گراف زنده (مثلاً ۳)
              ┌───────────────┼────────────────┐
              ▼               ▼                ▼
        Project A         Project B        Project C
        ─────────         ─────────        ─────────
        Artifact Graph    Artifact Graph   Artifact Graph
        BM25 index        BM25 index       ...
        ANN sidecar       ANN sidecar
        Memory / Feedback Memory / Feedback
              │
              ▼
        Hybrid Retrieval:  Symbol(stack-graphs/SCIP) + BM25(tantivy) + ANN(fastembed) + Graph hops
              │
              ▼
        Reranker  (cross-encoder کوچک یا امتیازدهی ترکیبی)
              │
              ▼
        Coverage Proof  (هر seed رزولوو شد؟ gap کجاست؟)
              │
              ▼
        Context Compiler:  Skeleton + Fold + Budget + (LLMLingua-2 اختیاری)
              │
              ▼
        MCP Packet  (seedها اول/آخر، sidecar وسط)
```

### ۴.۲ لایه P0 — ایزوله‌سازی واقعی پروژه

**۱. `ProjectId` قطعی و پایدار:**
- `ProjectId = blake3(canonical_git_root_or_workspace_path)` → hex ۱۶ کاراکتری.
- یک فایل `<workspace>/.neuromesh/id` (فقط شناسه، نه گراف) نوشته شود تا اگر مسیر عوض شد (rename/clone) شناسه ثابت بماند؛ اگر نبود از مسیر ساخته شود.
- `registry.sqlite` مرکزی: `projects(id, canonical_path, slot, lang_profile, created_at, last_indexed_at, schema_version)`.

**۲. Namespace سخت در هویت داده:**
- `NodeId` جدید: `n:<project_id>:<kind>:<path>[:<symbol>]` — یا نگه‌داشتن رشته فعلی ولی افزودن `project_id` به‌عنوان بخشی از کلید HashMap (`(ProjectId, NodeId)`).
- همه‌ی جداول مشتق (`name_to_nodes`, `file_to_nodes`, `token_to_nodes`, `path_index`, `export_index`, `impl_index`) کلید `project_id` بگیرند.

**۳. فیلتر کوئری اجباری:**
- امضای `search`, `trace`, `spreading_activation`, `resolve_seed` پارامتر `project_id` بگیرند و در همان ابتدا assert کنند هیچ گره خارج از scope وارد نتیجه نشود.
- یک `debug_assert!` سراسری: «هیچ گره‌ای با `project_id` متفاوت در `ContextView` نباشد».

**۴. Hot-Swap تمیز:**
- در `adopt_workspace_from_initialize` و هر مسیر سوییچ: **اول `graph.clear(Some(new_pid))`**، بعد `load_persisted`، بعد `reindex_incremental`.
- حذف زودخروج خطرناک `same_workspace_path` وقتی `project_id` فرق دارد.
- `reindex_incremental` نباید داخل حلقه `set_workspace` را عوض کند؛ workspace root یک‌بار در ابتدا قفل شود.

**۵. Multi-root / Monorepo:**
- تشخیص workspace: اگر چند marker زیر یک root بود، از کاربر/کانفیگ `sub_projects: [...]` بپذیر؛ هرکدام `ProjectId` جدا.
- حالت «monorepo واحد» هم به‌عنوان گزینه (`treat_as_single: true`).

**۶. تست نشتی (Leakage Benchmark) — بخشی از CI:**
- سناریو: پروژه A (وب Rust) و پروژه B (ML Python) را در یک پروسه MCP پشت‌سرهم `initialize` کن.
- کوئری‌های مشخص B بزن؛ assert کن **هیچ فایل/سمبل A** در پکت نیست (`cross_project_files == 0`).
- سناریو monorepo، سناریو «B هرگز index نشده»، سناریو «rootUri فرستاده نشد».

**۷. سرو هم‌زمان (اختیاری، ولی توصیه‌شده):**
- `RuntimeState { projects: LruCache<ProjectId, Arc<ProjectContext>> }` با ظرفیت پیش‌فرض ۳.
- هر ابزار MCP `project_id` را از context درخواست resolve کند (نه از state سراسری).

### ۴.۳ لایه P1 — Universal Artifact Graph

**تایپ‌های گره جدید (به `NodeType` اضافه شود):**

```
# موجود: Project, Directory, File, Component, Class, Function, Symbol, Import,
#         Dependency, Api, DbModel, Test, Config, Doc, Task, Decision, Memory, StyleToken
# جدید (عام):
Package, Module, Interface, Enum, Variable, Constant, TypeAlias

# جدید (داده/ML):
Dataset, DataSource, Transform, Feature, DataLoader,
Model, Layer, LossFn, Optimizer, Scheduler, Metric,
TrainLoop, EvalLoop, Experiment, Run, Checkpoint, Artifact,
Notebook, Cell, Pipeline, Stage, ConfigKey, Hyperparameter
```

**تایپ‌های یال جدید (به `EdgeType` اضافه شود):**

```
# موجود: Imports, Calls, References, Contains, DependsOn, ModifiedWith,
#         TestedBy, RelatedTo, UsedBy, PreviouslySuccessfulWith
# جدید:
Defines, Reads, Writes, Produces, Consumes,
Configures, Parameterizes, TrainedBy, EvaluatedBy,
Checkpoints, LogsMetric, DerivesFrom, RunsBefore (ترتیب سلول/مرحله)
```

**IR مشترک:** یک لایه‌ی «Artifact IR» که هم پروژه FastAPI و هم PyTorch از primitiveهای یکسان استفاده کنند. مثال زنجیره ML:

```
Dataset → Transform → DataLoader → Model → LossFn → Optimizer → TrainLoop → Checkpoint → EvalLoop → Metric
```

کوئری «چرا mAP افت کرد؟» دیگر keyword-search نیست: از `Metric(mAP)` → `EvalLoop` → `Model` → `Checkpoint` → `Transform` → `Dataset` عقب‌گرد می‌کند.

**گراف سه‌لایه:**
1. **Syntax Layer** — توابع/کلاس/ماژول (tree-sitter، از قبل هست).
2. **Resolution Layer** — رزولوشن نام دقیق (stack-graphs / SCIP ingest).
3. **Data/Config Layer** — YAML/Hydra/argparse → `ConfigKey`/`Hyperparameter` که با `Parameterizes` به آرگومان توابع وصل می‌شود.
4. **Notebook Layer** — `.ipynb` → `Cell` nodeهای ترتیبی با یال `RunsBefore` + DEF-USE بین سلول‌ها.

### ۴.۴ لایه P1 — پارسینگ عام

| ورودی | ابزار | خروجی |
|---|---|---|
| کد ۲۰+ زبان | tree-sitter + گرامرهای جدید (C, C++, R, Julia, Scala, Lua, Bash, TOML) | AST → Syntax Layer |
| رزولوشن نام | `tree-sitter-stack-graphs` (زبان‌های پشتیبانی‌شده) + ingest SCIP (`rust-analyzer --scip`, `scip-python`, `scip-typescript`, `scip-clang`) وقتی موجود | Resolution Layer دقیق |
| `.ipynb` | نرمال‌ساز مبتنی بر `nbformat` (الگوی jupytext): سلول کد → واحد قابل‌پارس؛ ترتیب اجرا حفظ | Notebook Layer |
| `config.yaml`, `params.yaml`, Hydra `conf/`, `argparse`/`click` | پارسر YAML/TOML + تطبیق کلید با نام آرگومان تابع | Data/Config Layer |
| فریم‌ورک‌های ML | Overlayهای جدید: PyTorch (`nn.Module`, `forward`, `DataLoader`), Lightning (`LightningModule`), HF (`Trainer`, `datasets.load_dataset`), sklearn (`Pipeline`), Keras (`Model.fit`) | یال‌های `TrainedBy`/`Produces`/… |

### ۴.۵ لایه P1 — Hybrid Retrieval + Reranker

```
seedها از پرامپت
  ├─ Symbol exact      : stack-graphs / SCIP / نام دقیق
  ├─ Lexical (BM25)    : tantivy  (به‌جای پیاده‌سازی دستی fa lexical)
  ├─ Semantic (ANN)    : fastembed + HNSW  (سطح فایل → سمبل تنبل، مثل الان)
  └─ Graph rank        : Personalized PageRank (پورت Aider) روی seedها
        │
        ▼
  ادغام + حذف تکراری (scoped به project_id)
        │
        ▼
  Reranker: cross-encoder کوچک (ONNX) یا امتیاز ترکیبی وزن‌دار
     وزن‌ها: تطبیق نام، هم‌مسیری، هم‌پکیج، «اخیراً ویرایش‌شده» (edge ranking)، feedback قبلی
        │
        ▼
  Coverage Proof: هر seed رزولوو شد؟ اگر نه → یک اکشن جستجو (نه لیست تکراری)
```

**Semantic Pruning (query-conditioned):** اگر پرسش درباره «Inference Pipeline» است، زیرگراف `TrainLoop`/`Experiment` از کاندیدها هرس شود؛ اگر درباره «Data Loader» است، `Metric`/`EvalLoop` هرس شود. این را به‌صورت query-intent classifier (که تا حدی در `retrieval/query_intent.rs` هست) گسترش بده.

### ۴.۶ لایه P1/P2 — Context Compiler

- **Skeletonization** (از قبل هست): امضای تابع/کلاس به‌جای بدنه؛ بدنه فقط با `expand_fold`.
- **Edge Ranking**: فایل‌های اخیراً ویرایش‌شده یا با وابستگی قوی‌تر، اولویت بالاتر برای ارسال.
- **LLMLingua-2 اختیاری** (`compression: none | light | aggressive`): بعد از اسکلت‌سازی، روی بخش‌های doc/comment/sidecar (نه امضاها و نه بدنه‌های express‌شده) اعمال شود تا نحو خراب نشود.
- **ترتیب پکت**: seedها اول و آخر، sidecarها وسط (طبق Lost-in-the-Middle).

### ۴.۷ لایه P1 — Memory / Feedback (per-project)

- همه‌ی `neuromesh.json` (اپیزودیک) و `ProjectFact`ها زیر slot پروژه — **از قبل هست**، فقط باید مطمئن شویم `record_feedback` هرگز وزن یک پروژه را روی پروژه دیگر اعمال نمی‌کند (تست).
- CHANGELOG نشان می‌دهد قبلاً با «نشتی یادگیری بین taskها» جنگیده‌اند (مثلاً `parse.ts` زاد لو می‌رفت)؛ همان دفاع را به سطح cross-project تعمیم بده.

### ۴.۸ لایه P0/P1 — Eval / Benchmark

- **Gold Dataset**: ۳ پروژه نمونه — یک وب (Next/FastAPI)، یک NLP (HF Transformers fine-tune)، یک CV (PyTorch detection). برای هرکدام ۲۰–۳۰ کوئری با «فایل/سمبل درست» برچسب‌خورده.
- **معیارها**: Retrieval Precision/Recall@k، Coverage، Token cost (before/after)، Latency، **Cross-Project Leakage (باید ۰ باشد)**.
- **هارنس**: روی SWE-bench Verified و CodeRAG-Bench (repo-level) اجرا شود تا عدد مستقل داشته باشیم.
- `neuromesh eval` فعلی را گسترش بده تا این‌ها را گزارش کند.

---

## ۵. نقشه راه فازبندی‌شده

### فاز P0 — «اول جلوی خطا را بگیر» (پایه)

| # | کار | crate | ریسک | تخمین |
|---|---|---|---|---|
| 1 | `ProjectId = blake3(git_root)`، فایل `.neuromesh/id`، `registry.sqlite` | core, index | کم | S |
| 2 | Namespace کردن کلیدهای HashMap با `(ProjectId, NodeId)` + جداول مشتق | graph | **متوسط** (تغییر گسترده) | L |
| 3 | فیلتر `project_id` اجباری در `search`/`trace`/`activation` + `debug_assert` | graph, context | کم | M |
| 4 | `graph.clear()` قبل از هر Hot-Swap؛ حذف زودخروج خطرناک؛ قفل workspace root | mcp, graph | کم | S |
| 5 | تشخیص monorepo / multi-root + `sub_projects` در کانفیگ | index | متوسط | M |
| 6 | Leakage Benchmark در CI (۴ سناریو) | context/tests | کم | M |
| 7 | (اختیاری) `LruCache<ProjectId, ProjectContext>` برای سرو هم‌زمان | mcp | متوسط | M |

**خروجی فاز:** ادعای «برای همه پروژه‌ها، هم‌زمان، بدون نشتی» قابل دفاع می‌شود.

### فاز P1 — «دانش را زیاد کن» (Universal)

| # | کار | تخمین |
|---|---|---|
| 8 | افزودن `NodeType`/`EdgeType`های عام + ML (Artifact IR) | M |
| 9 | گرامرهای tree-sitter جدید: C, C++, R, Julia, Scala, Lua, Bash, TOML | M |
| 10 | ادغام `tree-sitter-stack-graphs` برای رزولوشن دقیق (Python/JS/TS اول) | L |
| 11 | ingest فرمت SCIP (وقتی index موجود است) | M |
| 12 | پارسر `.ipynb` (nbformat + DEF-USE بین سلول‌ها) | M |
| 13 | لایه Config→Code (YAML/Hydra/argparse → `Hyperparameter`/`Parameterizes`) | M |
| 14 | Overlayهای ML: PyTorch, Lightning, HF, sklearn, Keras | L |
| 15 | BM25 با `tantivy` جایگزین retrieval لغوی دستی | M |
| 16 | Reranker (cross-encoder ONNX یا امتیاز ترکیبی) + query-conditioned pruning | M |
| 17 | Gold Dataset سه‌حوزه‌ای + گسترش `neuromesh eval` | M |

### فاز P2 — «فشرده‌سازی و دانش عمیق‌تر»

| # | کار | تخمین |
|---|---|---|
| 18 | LLMLingua-2 اختیاری بعد از fold | M |
| 19 | لایه دانش (community detection + خلاصه ماژول به‌سبک GraphRAG) | L |
| 20 | Federated multi-project retrieval (کوئری هم‌زمان روی چند پروژه مرتبط، با scope صریح) | L |
| 21 | بنچمارک مستقل روی SWE-bench Verified + انتشار عدد | M |

---

## ۶. تصمیم‌های باز (نیاز به نظر شما)

1. **Runtime**: سرو هم‌زمان چند پروژه (LRU) الان (P0) یا فعلاً فقط Hot-Swap تمیز و سرو هم‌زمان در P1؟
2. **ذخیره‌گاه per-project**: `graph.bin` تک‌فایلی فعلی را نگه داریم + `registry.sqlite` کنارش، یا کل ذخیره‌گاه را به SQLite/redb مهاجرت دهیم؟
3. **رزولوشن نام**: stack-graphs (سبک، Rust خالص، ولی هر زبان قواعد جدا) یا اتکا به SCIP indexerهای بیرونی (دقیق‌تر ولی نیاز به toolchain)؟ یا هر دو با fallback؟
4. **دامنه Universal اول**: کدام حوزه ML را اول هدف بگیریم — NLP (HF Transformers) یا CV (PyTorch detection)؟
5. **رابطه با upstream**: این‌ها را به‌صورت PR به `pinoox/neuromesh` بفرستیم یا فورک `ParsaVictor/neuromesh` را به‌عنوان یک محصول مستقل جلو ببریم؟
6. **هدف عدد**: روی «Maximum Task Success per Token» توافق داریم یا می‌خواهید «۹۰٪ کاهش» را هم به‌عنوان پیام بازاریابی نگه داریم؟

---

## ۷. پیوست — منابع

- NeuroMesh: <https://github.com/pinoox/neuromesh> · `crates/neuromesh-core/src/paths.rs` · `crates/neuromesh-mcp/src/server.rs` · `docs/architecture.md` · `docs/mcp.md`
- Pinoox / Yoosef Alipour: <https://github.com/pinoox/pinoox> · <https://github.com/yoosefap> · ZoomInfo (Co-Founder & CEO)
- Aider RepoMap: <https://aider.chat/2023/10/22/repomap.html> · <https://aider.chat/docs/repomap.html> · <https://anishgandhi.com/aider-pagerank-codebase-ranking/>
- Code Graph MCP: <https://github.com/colbymchenry/codegraph> · <https://github.com/sdsrss/code-graph-mcp> · <https://github.com/CodeGraphContext/CodeGraphContext>
- Serena: <https://github.com/oraios/serena>
- SCIP: <https://sourcegraph.com/blog/announcing-scip> · stack-graphs: <https://github.com/github/stack-graphs> · <https://docs.rs/tree-sitter-stack-graphs>
- Codebase-Memory (tree-sitter KG for LLM via MCP): <https://arxiv.org/html/2603.27277v1>
- Repomix: <https://github.com/yamadashy/repomix>
- LLMLingua: <https://github.com/microsoft/LLMLingua> · LongLLMLingua: <https://arxiv.org/pdf/2310.06839>
- GraphRAG: <https://neo4j.com/labs/genai-ecosystem/llm-graph-builder/> · Potpie · blarify (`blar-graph` روی PyPI)
- Lost in the Middle: <https://arxiv.org/pdf/2311.09198> · <https://arxiv.org/pdf/2510.10276>
- Context Rot (Chroma, 2025) · Retrieval Quality at Context Limit: <https://arxiv.org/pdf/2511.05850>
- When Retrieval Hurts Code Completion: <https://arxiv.org/pdf/2605.14478>
- CodeRAG-Bench: <https://arxiv.org/abs/2406.14497> · <https://github.com/code-rag-bench/code-rag-bench>
- Static analysis of Jupyter notebooks / dataflow: <https://arxiv.org/pdf/2605.01560> · <https://marimo.io/blog/dataflow>
- Kuzu (آرشیو اکتبر ۲۰۲۵): <https://lib.rs/crates/kuzu> · redb: <https://github.com/cberner/redb> · tantivy: <https://github.com/quickwit-oss/tantivy>
