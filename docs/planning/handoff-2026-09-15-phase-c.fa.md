# Handoff — ۱۵ سپتامبر ۲۰۲۶: فاز C (task-success با مدل واقعی) — آماده‌ی اجرا

قبل از این: `handoff-2026-09-14c.fa.md` (session 10)، `07-session10-plan.fa.md`، `phase-c-runbook.fa.md`.

## وضعیت لحظه‌ی handoff

- **PR #80 باز** (برنچ `phase-c-router`): همه‌ی فیکس‌های harness که فقط با مدل واقعی معلوم شد + یک فیکس موتور
  (skeletonizer خطوط خالی را در gap ماژول حذف می‌کرد → patch مدل روی packet apply نمی‌شد ولی روی whole-file می‌شد؛
  یعنی سوگیری علیه packet). با CI سبز مرج کن. `reports/phase-c/*.json` توی این PR **ناقص/بی‌اعتبار** است (اجراهای
  قطع‌شده) — پاک کن و از نو بساز.
- **هیچ عدد فاز C هنوز معتبر نیست.** تنها اعداد تمیز (deepseek-v4-flash پاسخ، glm-5.3 داور، روی agentrouter قبل از
  قطعی): فیکسچر packet 23/26 = 0.885 · whole 22/26 = 0.846 · grep 17/26 = 0.654؛ holdout-gin packet 5/10 · whole 3/10.
  این‌ها **قبل از فیکس skeleton** بودند (packet عمداً ضعیف‌تر بود) — باید دوباره اجرا شوند.

## گیت‌وی‌ها و کلیدها (فقط در shell؛ هرگز در فایل ریپو)

| گیت‌وی | base url | کلید | وضعیت تست‌شده |
|---|---|---|---|
| **9router (لوکال، ترجیحی)** | `http://localhost:20128` | `sk-7e0af1136bf0caf5-yrlmdm-03f1b9b5` | مدل‌های Cloudflare کار می‌کنند (پایین)؛ gemini/* → 403؛ deepseek-v4-pro، kimi-k3، kimi-k2.6، glm-4.7-flash → 402/403/400 (بدون اعتبار) |
| agentrouter | `https://agentrouter.org` (+ `ANTHROPIC_USER_AGENT='claude-cli/2.0.14 (external, cli)'`) | `sk-c7aimd4BTq5cWKKcsdEOS6pxEg9yVBe1KhNo9vUpNSrlWzWn` | deepseek-v4-flash + glm-5.3 کار می‌کردند؛ claude/gpt «Budget pool exhausted»؛ گاهی 504 |

**نتیجه‌ی تست 9router روی تسک واقعی (`cjs_save_returns_body`، patch + verify):**
- `cf/@cf/meta/llama-3.3-70b-instruct-fp8-fast` → **ok** (verify exit 0)، سریع (۱–۳s) ← **پاسخ‌دهنده‌ی پیش‌فرض**
- `cf/@cf/qwen/qwen2.5-coder-32b-instruct` → diff می‌نویسد ولی context خودش را عوض می‌کند (`createStore({})`)، patch fail
- `cf/@cf/deepseek-ai/deepseek-r1-distill-qwen-32b` → `<think>` طولانی، patch fail
- `cf/@cf/mistralai/mistral-small-3.1-24b-instruct` → diff درست در تست ساده؛ **داور** پیش‌فرض
- پک‌های 9router (`claude-opus-5` و…) fallback می‌کنند به مدل‌های در دسترس — نام مدل واقعی در فیلد `model` پاسخ می‌آید؛ برای گزارش، مدل *تک* را مستقیم بده نه پک.

## دستور اجرا (یک خط، resumable)

```bash
cd /c/1/1_پروژه/5_neuromesh/repo
rm -f reports/phase-c/*.json reports/phase-c/*.log
export ANTHROPIC_API_KEY='sk-7e0af1136bf0caf5-yrlmdm-03f1b9b5' ANTHROPIC_BASE_URL=http://localhost:20128 ANTHROPIC_MAX_RETRIES=3
PAIRS="cf/@cf/meta/llama-3.3-70b-instruct-fp8-fast:cf/@cf/mistralai/mistral-small-3.1-24b-instruct cf/@cf/mistralai/mistral-small-3.1-24b-instruct:cf/@cf/meta/llama-3.3-70b-instruct-fp8-fast" \
  bash scripts/phase-c-run.sh
```
خروجی: `reports/phase-c/<set>-<ctx>-<model>.json` برای fixtures / holdout-gin / holdout-vision × packet / whole-gold-files / grep.
هر فایل یک خط `Task success (...)` دارد. اگر یک جفت مدل خطای provider داد، runner خودش جفت بعدی را از اول همان context می‌زند.
قبل از اجرا 9router باید روی ماشین بالا باشد (`curl localhost:20128/v1/models`).

## گیت‌ها (سند ۰۶) — روی holdout (gin+vision، ۲۰ تسک) با `--context packet`
1. `task_success ≥ 0.5`
2. `success per 1k tokens` در packet > whole-gold-files
3. task_success packet ≥ grep

## بعد از اجرا
1. جدول ۳×۳ (fixtures / gin / vision × سه context) با مدل واقعی در `stage5-findings.fa.md` و `measured.md` («Task success with a real model» را از «Not measured» بردار).
2. عدد را به‌عنوان «کف با مدل ۷۰B متن‌باز» گزارش کن، نه عدد نهایی؛ اگر بعداً کلید Claude آمد، همان runner با `PAIRS="claude-opus-5:claude-sonnet-5"`.
3. README: فقط اعداد holdout.

## قوانین کم‌مصرف (تأیید Parsa)
- کار مکانیکی (اجرای runner، خواندن ۹ خط عدد، G1 grep) را به زیرعامل ارزان بده؛ داوری/طراحی با مدل اصلی.
- خروجی هر دستور را grep کن؛ dump کامل ممنوع. probe فقط روی افت، برای ثبت درس.
- بنچمارک ۷ مجموعه: label `benchmark` روی PR (CI)، نه لوکال.
- هرگز `git stash/checkout` هم‌زمان با cargo پس‌زمینه؛ exe قفل‌شده → `taskkill //F //IM neuromesh.exe` و `mv` قبل از build.

## تله‌های تازه‌ی فاز C
- 9router و agentrouter پاسخ SSE می‌دهند مگر `stream:false` صریح (فیکس شد).
- مدل‌ها hunk header را غلط می‌شمارند (`--recount`)، prose قبل از diff می‌نویسند (استخراج از اولین `diff --git`/`---`).
- داور با thinking کل بودجه را می‌سوزاند → `max_tokens` داور ۴۰۰۰؛ verdict از اولین خط `PASS/FAIL`.
- فرایند eval زامبی (retry بی‌پایان روی 402) «پول کم نمی‌شود» را توضیح می‌دهد — قبل از هر اجرا `tasklist | grep neuromesh`.
