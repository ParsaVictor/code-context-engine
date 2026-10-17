# پرامپت چت بعدی (session 20) — مسیر سریع

این متن را کامل در چت جدید Claude Code (پوشه‌ی `C:\1\1_پروژه\5_neuromesh`) بچسبان:

---

پروژه NeuroMesh است (repo در `repo/`، داده‌ها در `swebench/`). اول این‌ها را بخوان:
`repo/docs/planning/16-gap-analysis-and-phases.fa.md`، `repo/docs/planning/17-next-chat-prompt.fa.md`،
و در `repo/docs/research/contributions-log.md` بخش‌های §8.18 تا §8.21. هیچ سؤالی از من نپرس. همه‌چیز
مجاز است: commit، PR، merge، release. گزارش کوتاه بده و توکن کم مصرف کن: فقط خروجی لازم را بخوان (grep یا tail).

**وضعیت:** func Acc@5 روی Lite-274 برابر 0.496 است (SweRank vote). برای ۸۳٪ issueها همه‌ی توابع gold بین ۳۰۰ کاندید
اول engine هستند. مشکل مرتب‌سازی است، پس هدف یک reranker با LLM است.
لیست‌های ۳۰۰تایی: `swebench/{dev-s18a-all,gym-v140-all,lite-v140d300-all,verified-v140d300-all}.jsonl`.
gold توابع: `swebench/func-gold-{dev,gym,lite,verified}.json`. ابزارها: `scripts/research/{paired.py,func_eval.py,llm_localize.py,dense_shortlist.py}`.
کلیدهای LLM فقط در `C:\Users\parsa\.neuromesh-keys.json` هستند (هرگز در repo نگذار). تست کلیدها:
`py -3 C:/w/keytest.py C:/Users/parsa/.neuromesh-keys.json`. اگر همه HTTP 402 دادند، فاز L را رد کن و برو سراغ فاز C.

**قانون سرعت (سخت‌گیری کمتر):** غربال روی نمونه‌ی کوچک (dev-fast ۵۹ + gym-a ۱۴۰). هر ایده‌ای که روی هر دو بهتر بود
(+0.02 در func@5 یا file@1) نگه دار. Lite/Verified فقط یک بار، در آخر، برای ترکیب نهایی اجرا شود.

## فازها
- **L1 (۲–۳ ساعت):** `scripts/research/llm_rerank.py` بنویس: listwise rerank روی ۲۰ تابع برتر ورودی (= ترتیب
  SweRank/jina vote یا engine). prompt شامل issue (حداکثر ۴۰۰۰ کاراکتر) و برای هر کاندید path، name و ۱۵ خط اول بدنه.
  خروجی: لیست شماره‌ها به ترتیب. cache پاسخ‌ها با hash. ۴ مدل Baseten را روی ۳۰ issue از dev-fast مقایسه کن
  (توکن و هزینه را ثبت کن) و ارزان‌ترین مدل خوب را انتخاب کن.
- **L2 (۲ ساعت):** مدل منتخب روی dev-fast + gym-a. واریانت‌ها: ۲۰ در برابر ۳۰ کاندید، rerank فایل روی ۱۰ فایل برتر، و RRF با ترتیب قبلی.
- **L3 (۱ ساعت):** دسته‌بندی خطاهای باقی‌مانده (حدود ۲۰ مورد) و اصلاح prompt.
- **E (½ روز):** ترکیب برنده در engine با `--rerank llm` (یا `NM_RERANK=llm`)؛ endpoint و کلید از env، و در صورت خطا fallback به لیست فعلی.
- **H (۳ ساعت):** یک اجرای نهایی روی Lite-274 و Verified، جدول هزینه-دقت (توکن، $/issue، ثانیه)، و ثبت در contributions-log §8.22.
- **P (½ روز):** به‌روزرسانی `paper/main.tex` و paper-draft، انتشار v1.5.0 (tag، باینری‌ها، نصب)، به‌روزرسانی memory.
- **C (اگر کلید کار نکرد):** نتیجه‌ی cross-encoder (فایل‌های `swebench/ce-sl-{dev,gym}.jsonl`، pre-reg در
  `docs/research/prereg-s19-crossencoder.md`) را با `priors.py --prior ext --ext ce-sl-X.jsonl::dense_defs` بسنج.
  اگر pass شد، یک بار روی holdout اجرا کن. بعد مقاله را نهایی کن (workshop) و v1.5.0 را با `NM_DENSE` منتشر کن.

**قواعد:** یک build در هر لحظه. `df -h /c` را چک کن (حدود ۲۲ گیگ آزاد). README.md و nm.config.json را commit نکن.
footer commit: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. footer PR: `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
کارهای بدون CPU (مقاله، تحلیل) را در session موازی انجام بده. اجراهای سنگین را هم‌زمان نکن.

---
