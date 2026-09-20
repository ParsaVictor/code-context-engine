# Handoff — session 13 (2026-09-20)

main بعد از PR #101. سند قبلی: `handoff-2026-09-20-session12.fa.md` (بخش ۳ تله‌ها همچنان معتبر).

## ۱. چه شد

| PR | finding | اثر |
|---|---|---|
| #98 | F70 — `resolve_cluster_noun_seeds` بدون ترتیب کامل (HashMap tie) + «اسم بدون تمایز» (≥۳ فایل هم‌نام، بدون bonus) | large 0.578→0.583؛ `ultra_predict_stream` بین اجراها ثابت |
| #99 | F71 خواننده‌ی کلید config با stem نام‌برده صندلی می‌گیرد؛ F72 ترکیب `defaults:` هایدرا (yaml→Imports، linker: import هرگز به فایل خودش، composer صندلی می‌گیرد) | cfg recall 0.924→1.000، precision 0.690→0.712 |
| #100 | F68 — micro-header بعد از prune | ۸ مجموعه ثابت |
| #101 | F73 — alias با `contains` خام («in**validate**»، «Command**Error**») → term باید ابتدای کلمه باشد | large 0.583→**0.641**، holdout-ml 0.432→0.437 |

اعداد فعلی (recall همه 1.000): dev 0.938 · large 0.641 · holdout-2 0.554 · holdout-c 0.573 · holdout-lang 0.541 ·
holdout-ml 0.437 · holdout-ml2 0.632 · cfg 0.712. ratchet: large 0.62، cfg 0.95/0.69.

## ۲. تله‌های جدید

- `stage4_security` wall-clock (<30s) روی این ماشین **روی main دست‌نخورده هم ۵۱–۵۵s** است (بار ۱۲٪). CI لینوکس مرجع؛
  محلی نادیده بگیر، ولی اگر CI هم قرمز شد واقعی است.
- `target/explain-<set>.txt` append می‌شود (fresh + cached در یک process، و بین اجراها). قبل از هر probe `rm` کن؛
  `NM_EXPLAIN_MAX_PRECISION=1.01` همه‌ی تسک‌ها را می‌نویسد (پیش‌فرض فقط <0.6).
- `grep` در pipe پس‌زمینه block-buffered است — با `--line-buffered` یا صبر تا پایان.
- تغییر در `neuromesh-parser`/`graph` کش ایندکس را باطل می‌کند → large ~۱۰ دقیقه به‌جای ~۲.
- CLI: `neuromesh index`/`packet` روی cwd کار می‌کند؛ مسیر positional را workspace نمی‌گیرد (اشتباهاً خود ریپو را ایندکس کرد).
- probe لبه‌های گراف: تست موقت `tests/zz_probe_tmp.rs` با `support/index_cache::graph_for_checkout` + `get_edges_map`
  (F71/F72 هر دو با همین پیدا شدند). قبل از commit حذف شود.

## ۳. درس اصلی این session

dogfood با ۵ سؤال واقعی روی خود ریپو → ۱ finding (F73) → ‎+0.058 روی large. تیون امتیاز هیچ؛ باگ ساختاری همه‌چیز.
مسیر پیشنهادی جهش (به Parsa گفته شد، منتظر تصمیم): ۳۰ سؤال واقعی B2B (تیم بنویسد؛ = 5b) + ۲۰ روی این ریپو، یک batch
probe، فیکس خوشه‌ای؛ کلید Baseten برای فاز C؛ هدف صریح precision ≥0.75 / recall ≥0.95 روی ریپوی ندیده.

## ۴. باز مانده

1. فاز C روی main فعلی — کلید Baseten.
2. 5b — گلد خصوصی تیم.
3. dogfood ادامه: سؤال Q4 همان session («index cache … invalidate») بعد از F73 دوباره probe نشده — `index` به متد PHP در
   `tests/fixtures/mini-pinoox` می‌رفت و `tests/support/index_cache.rs` (stem `index_cache`) نمی‌آمد؛ احتمالاً F74
   (fixture ی خود ریپو به‌عنوان نویز + stem دوکلمه‌ای).
