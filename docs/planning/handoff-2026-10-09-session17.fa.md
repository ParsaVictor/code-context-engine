# Handoff — session 17 (۲۰۲۶-۱۰-۰۸ تا ۰۹)

نقشه: `12-roadmap-2026-10-08-session17.fa.md`. لاگ پژوهشی: `docs/research/contributions-log.md` §8.1–8.4.

## merge شده

| PR | محتوا | عدد |
|---|---|---|
| #131 | issue mode، اصلاح escalation، cap روی file_rank | — |
| #132 | سرعت ایندکس | django سرد 3m20s → 26s (links 147s → 10s، scan 16s → 3s)، گراف یکسان |
| #133 | دیسک | target/ به 55GB رسیده بود (debug 43GB، کش ایندکس 6.7GB)؛ مرزدار شد |
| #134 | issue: حذف قالب، توکن‌های `L031`، `chunk_rank` (BM25 سطح definition، عنوان ×3)، لیست `localization` | dev-fast Acc@5 0.456 → 0.596 |

## branch باز: `phase-o/release-1.2.0`

نسخه 1.2.0، `where_to_look` در پاسخ MCP برای گزارش‌های بلند (smoke test شد)، CHANGELOG،
harness سریع‌تر (worktree reuse + sharding با hash پایدار). بعد از عدد holdout: measured.md، PR، tag
`v1.2.0` (workflow release سه باینری را می‌سازد)، به‌روزرسانی MCP دسکتاپ.

## روش (مقاله)

- dev = SWE-bench dev split (۲۲۵، ۶ ریپو؛ `swebench/devset/`). dev-fast = ۱۰ از هر ریپو (۵۹).
- holdout = Lite test (۳۰۰) — یک بار با باینری main `d6854b6` → `swebench/results-final-test.jsonl`.
- ارزیابی: `scripts/research/eval_loc.py` (Acc@k، `--ci` بوت‌استرپ، `--fuse`).

## اعداد dev (اندازه‌گیری‌شده)

| | @1 | @3 | @5 | @10 |
|---|---|---|---|---|
| BM25 ساده (۲۲۵) | 0.160 | 0.347 | 0.427 | 0.538 |
| ما، localization (۲۲۵) | **0.249** | **0.484** | **0.600** | **0.680** |
| ما، ۱۶۶ issue تنظیم‌نشده | 0.247 | 0.482 | 0.608 | 0.675 |
| تک‌فایلی (۱۲۴) BM25 → ما | 0.290 → 0.452 | 0.532 → 0.710 | 0.613 → **0.863** | 0.774 → 0.911 |

## رد شده با عدد (تکرار نکن)

tf بدنه در BM25F؛ fusion برای seedها (بی‌اثر)؛ کنار گذاشتن فایل‌های داده (بی‌اثر)؛ رأی همسایه‌های گراف
(@1 0.254 → 0.136)؛ reranker v2 روی issue (@3 0.492 → 0.373، ۸ ثانیه)؛ embedding chunkها روی CPU
(۲.۶ chunk/s).

## پذیرفته ولی هنوز در موتور نیست

reranker v2 برای پرسش‌های کوتاه زبان ساده: ripgrep R@3 0.208 → 0.583، click 0.750 → 0.958، self
0.643 → 0.679 (فقط ترتیب را عوض می‌کند، نه محتوا). گام بعدی: پیاده‌سازی اختیاری (فقط وقتی مدل نصب
است) + یک ست تازه‌ی holdout برای تأیید.

## گام‌های بعدی

1. انتشار 1.2.0 (بالا).
2. reranker v2 برای پرسش‌های ساده در موتور.
3. مقاله §5: holdout‌های بزرگ‌تر (≥۵۰ سؤال، دو نفر)، task success per token با مدل، ablation جدول.
4. مرحله‌ی LLM (Agentless-style، انتخاب از `where_to_look`) — نیاز به کلید API.

## تله‌ها

- `df -h /c` قبل از کار؛ target/debug > 10GB → پاک کن.
- شاردهای harness را با hash پایدار تقسیم کن (حالا پیش‌فرض)؛ هرگز دو اجرا روی یک فایل خروجی.
- clone‌ها blobless‌اند: قطع proxy = خطای checkout (۴۶ pydicom یک بار).
- `sed` با `\\` در MSYS خراب می‌کند؛ ویرایش Python با نرمال‌سازی CRLF یا Edit.
