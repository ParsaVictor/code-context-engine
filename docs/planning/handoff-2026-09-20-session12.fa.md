# Handoff — session 12 (۱۹–۲۰ سپتامبر ۲۰۲۶): فاز D کامل، برنامه‌ی سرعت، dogfooding

قبل از این: `handoff-2026-09-15-phase-c.fa.md`، `08-roadmap-2026-09-20.fa.md` (رود‌مپ اجرایی این session)،
`stage5-findings.fa.md` (F55–F69). این سند = گزارش کامل کارهای انجام‌شده در غیاب Parsa + آموزه‌ها + پرامپت شروع چت بعدی.

## ۱. چه شد (به ترتیب زمان)

| PR | کار | نتیجه‌ی عددی |
|---|---|---|
| #84 | F30: dispatch با keyword-argument (`self.student(img, distill_token=…)` → `DistillMixin.forward` باز می‌ماند) | dev strict 19→20 |
| #85 | F31: یال `Api → handler` در parser/graph؛ gate ی sidecar در هر اندازه؛ **F57**: حلقه‌ی یادگیری روی هر ریپوی >۲۰ فایل خاموش بود | بدون تغییر عدد؛ حلقه‌ی feedback باز شد |
| #86/#87 | holdout-ml2 (peft + keras-hub) گلد قفل قبل از اجرا؛ اولین اجرا | **1.000 / 0.632 / 0** — گیت بدون تیون پاس شد (holdout-ml قدیمی: dev-class) |
| #88 | F59 (harness strict را کم گزارش می‌کرد)، F58 (نام dunder/snake در prompt)، F60 (orchestrator کلاس) | large strict 14→19، strict همه‌ی holdoutها بالا |
| #89 | F61: seed ضعیف در svg/css، مترادف بی‌owner، acronym کنار identifier بلند | large 0.502→0.541، holdout-2 0.496→0.538، lang 0.502→0.536 |
| #90 | **S1** کش ایندکس harness (large ۴۱۶s→۱۵۱s)، **S2** ابزار `NM_EXPLAIN=1`؛ **F63** id تکراری در indexها | holdout-c 0.656→**0.573** (اندازه‌گیری تصحیح‌شده — عدد قبلی با تکرار id باد کرده بود) |
| #91 | F64: twin هم‌نام → بدنه‌ای که prompt توصیف می‌کند + پوشه‌ای که نام می‌برد | large recall 0.95→**1.0**، 0.541→0.566، strict 20/20؛ dev 0.921 |
| #92 | **F62 (dogfood)**: کلیدواژه‌ی حدسی سرور با tier قوی؛ concept→identifier فقط در مسیر tiered (MCP/CLI) | dev **0.938**، large 0.574، holdout-ml 0.437، cfg 0.690؛ packet dogfood ۲۶۹۳→۸۴۱ توکن |
| #93 | F55: «the X key» → seed کلید config؛ فایل config که مقادیرش reader را نام می‌برد | cfg recall 0.879→**0.924** |
| #94 | F65: کلمه‌ی برهنه به فایل markdown با پیشوند نمی‌رسد | large 0.578 |
| #95 | F66 header کلاس گم‌شده + `}` سرگردان در پایتون؛ F67 fold ی docstring در بدنه‌ی باز | large −۶.۸٪ توکن؛ متن پایتون معتبر |
| #96 | README + measured.md = اعداد واقعی ۲۰ سپتامبر | — |
| #97 | **F69**: feedback مثبت precision را ۰.۹۳۸→۰.۶۷۶ می‌انداخت (`reinforce_callee_edges` کل همسایگی را تقویت می‌کرد)؛ feedback منفی روی فایل required بی‌اثر بود | positive بی‌ضرر؛ negative: fastapi_login 0.75→1.00 |

**Dogfooding:** باینری دسکتاپ Parsa تا ۲۰ سپتامبر **upstream v0.7.17** بود (نه fork!). حالا fork نصب است
(`%LOCALAPPDATA%\Programs\neuromesh\neuromesh.exe`، backup قدیمی کنارش)، گراف‌های قدیمی پاک، راه‌اندازی ۳ دقیقه ۳۷ ثانیه.
اولین سؤال واقعی → F62 (باگ فقط-MCP که هیچ بنچمارکی نمی‌دید).

## ۲. اعداد نهایی (main بعد از #97، `bash scripts/benchmark-holdout.sh`)

| set | recall | precision | forbidden | strict | نقش |
|---|---|---|---|---|---|
| dev-4 | 1.000 | 0.938 | 0 | 20/21 | dev |
| large | 1.000 | 0.578 | 0 | 20/20 | dev |
| holdout-2 (gin, torchvision) | 1.000 | 0.554 | 0 | 18/20 | **holdout** |
| holdout-c (libuv, fmt) | 1.000 | 0.573 | 0 | 14/16 | **holdout** (گیت target ۰.۶۰ قرمز، صادقانه) |
| holdout-lang (Scala/R/Julia) | 1.000 | 0.541 | 1 | 12/15 | **holdout** |
| holdout-ml (keras-io, setfit) | 1.000 | 0.432 | 0 | 10/10 | dev-class |
| holdout-ml2 (peft, keras-hub) | 1.000 | 0.632 | 0 | 7/10 | **holdout** (تیون‌نشده) |
| holdout-cfg | 0.924 | 0.690 | 0 | — | dev-class |