4. feedback منفی روی strong-seed twin عمداً بی‌اثر — اگر Parsa بخواهد.

## ۵. ادامه‌ی session 13 (بعد از گزارش)

| PR | چه |
|---|---|
| #103 | F74 — «index cache» → `index_cache.rs` (compound stem)، نیمه‌های bare حذف؛ ۸ مجموعه ثابت |
| #104 | مجموعه‌ی self dogfood (`tests/third_party/self`, ۱۰ سؤال) + F75: **recall 0.600 / precision 0.325 روی خود ریپو**، پنج خوشه‌ی ریشه |

**فاز C اجرا نشد:** کلید Baseten داده شد؛ endpoint برای این ماشین 403 (Cloud Armor؛ حتی بدون کلید؛ exit VPN آذربایجان و
بعد یک IP دیگر هم 403). دستور چک: `curl -s -o /dev/null -w "%{http_code}\n" https://inference.baseten.co/v1/models` —
باید 401/200 بدهد. بعدش: `OPENAI_BASE_URL=https://inference.baseten.co/v1 OPENAI_API_KEY=… PROVIDER=openai
PAIRS="deepseek-ai/DeepSeek-V4-Flash-0731:zai-org/GLM-5.3" bash scripts/phase-c-run.sh` (نتیجه‌ی قبلی در
`reports/phase-c-2026-09-15-pr81`، git-ignored). کلید در هیچ فایلی نیست.

**قدم بعدی پیشنهادی (به ترتیب):** F75-B (fallback seeds در مسیر نویز/تست) → F75-A (ایندکس literal های snake_case؛ feature)
→ فاز C به محض باز شدن مسیر → ۳۰ سؤال B2B تیم (5b).

## ۶. ادامه (بعد از دور سوم dogfood و holdout-web)

| PR | چه |
|---|---|
| #108 | F75 batch + F77 (ایندکس literal، seedهای حدسی خارج از نویز، acronym، stem-prefix، داور با سورس واقعی) |
| #109 | F78: اسکریپت‌های `.sh`/`.ps1` ایندکس؛ kebab token = فایل؛ env-var literal؛ artifact ML خارج از fixtures |
| #110 | F79: engine hybrid/MiniLM **بدتر** از fast (self 0.500/0.122 vs 0.675/0.406) — embedding اهرم نیست |
| #111 | F80: **holdout-web** (fastify/demo + shadcn taxonomy، ۲۰ سؤال، گلد کور) — untuned 0.542/0.402 → 0.642/0.502 |
| #112 | F82: stem قراردادی (`route.ts`…) نام‌برده نیست؛ web forbidden 0، precision 0.602؛ large 0.666 |
| #113 | ترمیم: sed بازه‌ای در #112 ماژول تست activator.rs (۲۴۰۴ خط) را برید؛ CI سبز ماند چون lib کامپایل می‌شد. **تله: بعد از هر sed بازه‌ای `wc -l` و `git diff --stat`** |

اعداد فعلی (recall/precision): dev 1.0/0.938 · large 1.0/**0.666** · holdout-2 1.0/0.554 · -c 1.0/0.573 · -lang 1.0/0.541 · -ml 1.0/0.437 ·
-ml2 1.0/0.632 · cfg 1.0/0.712 · **web 0.642/0.602** · self(20) 0.675/0.406.

ثبت‌شده و ship‌نشده (باز): F81 (Calls از تابع هم‌نام به فایل می‌چسبد؛ فیکس درست large را 0.558 می‌کند — اول صندلی callee
سخت‌گیرتر شود)، قاعده‌ی «کلمه‌ی نام‌پوشه» (web +0.125 recall ولی تست kosha)، path-words با anchor (web 0.742 ولی c/cfg −0.08).
