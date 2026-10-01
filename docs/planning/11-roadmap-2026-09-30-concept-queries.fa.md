# نقشه راه — بعد از گزارش یوسف (۲۰۲۶-۰۹-۳۰)

ورودی: `handoff-2026-09-30-yoosef-findings.fa.md` — ۳ از ۷ روی ریپوی خودمان، ۲۶ ثانیه تاخیر روی یک query، و باگ stdout در حالت `mcp`.

## تشخیص اولیه (قبل از هر اندازه‌گیری)

ست `self` (۳۰ سوال) تقریباً همه‌اش اسم یک شناسه را در متن دارد (`resolve_cluster_noun_seeds`, `select()`, …). سوال‌های یوسف **مفهومی** هستند و هیچ شناسه‌ای ندارند: «ابزار چطور جلوی ایندکس کردن مسیرهای خطرناک مثل ریشه فایل‌سیستم را می‌گیرد؟». پس ست‌های ما این کلاس را اصلاً اندازه نمی‌گرفتند؛ عدد خوب روی `self` و عدد بد یوسف با هم تناقض ندارند. این کلاس (سوال مفهومی، بدون اسم) رایج‌ترین شکل سوال کاربر واقعی است، پس اولویت اول است.

دومین مشکل: وقتی جواب غلط است، `coverage: no_recorded_gap` می‌گوید (اعتماد کاذب). عامل از روی این عدد تصمیم می‌گیرد جستجوی بیشتری بکند یا نه — اعتماد کاذب از جواب غلط بدتر است.

## فازها

| فاز | کار | معیار خروج |
|---|---|---|
| **A** — پروتکل | بنر داشبورد از stdout به stderr؛ تست یکپارچه: در حالت `mcp` هر خط stdout باید JSON معتبر باشد | تست سبز، روی باینری واقعی چک شده |
| **B** — ست مفهومی (قفل قبل از تیونینگ) | (۱) ست `concept-dev`: ۴ سوال یوسف + ~۱۲ سوال مفهومی دیگر روی همین ریپو، gold با خواندن کد. (۲) ست `concept-holdout`: ~۱۲ سوال مفهومی روی یک ریپوی **Rust شخص ثالث** (ripgrep، revision پین‌شده) که تا پایان فاز D اجرا نمی‌شود. gold هر دو قبل از دیدن هر packet قفل و commit می‌شود | دو فایل gold commit شده؛ baseline روی `concept-dev` اندازه‌گیری شده |
| **C** — تاخیر | بازتولید ۲۶ ثانیه با زمان‌گیری دقیق (باینری release، cache خاموش/روشن)؛ اگر واقعی بود پروفایل و رفع | p95 تک‌query روی ریپوی خودمان < ۲ ثانیه بعد از ایندکس |
| **D** — بازیابی مفهومی | ریشه‌یابی هر miss با `NM_EXPLAIN`؛ فقط قانون‌های عمومی (مثلاً: کلمات سوال ↔ doc-comment و نام فایل/ماژول، هم‌ریشه‌سازی `dangerous→safe/unsafe`, `cap→max/limit`)؛ کالیبره کردن `coverage` تا وقتی seed ضعیف است `partial`/`no_confident_match` بگوید | `concept-dev` ≥ ۰.۸۵ recall@packet، هیچ «no_recorded_gap» روی جواب غلط؛ ۹ ست قبلی بدون افت (ratchet −۰.۰۲) |
| **E** — اثبات روی holdout | یک بار اجرای `concept-holdout` بعد از قفل شدن کد | عدد صادقانه گزارش شود، چه خوب چه بد؛ اگر < ۰.۷ یافته‌ها ثبت و یک دور اصلاح عمومی دیگر |
| **F** — انتشار | v1.0.1: CHANGELOG، `docs/measured.md` (ستون جدید concept)، باینری‌ها از release.yml، جواب انگلیسی برای یوسف با اعداد و ground truth اصلاح‌شده `retry_negative` | tag + سه باینری، سند پاسخ |
| **G** — بک‌لاگ | ستون‌های token_precision/overhead (measure-only)، سپس GitHub Pages rebrand | در `measured.md` و Pages فعال |

## قواعد اجرا

- هر فاز یک branch و یک PR روی `ParsaVictor/code-context-engine`؛ merge بعد از CI سبز.
- فایل‌های تغییرداده‌شده توسط session دیگر (`README.md`, `crates/neuromesh-cli/src/main.rs`, `nm.config.json`) دست نمی‌خورند؛ فقط `git add <file>` با نام.
- gold قبل از packet قفل؛ هیچ قانونی که فقط یک سوال را درست کند (hardcode اسم) پذیرفته نیست.
- ratchetها فقط بالا می‌روند.
- `retry_negative` باگ نیست: این fork واقعاً retry دارد (`crates/neuromesh-provider/src/anthropic.rs`)؛ در ست ما gold آن همان فایل‌های provider است.

## به‌روزرسانی ۲۰۲۶-۱۰-۰۱ — نتیجه‌ی A–E و فازهای بعدی

انجام‌شده: A (#126)، B، C، D، E در PR #127 — concept 0.286→0.679، holdout ripgrep 0.042→0.500، cold start 3.6→2.6 s، ۹ ست بدون افت.

| فاز | کار | الگو | معیار خروج |
|---|---|---|---|
| H | gate نهایی + انتشار 1.1.0 | — | tag + سه باینری |
| I | harness در release + اجرای موازی ست‌ها | — | دور کامل ≤ ۱۰ دقیقه |
| J | ادغام RRF رتبه‌ی BM25F با embedding (MiniLM موجود) فقط برای سؤال بدون anchor | Cody / Continue / claude-context (hybrid) | concept-holdout ≥ 0.7 روی holdout تازه |
| K | packet در سطح تعریف برای سؤال مفهومی + رتبه‌ی PageRank داخل packet | Aider repo-map، chunking در claude-context | precision مفهومی ≥ 0.4 بدون افت recall |
| L | holdout تازه‌ی دوم (Python)، gold قفل قبل از اجرا | — | عدد صادقانه |
| M | مقایسه‌ی مستقیم با رقبا (claude-context، Serena، Aider repo-map) روی همین ست‌ها | — | جدول منتشرشده در `docs/measured.md` |
