# Handoff — session 16 (۲۰۲۶-۰۹-۳۰ تا ۲۰۲۶-۱۰-۰۱)

ورودی: گزارش یوسف (۳ از ۷ روی ریپوی خودمان، ۲۶ ثانیه، باگ stdout). نقشه: `11-roadmap-2026-09-30-concept-queries.fa.md`.

## چه چیزی merge شد

| PR | محتوا |
|---|---|
| #126 | بنر داشبورد از stdout به stderr؛ تستی که باینری واقعی را اجرا می‌کند |
| #127 (v1.1.0) | رتبه‌بند BM25F کل‌سؤال (`neuromesh-graph/src/file_rank.rs`)، `comment_index`، thesaurus در `thesaurus.txt`، F89 (کلمه‌ی انگلیسی anchor نیست)، اصلاح cold start، `NM_TIMING=1` |
| #128 | سؤال ساده فقط به رتبه‌بند اعتماد می‌کند؛ رفع panic در `skeleton.rs`؛ `scripts/compare_baselines.py` |
| #129 | جدول مقایسه با BM25 و Aider در `docs/measured.md`؛ holdout دوم (click)؛ folds تکراری؛ حدس‌های سرور «missing» نیستند |

انتشار: tag `v1.1.0` با سه باینری. MCP دسکتاپ پارسا روی v1.1.0 است (backup: `neuromesh-v1.0.0-backup.exe`).

## اعداد (صادقانه)

| ست | v1.0.0 | الان |
|---|---|---|
| concept (ریپوی خودمان، dev) | 0.286 | 0.679 / 0.404 |
| concept-holdout (ripgrep، یک بار) | 0.042 | 0.500 / 0.156 |
| concept-holdout2 (click، تازه، یک بار) | — | 0.958 / 0.342 |
| ۹ ست قدیمی | — | بدون افت؛ web 0.608→0.643 |
| query اول روی کلون تازه | 3.6 s | 2.6 s؛ بعدی‌ها 0.1–0.4 s |

مقایسه با BM25: روی سؤال‌هایی که اسم کد دارند جلوییم (recall 1.0 با دقت 0.59–0.70)، جز holdout-lang که BM25@1 دقت 0.93 دارد. روی ripgrep (سؤال ساده) مساوی، روی click جلوتر.

## رد شده با عدد (تکرار نکنید)

- seed کردن تعریف‌های فایل بزرگ به‌جای کل فایل: ripgrep 0.500→0.375
- ترکیب با MiniLM: ripgrep 0.375، self 0.607 (MiniLM کد نمی‌فهمد؛ F79 دوباره تایید شد)
- gate روی fillهای packet مفهومی: فایل‌های اضافه خود seedها هستند، نه fill

## باز

- J2: مدل `jina_code_v2` (مخصوص کد) به‌عنوان گزینه اضافه شد (`NEUROMESH_EMBED_MODEL=jina_code_v2`، فایل‌ها در `%LOCALAPPDATA%/neuromesh/models/jina-code-v2`). نتیجه‌ی آزمایش fusion در بخش پایین.
- F92 دقت packet سؤال مفهومی (0.15–0.40)؛ F93 واژه‌هایی که در کد نیست؛ F94 فایل‌های hub.
- holdout-lang: fillهای connector (utility:16–41) دقت را پایین می‌آورند — تیون precision دو بار بسته شده؛ فقط با holdout جدید.

## روش کار سریع

- `bash scripts/benchmark-fast.sh` — ۹ ست در ۱.۵ تا ۵ دقیقه (release، موازی)
- `bash scripts/benchmark-fast.sh concept concept-holdout concept-holdout2`
- یک build در هر لحظه؛ دیسک یک بار پر شد.
