# بریف کامل چت بعدی — code-context-engine (2026-09-21، main بعد از PR #116)

این سند تنها چیزی است که چت بعدی باید بخواند تا بدون خطا و سریع ادامه دهد. هر چیزی که این‌جا نیست، در فایل‌های
ارجاع‌شده هست. ترتیب خواندن: همین سند → `stage5-findings.fa.md` فقط برای finding ای که به آن می‌رسی.

> **2026-09-21 بعد از #118:** §۱۱ انجام شد (W1/W2). فازبندی جدید و بازبینی در **§۱۲**؛ از W3 (بازتعریف‌شده) شروع کن.

## ۰. پروژه در یک پاراگراف

fork از NeuroMesh v0.9.0 (Rust، MCP context engine محلی). هدف: **بیشترین task-success به ازای هر توکن روی هر
codebase ای، بدون نشت بین پروژه‌ها.** ریپو: `C:\1\1_پروژه\5_neuromesh\repo` → GitHub `ParsaVictor/code-context-engine`.
Parsa (فارسی‌زبان، تیم B2B با stack Fastify + Drizzle + Next.js) اختیار کامل داده: بدون پرسیدن، PR بزن، با CI سبز مرج کن،
گزارش کوتاه بعد از هر PR، گزارش کامل فقط وقتی بخواهد. **سریع** برایش مهم است؛ ولی «مخلوط = ship نکن» و holdout ها
خط قرمز اند.

## ۱. اعداد فعلی (recall / precision / forbidden)

| مجموعه | نقش | عدد | ratchet |
|---|---|---|---|
| dev (nanoGPT, express, vit-pytorch, fastapi-template) | تیون | 1.000 / 0.938 / 0 | — |
| large (django, ultralytics) | تیون | 1.000 / 0.666 / 0 | precision ≥0.64 |
| holdout-2 (gin, torchvision) | **تیون‌نشده** | 1.000 / 0.554 / 0 | گیت |
| holdout-c (libuv, fmt) | **تیون‌نشده** | 1.000 / 0.573 / 0 | گیت |
| holdout-lang (os-lib, cli, Flux.jl) | **تیون‌نشده** | 1.000 / 0.541 / 1 | گیت |
| holdout-ml2 (peft, keras-hub) | **تیون‌نشده** | 1.000 / 0.632 / 0 | گیت |
| holdout-ml (keras-io, setfit) | dev-class | 1.000 / 0.437 / 0 | — |
| holdout-cfg (hydra, detr) | dev-class | 1.000 / 0.712 / 0 | recall ≥0.95 / precision ≥0.69 |
| **holdout-web** (fastify/demo, shadcn taxonomy) | dev-class برای web | **0.817 / 0.618 / 0** | recall ≥0.90 / precision ≥0.60 (target-only، غیر ratchet) |
| self (۲۰ سؤال واقعی روی خود ریپو) | dev-class | 0.675 / 0.406 / 0 | — |

فاز C (task-success با مدل واقعی، DeepSeek-V4-Pro + داور GLM-5.3 از Baseten): هر ۹ سلول هر ۳ گیت پاس؛ packet 1.00/1.00
روی دو holdout. جدول در `docs/measured.md`.

## ۲. قوانین ثابت (تخطی = کار دوباره)

1. **holdout ها (holdout-2، -c، -lang، -ml2) هرگز تیون نمی‌شوند.** probe فقط روی dev/large/cfg/web/self. یک بار تیون روی
   holdout = dev-class و باید در `docs/measured.md` نوشته شود.
2. **ratchet فقط بالا** (margin −0.02 از عدد اندازه‌گیری‌شده).
3. **نتیجه‌ی مخلوط (یک مجموعه بالا، یکی پایین) = ship نکن.** یا سیگنال گم‌شده را پیدا کن (نه threshold) یا revert کن و در
   `stage5-findings.fa.md` ثبت کن. تیون امتیاز/threshold هیچ‌وقت جواب نداده — دو session اثبات.
4. **هر PR = ۹ مجموعه + self + `cargo test --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` سبز → CI
   سبز → مرج (بدون پرسیدن).** اندازه‌گیری فقط مجموعه‌های مؤثر در حین کار؛ همه قبل از PR.
