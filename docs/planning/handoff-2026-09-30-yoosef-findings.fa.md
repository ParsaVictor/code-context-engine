# Handoff — یافته‌های گزارش یوسف روی باینری v1.0.0 (self-repo)، ۲۰۲۶-۰۹-۳۰

منبع: یوسف (نویسنده اصلی NeuroMesh) گزارشی فرستاد که باینری v1.0.0 ما (Windows زیر Wine + Linux native) را روی همین ریپازیتوری خودمان (`neuromesh`/`code-context-engine`) با همون ۷-query battery قدیمی تست کرده. نتیجه گزارش‌شده: ۳/۷ (۴۳٪)، در مقابل upstream v0.9.11 که ۷/۷ گزارش شده.

این session (Claude، با دسترسی زنده به MCP خود پروژه روی همین ریپو) سه‌تا از ۴ ادعای fail را **مستقل بازتولید و تایید کرد** با `get_context_packet` واقعی. یکی از ۴ ادعا **رد شد** (ground truth اشتباه بود). یک مورد (latency) تایید نشد چون زمان نکردیم.

## تایید شده — باگ‌های واقعی (۳ مورد)

### 1. `root_fs_safety` — پاسخ کاملاً اشتباه
Query: "How does this tool prevent indexing dangerous paths like the filesystem root?"
- پاسخ گرفته‌شده: `crates/neuromesh-mcp/src/descriptors.rs` (تعریف ابزارهای MCP — کاملاً بی‌ربط)
- پاسخ درست: `crates/neuromesh-index/src/confine.rs` — دارای `is_safe_workspace`, `assert_safe_workspace`, `path_escapes_workspace`
- `coverage: no_recorded_gap` یعنی سیستم حتی متوجه نشد که جواب اشتباه داده (اعتماد کاذب)
- packet_id: `ctx_58280a8891c44c0687c04d654633d642`

### 2. `reinforcement` — پاسخ اشتباه
Query: "How does a file's importance get reinforced after repeated edits?"
- پاسخ گرفته‌شده: `crates/neuromesh-graph-proxy/src/cbm.rs` (کلاینت CBM proxy — بی‌ربط)
- پاسخ درست: احتمالاً `crates/neuromesh-graph/src/edge.rs` یا `crates/neuromesh-context/src/activator.rs` (`PheromoneEngine`/`reinforce_path` — طبق grep قبلی این‌جا هستن)
- `coverage: partial`, `sufficiency_score: 0.494`
- packet_id: `ctx_fd9a76b1574641a9acec12449bd76dc5`

### 3. `max_files_cap` — پاسخ اشتباه
Query: "How does the system determine the maximum number of files to index automatically?"
- پاسخ گرفته‌شده: `crates/neuromesh-cli/src/commands/tasks.rs` (task harness — بی‌ربط)
- پاسخ درست: باید `FileCapArg` / `max_files_from_args` باشه (این نمادها طبق grep در `crates/neuromesh-cli/src/main.rs` و `commands/*.rs` پخش شدن — باید دقیق پیدا بشه)
- `coverage: partial`, `sufficiency_score: 0.477`
- packet_id: `ctx_866ce80ff3aa455ea18f5957827e9220`

**نتیجه مشترک:** هر سه مورد seed resolution/retrieval روی این ریپوی خاص (Rust، چندماژوله، اسم‌های عمومی مثل `confine`, `reinforce`, `cap`) گم می‌شه و به‌جای فایل درست یه فایل نسبتاً بی‌ربط با امتیاز utility بالا برمی‌گرده. این دقیقاً همون کلاس مشکلی هست که در benchmarkهای holdout قبلی (stage 4/5) دیده و تا حدی حل شده بود، اما ظاهراً روی این ریپوی خاص (self) هنوز ضعیفه — که با عدد قبلی‌مون هم جور در میاد: session 15 گزارش کرده بود self(30) precision 0.867 ولی recall فقط 0.583 — یعنی حتی قبلاً هم می‌دونستیم این holdout ضعیف‌تره، فقط این ۳ query مشخص رو تست نکرده بودیم.

## رد شده — ادعای نادرست (۱ مورد)