## ۳. آموزه‌ها (تکرار نشود)

1. **مسیر MCP/CLI ≠ مسیر harness** (`activate_tiered` vs `activate`). باگ فقط-MCP (F62) را فقط dogfooding دید. هر فایل
   عجیب در استفاده‌ی روزانه = finding.
2. **اول دلیل، بعد فیکس.** `NM_EXPLAIN=1` → `target/explain-<set>.txt`. F59/F61/F62/F65 همه از dump آمدند؛ حدس امتیاز هیچ‌وقت.
3. **نتیجه‌ی مخلوط = ship نکن.** F64 نسخه‌ی اول holdout-c را شکست؛ سیگنال گم‌شده (کلمات پوشه) اضافه شد، نه threshold.
4. **یک بار تیون روی holdout = dev-class.** در measured.md بنویس (holdout-ml، holdout-cfg).
5. **دو مسیر گراف (fresh vs snapshot) باید یکی باشند.** F63 از مقایسه‌ی fresh/cached کش لو رفت؛ عدد holdout-c اصلاح شد.
6. **ابزار، نه eprintln.** `cargo fmt` خط probe موقت را می‌شکند و `sed` بعدش فایل را خراب می‌کند (دو بار). probe را با Edit
   بگذار/بردار یا `git checkout --`. هرگز حین cargo پس‌زمینه ویرایش نکن.
7. **گیت‌های wall-clock** (`stage4_security` <30s، index <30s) زیر بار هم‌زمان قرمز می‌شوند؛ تنها بسنج.
8. پیام commit با `-F file`؛ heredoc + backtick متن را می‌خورد. perl روی فایل CRLF match نمی‌کند — Edit tool.
9. **feedback با معنی درست:** «این فایل مفید بود» نباید همسایگی‌اش را تقویت کند (F69). «مفید نبود» باید روی فایل required
   غیرseed اثر کند.

## ۴. باز مانده (به ترتیب پیشنهادی)

1. **فاز C دوباره روی main فعلی** — نیاز به کلید Baseten (Parsa). ۸ PR روی packet اثر گذاشته‌اند (docstring fold، header کلاس)؛
   task-success باید دوباره سنجیده شود.
2. `ultra_predict_stream`: مجموعه‌ی `val.py` های هم‌نام بین دو اجرا فرق می‌کند (nondeterminism قدیمی، precision ثابت).
3. `hydra_early_stopping` (ترکیب `defaults:` هایدرا)، `detr_masks_flag` (خواننده‌ی دوم argparse).
4. F68: `@nm:seeds` در micro-header هنوز seedهای prune‌شده را نشان می‌دهد (cosmetic).
5. حلقه‌ی یادگیری: feedback منفی روی فایل‌های *strong seed* (twin هم‌نام) عمداً بی‌اثر است — اگر Parsa بخواهد، بحث.
6. private B2B: گلد تیم (۵b) هنوز نیامده.

## ۵. پرامپت شروع چت بعدی

```
پروژه: fork NeuroMesh در C:\1\1_پروژه\5_neuromesh\repo (GitHub ParsaVictor/code-context-engine). حافظه‌ات را بخوان
(MEMORY.md: neuromesh-session12، neuromesh-speed-plan، neuromesh-dogfood-setup، feedback-parsa-working-style،
feedback-token-budget). سند مرجع: docs/planning/handoff-2026-09-20-session12.fa.md و 08-roadmap-2026-09-20.fa.md.

وضعیت: فازهای A–E انجام شده؛ اعداد در docs/measured.md (dev 0.938، large 0.578، holdout-2 0.554، holdout-c 0.573،
holdout-ml2 0.632 تیون‌نشده). دسکتاپ Parsa روی fork است. ابزارها: `bash scripts/benchmark-holdout.sh [set]` (کش
ایندکس؛ large ~۲.۵ دقیقه)، `NM_EXPLAIN=1` برای probe، `cargo test -p neuromesh-context --test learning_loop` برای حلقه‌ی
feedback.

قوانین: holdout تیون نمی‌شود (holdout-2، holdout-c، holdout-lang، holdout-ml2)؛ probe فقط روی dev/large؛ ratchet فقط
بالا؛ نتیجه‌ی مخلوط → revert + ثبت؛ هر PR = ۸ مجموعه + workspace tests + clippy سبز + CI سبز → مرج (اختیار تام).
تله‌ها: بخش ۳ همین handoff. هیچ eprintln موقتی با sed پاک نکن؛ حین cargo پس‌زمینه ویرایش نکن؛ commit با -F.

اول: (۱) اگر کلید Baseten داده شد، فاز C روی main فعلی (scripts/phase-c-run.sh، docs/planning/phase-c-runbook.fa.md).
(۲) وگرنه از بخش ۴ handoff شروع کن، به ترتیب. بعد از هر PR گزارش ۳–۵ خطی (جدول قبل/بعد + قدم بعد)؛ وقتی Parsa گزارش
کامل خواست: مرحله را تمام کن، مستند کن، handoff و پرامپت بعدی را بنویس.
```