5. اول **دلیل** بعد فیکس: `NM_EXPLAIN=1 NM_EXPLAIN_MAX_PRECISION=1.01` → `target/explain-<set>.txt` (قبلش `rm` کن؛ append می‌شود).
6. هر فایل عجیب در استفاده‌ی واقعی = finding. شماره‌ی بعدی: **F83**.
7. اعداد بدون اندازه‌گیری هرگز در README/measured نمی‌روند.

## ۳. دستورها

```bash
export CARGO_BUILD_JOBS=2                       # همیشه (ICE روی این ویندوز)
bash scripts/benchmark-holdout.sh               # ۹ مجموعه (dev large holdout holdout-c holdout-lang holdout-ml holdout-ml2 holdout-cfg holdout-web)
bash scripts/benchmark-holdout.sh holdout-web   # یکی
NM_EXPLAIN=1 NM_EXPLAIN_MAX_PRECISION=1.01 bash scripts/benchmark-holdout.sh holdout-web   # + دلیل هر فایل packet
# self set (۲۰ سؤال روی خود ریپو):
NM_THIRD_PARTY=1 NM_PRIVATE_SET_DIR="$PWD/tests/third_party/self" NM_PRIVATE_DIR="C:/1/1_پروژه/5_neuromesh" NM_INDEX_CACHE=0 \
  cargo test -q -p neuromesh-context --test third_party_private_gold -- --nocapture | grep "^third_party_private"
# per-task diff قبل/بعد:
grep "precision=" target/explain-X.txt | awk '{print $2,$3}' | sort -u > before.txt   # (git stash / pop برای قبل)
# probe یال‌های گراف: تست موقت crates/neuromesh-context/tests/zz_probe_tmp.rs با
#   #[path = "support/index_cache.rs"] mod index_cache; index_cache::graph_for_checkout(&root, "<set>", "<name>", &repo)
#   + graph.get_edges_map() / nodes_named / nodes_in_file — قبل از commit حذف شود.
# packet یک سؤال دلخواه روی خود ریپو (cwd = repo):
target/release/neuromesh.exe packet --json --query "…"   # (بعد از cargo build --release -p neuromesh-cli)
# باینری دسکتاپ (MCP ی Parsa) بعد از هر مرج مؤثر:
cargo build --release -p neuromesh-cli && taskkill //F //IM neuromesh.exe; cp target/release/neuromesh.exe "$LOCALAPPDATA/Programs/neuromesh/neuromesh.exe"
# فاز C (وقتی Baseten شارژ شد؛ VPN با exit اروپا/آمریکا — curl باید 401/200 بدهد نه 403):
OPENAI_BASE_URL=https://inference.baseten.co/v1 OPENAI_API_KEY=… PROVIDER=openai \
  PAIRS="deepseek-ai/DeepSeek-V4-Flash-0731:zai-org/GLM-5.3" bash scripts/phase-c-run.sh   # Flash کافی و ارزان؛ Pro اعتبار را می‌سوزاند
```

