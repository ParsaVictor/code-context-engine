# Handoff — session 16 (۲۰۲۶-۰۹-۳۰ تا ۲۰۲۶-۱۰-۰۱) — نسخه‌ی نهایی

ورودی: گزارش یوسف (۳ از ۷ روی ریپوی خودمان، ۲۶ ثانیه، باگ stdout). نقشه: `11-roadmap-2026-09-30-concept-queries.fa.md`. لاگ پژوهشی (برای مقاله): `docs/research/contributions-log.md`.

## merge شده

| PR | محتوا |
|---|---|
| #126 | بنر stdout → stderr؛ تست باینری واقعی |
| #127 = v1.1.0 | رتبه‌بند BM25F کل‌سؤال، comment_index، thesaurus، F89، اصلاح cold start، NM_TIMING |
| #128 | سؤال ساده فقط به رتبه‌بند اعتماد می‌کند؛ panic در skeleton |
| #129 | جدول BM25/Aider، holdout click، folds تکراری، حدس‌ها «missing» نیستند |
| #130 | مدل اختیاری **jina-code v2** + fusion (فقط برای سؤال ساده؛ MiniLM هرگز)؛ `install embed jina-code`؛ `selected_paths`؛ harness SWE-bench |

## باز — PR #131 (`phase-n/issue-mode`)

۱. **حالت issue**: پرامپت ≥۶۰ کلمه رتبه‌ی BM25F را هم seed می‌کند و ترتیب packet با RRF (activation + BM25F×2) — `gold::packet_file_order`. SWE-bench Lite **dev split** (flask/requests/seaborn/xarray/pylint، ۲۴ issue): hit@1 0.125→0.333، @3 0.250→0.583، @5 0.417→0.667 (BM25: 0.250/0.625/0.750).
۲. **باگ escalation (مهم)**: L3 با موتور fast وقتی مدل embedding روی دیسک بود، کل sidecar را **داخل سؤال** می‌ساخت (django >۱۰ دقیقه). حالا یک‌بار در پس‌زمینه. همان issue: >600s → 171s.
۳. **cap روی file_rank** (۳۲KB، ۲۵۶ واژه) — gate امنیتی stage4 (57s > 30s) را درست کرد؛ محلی پاس شد (7.5s).
۴. `--shard/--only` در harness. CI آخرین commit (4ce7a57) هنوز دیده نشده — **اول این را چک کن.**

## در حال اجرا (ممکن است تمام شده باشد)

SWE-bench Lite **test split** (astropy, sphinx, pytest, sklearn, matplotlib, sympy, django — ۲۷۶ issue) با باینری اصلاح‌شده، ۴ shard:
`C:\1\1_پروژه\5_neuromesh\swebench\results-new-{0..3}.jsonl` (resumable؛ همان دستور را دوباره اجرا کن تا فقط باقی‌مانده‌ها اجرا شوند). سرعت ~۱ issue/دقیقه کل (ایندکس سرد django در هر issue ۱–۳ دقیقه). نتایج قبلی آلوده به MiniLM بودند → `old-runs/`، استفاده نکن. dev split هم باید با `nm-fix` دوباره اجرا شود (اعداد بالا با باینری قبل از اصلاح escalation گرفته شده).

## اعداد فعلی (صادقانه)

| ست | v1.0.0 | الان |
|---|---|---|
| concept (dev، ریپوی خودمان) | 0.286 | 0.679/0.404؛ با jina 0.786/0.429 |
| ripgrep holdout | 0.042 | 0.500/0.156؛ با jina **0.667/0.347** (BM25@3 0.500/0.222) |
| click holdout | — | 0.958/0.342؛ با jina 0.958/0.439 |
| ۹ ست قدیمی | — | بدون افت |
| SWE-bench Lite dev | 0.125@1 | 0.333@1، 0.583@3 |

## رد شده با عدد (تکرار نکن)

seed تعریف‌های فایل بزرگ؛ fusion با MiniLM؛ gate روی fill؛ reranker jina-v1-turbo (ripgrep 0.667→0.375)؛ Aider RepoMap به‌عنوان retriever.

## گام‌های بعدی به ترتیب

1. CI PR #131 → merge. نتایج test split را جمع کن (اسکریپت خلاصه در handoff بالا) و با BM25 مقایسه کن؛ در `contributions-log.md` §8 ثبت کن.
2. **سرعت ایندکس**: `manifest+links` روی django ۶۰–۹۷ ثانیه (روی ریپوی خودمان ۲.۴s) — رشد فوق‌خطی در `finalize_links`؛ با `NM_TIMING=1` پروفایل کن. این همان تأخیر cold start یوسف است.
3. **BM25F با tf واقعی** در فیلد body (الان باینری) — BM25 ساده در @3/@5 به همین دلیل جلوتر است.
4. reranker v2 (کد-آموزش‌دیده، `%LOCALAPPDATA%\neuromesh\models\jina-reranker-v2`، اگر دانلود کامل شده) روی concept-holdout.
5. انتشار 1.2.0 (jina-code، issue mode، اصلاح escalation).
6. مسیر مقاله: §5 در `contributions-log.md` (SWE-bench کامل + Agentless/LocAgent، task success per token، ablation، ≥۵۰ سؤال در هر holdout با دو نفر).

## تله‌ها

- یک build در هر لحظه؛ دیسک یک بار پر شد (الان ~۶ GB آزاد). `sed` با `\\` و `a\`/`i\` در MSYS خراب می‌کند — Edit یا head/tail.
- `benchmark-fast.sh` ۹ ست در ۱.۵–۵ دقیقه. private harness مسیر مطلق می‌خواهد.
- MCP دسکتاپ پارسا روی v1.1.0 (backup v1.0.0 کنارش).
- مدل‌ها در `%LOCALAPPDATA%\neuromesh\models\` (jina-code-v2، jina-reranker-v1-turbo)؛ MiniLM در `repo/crates/neuromesh-embed/models` (gitignored) — وجود آن روی این ماشین رفتار L3 را تغییر می‌دهد.
