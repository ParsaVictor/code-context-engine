# فاز C — دفترچه‌ی اجرا (task-success با مدل واقعی)

وضعیت (۱۴ سپتامبر ۲۰۲۶، بعد از F40–F45): همه‌چیز به‌جز کلید API آماده است. این سند دقیقاً می‌گوید با کلید چه
اجرا شود، روی کدام commit، و چه عددی «قبول» است — تا اجرای C یک جلسه‌ی مکانیکی باشد، نه طراحی.

## چه چیزی از قبل آماده شد (بدون کلید)

| قطعه | کجا | وضعیت |
|---|---|---|
| داور جدا از پاسخ‌دهنده | `neuromesh eval --tasks --executor model` → `--judge-provider`، `--judge-model`؛ پیش‌فرض: همان provider با مدل متفاوت (Anthropic: پاسخ `claude-opus-5`، داور `claude-sonnet-5`)؛ داور = پاسخ‌دهنده بدون `--allow-self-judge` رد می‌شود | ✓ PR این سند |
| `verify` واقعی برای ۵–۸ تسک | `tests/fixtures/mini-orders` (۶ تسک، باگ‌های کاشته‌شده، verify با `node -e`)؛ + `cjs_save_returns_body` قدیمی = ۷ تسک verify-دار | ✓؛ هر ۶ verify روی نسخه‌ی فیکس‌شده pass و روی باگ fail می‌کنند (تست شد) |
| مسیر mock سرتاسری | `--provider mock --mock-reply target/x.patch` → patch apply → verify؛ تست شد: patch درست → `ok`، patch برای تسک دیگر → `FAIL` | ✓ |
| baseline سرعت (G4/G5) | همان‌ماشین، همان‌روز: 62b4f8b index 3478ms p50 839ms؛ 1db1380 index 3415ms p50 659ms (۰.۹۸× / ۰.۷۹×)؛ `l1_p95` 49→42ms | ✓ بدون regression |

## اجرای C — دستورها

پیش‌نیاز: `export ANTHROPIC_API_KEY=...` (فقط در shell، هرگز در فایل ریپو). باینری بدون `--features embeddings`.

```bash
# 1. baseline «قبل از B» — commit 7442600 (بعد از فاز A، قبل از F33)
git worktree add /tmp/nm-before-b 7442600
(cd /tmp/nm-before-b && CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/tmp/nm-before-b-target cargo build --release -p neuromesh-cli)

# 2. فعلی
CARGO_BUILD_JOBS=2 cargo build --release -p neuromesh-cli

# 3. برای هر باینری، هر سه context، روی تسک‌های فیکسچر + third-party (dev + holdout):
for ctx in packet whole-gold-files grep; do
  ./target/release/neuromesh eval --tasks --executor model --context $ctx --json \
    > "reports/phase-c/current-$ctx.json"
  /tmp/nm-before-b-target/release/neuromesh eval --tasks --executor model --context $ctx --json \
    > "reports/phase-c/before-b-$ctx.json"
done
```

نکته: باینری قدیمی (`7442600`) flag داور جدا را ندارد — داور = پاسخ‌دهنده خواهد بود؛ این را در گزارش ذکر کن
(عدد baseline خوش‌بینانه‌تر است، نه بدبینانه‌تر، پس مقایسه به ضرر ما خطا می‌کند — قابل قبول).

تسک‌های third-party: `tests/third_party/*/tasks.toml` از طریق `--tasks-file` (هر ریپو جدا، چون `repo =` نسبی است
و checkout در `target/third_party/...` است — اول `scripts/fetch-third-party.sh` برای هر سه مانیفست).

## گیت‌ها (از سند ۰۶)

- `task_success ≥ 0.5` روی holdout (gin+torchvision) با `--context packet`.
- `success_per_1k_tokens` در `packet` > `whole-gold-files` (ادعای اصلی: packet ارزان‌تر *و* کافی است).
- `packet` ≥ `grep` در task_success (وگرنه «grep کافی است» درست است).

## چه چیزی هنوز اندازه‌گیری نشده و چرا

- هیچ عدد مدل واقعی — کلید نیست. عددهای oracle (reachable/strict) جانشین نیستند؛ در README به‌جای آن ننویس.
- ۵b (B2B خصوصی) — گلد Parsa.
- F25 — تصمیم Parsa (G3).

## هزینه‌ی تقریبی

۲۲ تسک فیکسچر + ۲۱ dev + ۲۰ large + ۲۰ holdout ≈ ۸۳ تسک × ۳ context × ۲ باینری ≈ ۵۰۰ جفت فراخوانی (پاسخ + داور
یا patch)؛ context ≈ ۱–۲۰k توکن. با opus/sonnet در حد چند ده دلار. اگر محدودیت بود: فقط holdout + فیکسچر (۴۲ تسک).