commit با `-F file` (heredoc + backtick متن را می‌خورد). PR body آخرش `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
CI با `gh pr checks N` (tab-separated؛ ستون ۲ = وضعیت). مرج: `gh pr merge N --squash --delete-branch`.

## ۴. تله‌ها (هر کدام یک بار وقت گرفته)

- **sed بازه‌ای (`sed -i 'N,$d'`) ماژول تست activator.rs (۲۴۰۴ خط) را برید و CI سبز ماند** (lib کامپایل می‌شد). بعد از هر
  sed بازه‌ای: `wc -l` و `git diff --stat`. ترجیح: Edit tool.
- فایل‌ها CRLF اند؛ `perl -0pi` با `\n` match نمی‌کند (`\r?\n`)؛ `python` روی ماشین نیست.
- هرگز حین cargo پس‌زمینه فایل Rust ویرایش نکن (build خراب می‌شود، دو بار).
- `cargo fmt` خط probe eprintln را reflow می‌کند؛ probe را با Edit بگذار/بردار.
- `target/explain-*.txt` append می‌شود؛ پیش‌فرض فقط تسک‌های <0.6.
- `grep` در pipe پس‌زمینه block-buffered است (`--line-buffered`).
- تغییر parser/graph/index کش ایندکس را باطل می‌کند → large ~۱۰ دقیقه؛ فقط context → ~۲ دقیقه.
- `stage4_security` (<30s) روی این ماشین ۵۰–۶۰s است حتی روی main؛ CI مرجع. `indexes_real_neuromesh_repo_with_usable_graph`
  هم‌زمان با cargo دیگر fail می‌کند، تنها پاس.
- تسک `handle_tool_call_intent` (فیکسچر روی خود ریپو) سقف token دارد (۲۸.۵k)؛ `activator.rs` خودش گلد است و رشد می‌کند.
- `max_resolved_seeds = 5`: seedهای فایل‌محور با هم می‌جنگند؛ قبل از insert رتبه‌بندی کن.
- CLI `index`/`packet` روی cwd کار می‌کند نه مسیر positional.
- Baseten: از exit ایران/آذربایجان 403 (حتی بی‌کلید)؛ کلید در هیچ فایلی نیست.
- `NEUROMESH_ENGINE=hybrid` بدون ایندکس embedding بی‌صدا بدتر می‌شود.

## ۵. معماری در ۱۰ خط (کجا چه چیزی را عوض می‌کنی)

- `crates/neuromesh-parser/src/` — یک فایل per زبان (`yaml.rs`, `shell.rs`, `config_reads.rs` = خواندن config در کد).
  خروجی: symbols + relationships (Imports/Calls/Parameterizes با `target_file_hint`).
- `crates/neuromesh-graph/src/graph.rs` — `ingest_file_keep` (ساخت نود + **literal_index** از رشته‌های داخل کوتیشن)،
  `finalize_links` (resolve یال‌ها؛ arm های Imports/Calls/Parameterizes)، `search_symbols`، `resolve_file_hint`. `intern.rs`:
  `GraphData`/`GraphSnapshot` (هر فیلد جدید باید در هر دو + `install_snapshot` + `save_to` بیاید)، `name_like_literals`.
- `crates/neuromesh-context/src/activator.rs` — `activate_inner`: pipeline seed (resolve → prune ها → twin_cohere → header →
  seed_set) → `select()` → physarum → materialize (fold/skeleton، گیت sidecar، cap). `resolve_seed_query_once` = resolver.
- `crates/neuromesh-context/src/activator_seed.rs` — `push_anchor_queries` (identifier → symbol → literal files)، compound
  stem / kebab / word-stem / path-word seeds، `push_compound_symbol_seeds` (فقط fallback).
- `crates/neuromesh-context/src/seed/` — `sink.rs` (هر push از این‌جا؛ guard های noise/acronym/ambiguous-file)،
  `twin_cohere.rs` (هم‌نام‌ها: dir words، body words، plural-tolerant)، `weak_file_seed.rs` (لیست STRONG/WEAK)،
  `ranker.rs` (cap 5)، `fallback.rs`.
- `crates/neuromesh-context/src/selector.rs` — `select()`: صندلی callee (Calls، Parameterizes readers، composer های
  config)، `is_noise_path_in`، `consumer_named_in_focus`.
- `crates/neuromesh-context/src/retrieval/alias.rs` — جدول alias چندزبانه (match از ابتدای کلمه).
- harness: `crates/neuromesh-context/tests/support/{gold_set,index_cache,explain}.rs`؛ گلدها در `tests/third_party/<set>/<repo>/gold_tasks.toml`.
- task-success: `crates/neuromesh-cli/src/commands/tasks.rs` (داور با سورس واقعی، `number_hunk_headers`).

## ۶. باز مانده — به ترتیب اجرا

1. **F81 + صندلی callee.** یال Calls از تابع هم‌نام (هر `route.ts` یک `POST`) به *فایل* می‌چسبد (`finalize_links`، arm Calls:
   `resolve_unique` قبل از `resolve_in_file`). فیکس درست (`resolve_in_file` اول) web recall +0.025 ولی large 0.641→0.558 +
   forbidden. پس اول `select()`: صندلی callee فقط وقتی prompt نام callee یا stem فایلش را دارد (الان `focus` = نام یا stem؛
   بدون focus هم تا ۳ صندلی می‌دهد اگر caller_count ≤5). بعد F81. هدف: web ≥0.70 recall بدون افت large.
2. **کلمه‌ی نام‌پوشه** (`dashboard` → `DashboardLoading` با prefix): web +0.125 recall، ولی تست `learning_to_emission_kosha_routes_emitted`
   (پوشه‌ی `school/`) را می‌شکند چون هیچ seed ای نمی‌ماند. سیگنال گم‌شده: وقتی رد کردی، مسیر path-word باید فعال شود
   (الان `anchored` آن را خاموش می‌کند). کد حذف‌شده در تاریخچه‌ی #112 (قبل از squash) — از `stage5-findings` بازنویسی کن.
3. **path-word با anchor**: web 0.742 ولی c/cfg −0.08؛ راه درست احتمالاً امتیاز کلمات مسیر *داخل* `select()`، نه seed.
4. **web باقی‌مانده:** `fd_login_flow` (سه فایل گلد، packet یکی)، `fd_session_plugin`، `tx_stripe_*` (route های stripe بی‌نام).
5. self: ۵ سؤال «مفهوم بی‌نام» — مرز روش لغوی؛ embedding جواب نداد (F79). نگه‌دار.
6. فاز C: ۲ سلول fixtures با F76/F77 (نیاز شارژ Baseten؛ ۵ دقیقه با Flash).
7. 5b: ۳۰ سؤال تیم روی ریپوی خصوصی (Parsa/تیم می‌نویسند؛ engine قبل از قفل گلد packet نمی‌بیند).
8. release v1.0 بعد از بستن web.

## ۷. ایده‌های سرعت و دقت (ارزیابی‌شده)

**سرعت اندازه‌گیری (گلوگاه واقعی، ~۲۰–۳۰ دقیقه/PR):**
- (الف) **CI به‌عنوان ماشین اندازه‌گیری:** workflow `holdout-benchmark.yml` هست؛ ۹ مجموعه را در matrix موازی روی لینوکس بزن و
  فقط جدول را بخوان. محلی فقط مجموعه‌ی مؤثر. تخمین: ۳ برابر سریع‌تر.
- (ب) WSL/لینوکس محلی: بدون ICE، بدون `CARGO_BUILD_JOBS=2`.
- (ج) کش ایندکس برای parser-change: کلید کش فعلاً هش کل چهار crate است؛ اگر فقط `neuromesh-context` عوض شد کش می‌ماند —
  خوب است؛ برای parser-change می‌شود کش per-language کرد (ارزشش کم؛ نکن مگر روزانه بخورد).

**دقت (recall روی ریپوی ندیده):**
- (۱) **صندلی callee با شواهد prompt** (بند ۶.۱) — بزرگ‌ترین اثر مستقیم روی web + امکان F81.
- (۲) **امتیاز کلمات مسیر در selection** (بند ۶.۳) — جایگزین امن path-word seeds.
- (۳) **گلد واقعی از تیم** (5b) — تنها منبع خوشه‌های جدید؛ هر ۱۰ سؤال واقعی تا حالا ۱–۲ باگ ساختاری داده.
- (۴) BM25 با tantivy روی بدنه‌ی فایل‌ها به‌عنوان *fallback فقط وقتی هیچ seed ای نیست* (نه جایگزین): برای ۵ سؤال مفهومی self.
  گیت: self recall بالا برود بدون افت ۹ مجموعه. اگر مثل embedding مخلوط شد، رها کن.
- (۵) ratchet خودکار: بعد از هر مرج، اگر عدد ≥ ratchet+0.03 بود، ratchet را در همان PR بالا ببر.

**چیزهایی که نکن:** تیون امتیاز؛ embedding به‌عنوان مسیر اصلی؛ قاعده‌ی عمومی «هر stem مشترک» (flux را می‌شکند)؛ حذف
مسیر fallback با یک seed «دقیق».

## ۸. الگو گرفتن از پروژه‌های مشابه؟ (سؤال Parsa)

بررسی‌شده در `01-deep-research-report.fa.md`. خلاصه‌ی صادقانه: Cursor/Continue/Aider/Sourcegraph Cody/Augment همه ترکیب
«embedding + BM25 + گراف symbol» دارند؛ هیچ‌کدام packet با گلد و holdout عمومی منتشر نمی‌کنند، پس «کپی» ممکن نیست —
فقط معماری. چیزی که آن‌ها دارند و ما نداریم: (۱) BM25 روی بدنه (ایده‌ی ۷.۴)، (۲) reranker (مدل کوچک cross-encoder روی
کاندیداها — گران در CPU، ولی روی ≤۳۰ کاندیدا شدنی)، (۳) name-resolution دقیق (stack-graphs/SCIP). چیزی که ما داریم و آن‌ها
نه: fold، گراف config→code/ML، و اندازه‌گیری صادقانه. نتیجه: **مسیر را عوض نکن؛ ۷.۴ و بعد reranker را به‌عنوان لایه‌ی fallback
اضافه کن، با همان گیت‌ها.** روش «کپی کامل» وجود ندارد چون هیچ‌کدام باز نیستند.

## ۹. فازبندی باقی‌مانده (زمان کار خالص)

| فاز | چه | زمان | گیت |
|---|---|---|---|
| W1 | بند ۶.۱ (صندلی callee + F81) | ۱ روز | web recall ≥0.70، ۹ مجموعه بدون افت |
| W2 | بند ۶.۲–۶.۴ | ۱ روز | web recall ≥0.80 |
| W3 | BM25 fallback (۷.۴) | ۱ روز | self recall ≥0.80 بدون افت |
| G3 | 5b با ۳۰ سؤال تیم | ۱ روز کار (تقویمی: هر وقت برسد) | private recall ≥0.95 / precision ≥0.75 |
| G5 | release v1.0 | ۱ روز | همه سبز، measured.md مرجع |

جمع ~۵ روز کار من. کندی واقعی = اندازه‌گیری (۷.الف/ب) و ورودی تیم (G3).

## ۱۰. شروع چت بعدی (کپی کن)

```
پروژه: fork NeuroMesh در C:\1\1_پروژه\5_neuromesh\repo (GitHub ParsaVictor/code-context-engine). اول
docs/planning/10-next-session-brief.fa.md را کامل بخوان (قوانین §۲، دستورها §۳، تله‌ها §۴، معماری §۵، کارهای باز §۶) و
حافظه‌ات (MEMORY.md → neuromesh-session13). main بعد از PR #116. اختیار کامل: بدون پرسیدن PR بزن، با CI سبز مرج کن،
بعد از هر PR گزارش ۳–۵ خطی (جدول قبل/بعد + قدم بعد). از §۶ به ترتیب شروع کن: W1 = صندلی callee با شواهد prompt، بعد F81.
هر PR = ۹ مجموعه + self + tests + clippy سبز. holdout ها را تیون نکن؛ مخلوط = revert + ثبت در stage5-findings (F83 به بعد).
بعد از هر sed بازه‌ای `git diff --stat`. سریع پیش برو؛ وقتی گزارش کامل خواستم، مرحله را تمام کن، handoff را به‌روز کن و
بایست.
```

## ۱۱. افزوده‌ی 2026-09-21 (بعد از #116) — محدودیت اصلی و راه حلش

**محدودیت اصلی = recall روی ریپوهای web/route-shaped (Fastify + Next.js app router): 0.642.** روی همه‌ی ۱۱۲ تسک دیگر
recall ما 0.938 در برابر 0.735 نسخه‌ی اصلی است؛ web تنها جایی است که هر دو ضعیف‌اند. علت‌های اندازه‌گیری‌شده (همه در
`stage5-findings.fa.md` F80–F82):

1. **F81** — یال Calls از تابع هم‌نام (`POST` در هر `route.ts`) به *فایل* می‌چسبد (`finalize_links` → arm Calls →
   `resolve_unique` قبل از `resolve_in_file`). فیکسِ `resolve_in_file` اول: web recall +0.025 ولی large 0.641→0.558 +
   forbidden، چون `select()` تا ۳ صندلی به callee می‌دهد حتی بدون شواهد prompt (`focus` فقط نام/stem؛ بدون focus هم اگر
   caller_count ≤5). **ترتیب درست: اول صندلی callee فقط با شواهد prompt (نام callee یا stem/پوشه‌ی فایلش در prompt)، بعد F81.**
2. **کلمه‌ی نام‌پوشه** (`dashboard` → `DashboardLoading` با prefix، به‌جای `dashboard/layout.tsx`): +0.125 web ولی تست
   `learning_to_emission_kosha_routes_emitted` شکست چون بعد از رد کردن هیچ seed ای نماند و path-words (چون `anchored`)
   خاموش بود. سیگنال: وقتی prefix-hit به‌خاطر نام‌پوشه رد شد، path-word seeds باید اجازه‌ی اجرا داشته باشند.
3. **path-word seeds با anchor**: web 0.742 ولی c/cfg −0.08 (`loop-watcher.c` کنار `loop.c`). راه درست: امتیاز کلمات مسیر
   *داخل* `select()`/fill (بالا بردن رتبه‌ی فایلی که ≥۲ کلمه‌ی prompt در مسیرش دارد)، نه seed جدید.
4. web باقی‌مانده: `fd_login_flow` (۳ فایل گلد، packet ۱ — repository/password-manager ی callee نمی‌آیند ⇒ همان ۱)،
   `fd_session_plugin`، `tx_stripe_*` (route های stripe بی‌نام ⇒ همان ۳).

**هدف W1–W2:** holdout-web recall ≥0.80 با precision ≥0.60، بدون افت هیچ‌کدام از ۹ مجموعه و self. اگر یک قاعده web را
بالا برد و یکی دیگر را انداخت: سیگنال گم‌شده را از dump پیدا کن (`NM_EXPLAIN`)؛ اگر دو تلاش جواب نداد، revert و ثبت.

ابزار مقایسه با نسخه‌ی اصلی: `scripts/compare-baseline.sh` (نتایج `docs/baseline-vs-fork-2026-09-21.txt`؛ باینری baseline
از worktree روی tag `baseline-v0.9.0` با `CARGO_TARGET_DIR` جدا، ~۲۵ دقیقه build). A/B ی Claude Code: `claude -p "<task>"
--output-format json --model sonnet --mcp-config <json> --strict-mcp-config --allowedTools …` (مسیر exe در JSON با `/`).

## ۱۲. افزوده‌ی 2026-09-21 (بعد از #118) — بازبینی و فازبندی دوباره

**وضعیت:** W1 (#117، F83) و W2 (#118، F84) مرج شدند. web 0.642→**0.817 / 0.618**، holdout-2 0.554→**0.700**، large 0.675
(ratchet 0.655)، cfg 0.767 (ratchet 0.745)، holdout-c 0.589، lang 0.589، ml2 0.632، self 0.675/0.428. سه نسخه‌ی «صندلی
بی‌شاهد» اندازه‌گیری و رد شد (F83) — درس: هیچ فایلی بدون کلمه‌ای از prompt نباید صندلی بگیرد؛ هر بار large/holdout-2 −0.1.

**بازبینی self (۷ تسک گم‌شده):** همه «مفهوم بی‌نام» اند و ۴ تای‌شان seed *غلط* دارند (`token:pheromone → PheromoneConfig`،
`concept:database → DatabaseError`)، یعنی «BM25 فقط وقتی هیچ seed ای نیست» (W3 قدیمی) ۴ تا از ۷ را نمی‌گیرد. جواب همه در بدنه/کامنت
فایل گلد است (`argparse`، `add_argument`، `ratchet`، `judge`، `exon budget`، `pheromone`) — grep ساده پیدایشان می‌کند.
**W3 بازتعریف:** BM25 (tantivy، روی بدنه‌ی فایل‌های غیر‌noise) به‌عنوان *منبع کاندید* که فقط با seedهای حدسی (tier ی
token/concept/fallback) رقابت می‌کند، هرگز با seed قوی (identifier/file/config). وقتی بهترین hit ی BM25 ≥۲ کلمه‌ی prompt را در بدنه
دارد و seed حدسی فقط یک کلمه را با prefix گرفته، BM25 جایگزین می‌شود. گیت: self recall ≥0.85، ۹ مجموعه بدون افت.

**بازبینی web (۴ تسک باقی):** `env.ts` («env variable» سه‌حرفی؛ کلید `RATE_LIMIT_MAX` در prompt نیست)، `lib/subscription.ts` و
`password-manager` ی login (callee بی‌نام) — همان مرز «بی‌شاهد». با BM25 ی بازتعریف‌شده ممکن است ۱–۲ تا بیاید؛ تیون جدا نمی‌کنیم.

**بازبینی precision:** روی گلدهای تک‌فایلی، هر فایل اضافه precision را 0.5 می‌کند؛ 0.6–0.7 یعنی به‌طور میانگین یک فایل اضافه.
فاز C نشان داد task-success با همین packet ها ۱.۰ است. بنابراین precision-tuning تمام است (session 10 هم همین را گفت)؛ فقط
ratchet نگه می‌داریم.

| فاز | چه | انتظار | گیت |
|---|---|---|---|
| **W3** | BM25 رقیب seed حدسی (tantivy) | self 0.675→≥0.85؛ web شاید +0.02 | ۹ مجموعه بدون افت |
| **G3** | ۳۰ سؤال تیم روی ریپوی خصوصی (هر وقت رسید) — یک دور probe، fix خوشه‌ها | private recall ≥0.90 | holdout واقعی؛ گلد قفل قبل از اجرا |
| **G4** | dogfood: ۱۰ سؤال واقعی جدید روی خود ریپو + ۱۰ روی web | ۱–۲ finding ساختاری | هر ۱۰ سؤال = یک probe |
| **G5** | release v1.0: measured.md مرجع، باینری، PR های upstream | — | همه سبز |

زمان خالص: W3 یک روز، G4 نصف روز، G3 وابسته به تیم، G5 یک روز.

## ۱۳. افزوده‌ی 2026-09-22 (بعد از #120) — وضعیت نهایی session 14، روش کار جدید، درس‌ها

**main = بعد از PR #120.** چهار PR امروز: #117 (W1/F83)، #118 (W2/F84)، #119 (W3/F85)، #120 (G4/F86). باینری دسکتاپ از همین main.

| مجموعه | عدد فعلی (recall / precision / forbidden) | ratchet |
|---|---|---|
| dev | 1.000 / 0.938 / 0 | — |
| large | 1.000 / 0.675 / 0 | precision ≥0.655 |
| holdout-2 | 1.000 / 0.700 / 0 | گیت (تیون نشود) |
| holdout-c | 1.000 / 0.589 / 0 | گیت |
| holdout-lang | 1.000 / 0.589 / 1 | گیت |
| holdout-ml2 | 1.000 / 0.632 / 0 | گیت |
| holdout-ml | 1.000 / 0.478 / 0 | — |
| holdout-cfg | 1.000 / 0.767 / 0 | recall ≥0.95 / precision ≥0.745 |
| holdout-web (**۳۰** سؤال) | 0.800 / 0.608 / 0 | target-only ≥0.90 / ≥0.60 |
| self (**۳۰** سؤال) | 0.833 / 0.578 / 0 | — |

**روش کار جدید (۳ برابر سریع‌تر، از W3 به بعد — همین را ادامه بده):**
1. همه‌ی تغییرات یک مرحله در **یک branch**؛ بعد از هر اصلاح فقط مجموعه‌ی مؤثر را بسنج (self ~۲ دقیقه، web ~۲ دقیقه با
   کش ایندکس؛ تغییر parser/graph کش را باطل می‌کند: web ~۴ دقیقه، large ~۱۰).
2. **یک گیت کامل در آخر** (۹ مجموعه + self + `cargo test --workspace` + clippy) در background با `--line-buffered`.
3. اگر گیت مخلوط شد: **bisect** با کامنت کردن یک قاعده (`// BISECT`) و سنجش فقط مجموعه‌ی افتاده (large/holdout-2 معمولاً
   ۳ دقیقه با کش) — نه حدس. در G4 یک bisect قاعده‌ی مقصر را در ۵ دقیقه پیدا کرد.