### `retry_negative` — این یک باگ نیست
Query: "Is there retry logic or exponential backoff when calling the AI provider API?"
- گزارش یوسف انتظار داشت جواب `no_confident_match` باشه (چون در ریپوی upstream چنین منطقی وجود نداره)
- اما **این ریپو (فورک ما) واقعاً retry/backoff داره**: `crates/neuromesh-provider/src/anthropic.rs` یک حلقه retry کامل با `ANTHROPIC_MAX_RETRIES`, backoff نمایی (`15u64 << attempt`), و تشخیص خطای transient (429/5xx/"Budget pool"/"overloaded") داره. `openai.rs` هم مشابه.
- سیستم ما این فایل‌ها رو درست پیدا کرد و جواب داد (`coverage: partial` ولی با محتوای درست: انتخاب فایل‌های provider که واقعاً retry دارن)
- **نتیجه:** ground truth گزارش یوسف برای این query از تست روی ریپوی upstream به‌ارث رسیده و برای ریپوی ما غلطه. این مورد را از لیست باگ‌ها حذف کنید؛ اگر جای دیگری هم این ground truth استفاده می‌شه، اصلاحش کنید.

## تایید نشده — نیاز به بررسی بیشتر

### Latency outlier (۲۶ ثانیه روی `root_fs_safety`)
گزارش یوسف ادعا می‌کنه این query حدود ۲۶ ثانیه طول کشیده (هم روی build ویندوزی زیر Wine، هم روی Linux native). این session زمان دقیق call را اندازه نگرفت (فقط از طریق MCP زنده صدا زدیم، بدون timing). **قدم بعدی:** با `time` دقیق روی یک باینری تازه‌ساز، همین query رو چند بار اجرا کنید و ببینید تکرار می‌شه یا نه؛ اگر واقعی بود، پروفایل بگیرید (شاید regex یا walk روی مسیرهای root سیستم واقعاً کند باشه — `confine.rs` با `canonicalize` و چک‌های مسیر سر و کار داره).

## تایید شده — باگ پروتکلی جدی (خارج از accuracy)

### stdout pollution در حالت `mcp`
کد بررسی شد و **مستقیماً تایید شد**:
- `crates/neuromesh-api/src/server.rs:75-81` — بنر `🌿 NEUROMESH ... UI MONITOR & MCP DASHBOARD ACTIVE` با `println!` چاپ می‌شه (یعنی روی **stdout**)
- `crates/neuromesh-cli/src/main.rs:294-306` — در حالت `neuromesh mcp`، این `HttpServer` به‌صورت یک تسک tokio جدا (`tokio::spawn`) در **همون پروسه و همون stdout** که `McpServer::run_stdio()` (خط 318) روی آن پروتکل JSON-RPC خط‌به‌خط می‌فرسته، استارت می‌شه
- یعنی اگه دو تسک به هر دلیلی (تایمینگ race، یا هر بار) با هم روی stdout بنویسن، کلاینت‌های سخت‌گیر MCP (که خط اول رو مستقیم JSON.parse می‌کنن) می‌شکنن
- **فیکس پیشنهادی:** بنر را از `println!` به `eprintln!` (stderr) تغییر بده، یا کلاً در حالت `mcp` چاپش نکن (فقط در `neuromesh monitor` standalone چاپ بشه چون اونجا stdout مصرف‌کننده JSON-RPC نداره)

## اولویت پیشنهادی برای فاز بعدی

1. **فوری، کم‌ریسک:** فیکس stdout pollution (`println!` → `eprintln!` در `server.rs`) — یک‌خطی، بدون ریسک رگرسیون
2. **میان‌مدت:** ریشه‌یابی چرا این ۳ symbol (`confine`/`is_safe_workspace`, `reinforce_path`/`PheromoneEngine`, `FileCapArg`) روی این ریپوی خاص seed نمی‌شن — احتمالاً یه مسئله seed resolution برای نام‌های generic/چندجا-استفاده‌شده در یک ریپوی بزرگ Rust چندماژوله است؛ این می‌تونه با روش‌شناسی holdout قبلی (stage 4/5) بررسی بشه، یعنی این ۳ query رو به یه گلد-ست کوچیک self-repo اضافه کنید و دلیل دقیق seed miss رو پیدا کنید (شبیه F-numberهای قبلی)
3. **کم‌اولویت، نیاز به تایید:** latency 26s — قبل از هر کاری، reproduce کنید

## منابع

- گزارش اول یوسف (روی نسخه خیلی قدیمی fork با نام `code-context-engine`): دقت ۴۳٪، نشون داد نسخه قدیمی از upstream عقب بود
- گزارش دوم یوسف (روی باینری v1.0.0 فعلی ما، همین گزارش): دقت ۴۳٪ گزارش شده، همون query battery
- این handoff: نتیجه verify مستقیم با `mcp__neuromesh__get_context_packet` زنده روی همین ریپو، ۲۰۲۶-۰۹-۳۰