4. probe موقت: `if std::env::var("NM_PROBE").is_ok() { eprintln!("PROBE …") }` با sed یک‌خطی، بعد `sed -i '/PROBE/d'`.
   برای گراف: `tests/zz_probe_tmp.rs` با `support/index_cache::graph_for_checkout` + `get_edges_map` (قبل از commit حذف).
5. جدول per-task از dump: `awk` روی `target/explain-<set>.txt` (خط `===`، `gold:`، `✓`) → diff قبل/بعد.

**درس‌های session 14 (دیگر تکرار نشود):**
- **هیچ فایلی بدون کلمه‌ای از prompt صندلی نمی‌گیرد.** سه نسخه‌ی «صندلی بی‌شاهد» (≤۵ caller؛ import شده؛ seed کوچک) هر بار
  large −0.1 + forbidden و holdout-2 −0.03 دادند (F83). گلدهایی که چنین callee ای می‌خواستند از همین قاعده‌ی حذف‌شده
  تغذیه می‌شدند و با دلیل اصلاح شدند (`handle_tool_call_intent`، `orders_stock_of_unknown`).
- **کلمه‌ی عادی که اتفاقاً stem یک فایل است، آدرس نیست** («session» → `session.ts`): حتی به‌عنوان حدس large −0.05 (F86).
- **دو کلمه‌ی مسیر بدون شرط کم‌تکرار = «جا» نه فایل** (`management/commands/*` ← «management command»، large −0.033، F84).
- **BM25 با امتیاز خالص** فایل ریز یا غول‌پیکر را برنده می‌کند؛ اول پوشش، بعد چگالی؛ نثر (README/docs) هرگز؛ بین
  هم‌پوشش‌ها اول فایلی که *مسیرش* prompt را می‌گوید (F85/F86).
- **عدد یک مجموعه‌ی کوچک که رویش فیکس شده، عدد پروژه نیست**: self ۲۰ سؤالی 0.925 بود، ۱۰ سؤال کور جدید 0.55 داد. فقط
  holdout ها و سؤال‌های تیم (G3) عدد واقعی‌اند.
- کامنت/دوک‌کامنتی که prompt یک گلد را نقل کند، خودش گلد را می‌گیرد (self-reference؛ F25/F83) — مثال‌های دیگر بنویس.
- heredoc ی bash `'\'` را می‌خورد → `replace('\', "/")` خطای کامپایل؛ خطوط با backslash را با Edit tool بنویس.
- `git stash` روی branch ی که همه‌چیز commit شده کاری نمی‌کند؛ برای مقایسه با main: `git checkout <sha>` + فایل probe untracked.
- perl -0pi با چند جایگزینی: اگر یکی `die` کند هیچ‌کدام اعمال نمی‌شود؛ بعدش grep کن.
- `stage4_security` محلی ۳۵–۹۶ ثانیه است (گیت <30s) — CI مرجع؛ نادیده بگیر.

**باز مانده (به ترتیب):**
1. **G3** — ۳۰ سؤال تیم روی ریپوی خصوصی (`NM_PRIVATE_SET_DIR`/`NM_PRIVATE_DIR`؛ گلد قفل قبل از اجرا؛ engine قبلش packet
   نمی‌بیند). یک دور probe، fix خوشه‌ها با روش batch. هدف: private recall ≥0.90.
2. خوشه‌های ثبت‌شده در F86 برای دور بعد: تماس بدون receiver به شیء decorate شده (`knex('users')` → literal `decorate('knex')`)؛
   `identifier:MDX` (acronym) anchor ی غلط؛ literal «upload» فایل route را anchor می‌کند؛ `self_path_address`/`self_strip_plural`
   (فایل درست ۱–۲ کلمه کمتر از haystack می‌گوید).
3. **G5** — release v1.0: measured.md مرجع، باینری دسکتاپ، PR های upstream (پس از G3 یا با اعداد فعلی اگر Parsa بخواهد).
4. فاز C دوباره با Flash وقتی Baseten شارژ شد (۲ سلول fixtures).
