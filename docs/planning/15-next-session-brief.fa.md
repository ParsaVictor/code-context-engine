# راهنمای session 19 — بعد از v1.4.0

> اول این را بخوان، بعد `docs/research/contributions-log.md` §8.13–8.17 و `docs/research/paper-draft.md`.
> همه‌ی اعداد اندازه‌گیری‌شده‌اند؛ منبع هر کدام در contributions-log است.

## ۱. وضعیت در پایان session 18 (۲۰۲۶-۱۰-۱۰)

**نسخه‌ها:** v1.4.0 منتشر و نصب شد (PR #144، tag روی `ee6cc27`، پشتیبان `neuromesh-v1.3.0-backup.exe`). تغییرات engine: رأی traceback→تابع، لیست تابع برای همه‌ی پرسش‌ها در
`packet --json`، اصلاح سقف واژه‌های query در رتبه‌بندی definition. سوییچ پژوهشی `NM_DIAG=1`.

**اعداد (فایل = همه‌ی فایل‌های gold در top-k؛ تابع = همه‌ی توابع ویرایش‌شده در top-k):**

| بنچمارک | نسخه | Acc@1 | Acc@3 | Acc@5 | Acc@10 |
|---|---|---|---|---|---|
| Lite سخت (۲۷۶) | v1.3.0 | 0.507 | 0.717 | 0.750 | 0.815 |
| LocAgent (۲۷۴) فایل | v1.3.0 | 0.500 | 0.723 | 0.755 | 0.828 |
| LocAgent (۲۷۴) تابع | v1.3.0 | 0.168 | – | 0.394 | 0.482 |
| Verified (۵۰۰) | v1.3.0 | 0.446 | 0.680 | 0.736 | 0.814 |
| dev تابع (۲۱۰) | v1.3.0 → v1.4.0 | 0.157 → 0.190 | – | 0.290 → 0.324 | 0.362 → 0.390 |
| LocAgent (۲۷۴) تابع | **v1.4.0** | **0.223** | – | **0.460** | **0.540** |
| Verified تابع (۴۵۹) | v1.3.0 → **v1.4.0** | 0.163 → **0.198** | – | 0.346 → **0.429** | 0.416 → **0.497** |
| فایل Lite/Verified | v1.4.0 | = v1.3.0 (دست نخورد) | | | |

**زبان ساده:** jsoup (holdout تازه‌ی پنجم، جاوا) R@3 فهرست 0.750 در برابر BM25 0.667 (پیش از S1: 0.417).
همه‌ی پنج holdout زبان ساده اکنون دیده شده‌اند — ادعای جدید زبان ساده holdout ششم لازم دارد.

## ۲. قواعد ثابت (همان session 18)

1. تنظیم فقط روی SWE-bench dev (۲۲۵؛ dev-fast ۵۹ / dev-rest ۱۶۶). Lite/Verified یک بار برای هر نسخه، بدون نگاه per-instance.
2. اول prototype پایتونی آفلاین (`scripts/research/priors.py`، `--prior ext` برای هر لیست ذخیره‌شده)، آستانه از قبل:
   file Acc@1 یا func Acc@5 ≥ +0.02 در **هر دو** نیمه. آستانه را بعد از دیدن عدد عوض نکن.
3. **baseline درست:** v1.4.0 روی dev = `swebench/dev-s18a-all.jsonl` (NM_DEFINITIONS=300). فایل
   `dev-func300-all.jsonl` قدیمی است (پیش از #140) — استفاده نکن.
4. مخرج سطح تابع: instance بدون لیست = miss (`func_eval.py` پیش‌فرض).
5. یک build در هر لحظه؛ `df -h /c` (session 18: ۳۸ → ۲۷ GB، بخشی از پروژه‌ی دیگر).
6. تست MCP با `NEUROMESH_NO_BROWSER=1` و Python subprocess. README.md و nm.config.json را commit نکن.
7. harness با `--no-bm25` (BM25 در اجراهای v1.3.0 ذخیره است) — چند برابر سریع‌تر.
8. heredoc حاوی سه backtick ابزار Bash را می‌شکند؛ اسکریپت patch را با Write بنویس.

## ۳. ابزارهای جدید session 18

| ابزار | کار |
|---|---|
| `scripts/research/priors.py` | prototype هر prior (traceback, repro, history, testlink, codeonly, ext) روی دو نیمه |
| `scripts/research/chunk_prf.py` | BM25 سطح definition پایتونی + RM3 + dense روی short-list (با checkout) |
| `scripts/research/dense_shortlist.py` | jina-code روی top-k definition engine (بدون checkout، با cache) |
| `scripts/research/diag_eval.py` | ارزیابی واریانت‌های `NM_DIAG` (no_stem, owner, raw, no_trace) و RRF آن‌ها |
| `scripts/research/doc_bridge.py` | لیست engine برای پرسش‌های ساده (cache) + آزمایش پل doc→code |
| `scripts/research/paper_tables.py` | همه‌ی جدول‌های مقاله از فایل‌های نتیجه |
| `scripts/research/llm_localize.py --model --functions` | مرحله‌ی LLM سبک Agentless (فایل → تابع)، آماده |

## ۴. فازهای پیشنهادی session 19

### فاز A — مرحله‌ی LLM (مسدود: اعتبار API)
هر پنج کلید Baseten که پارسا داد احراز هویت می‌شوند ولی chat → HTTP 402 (بدون اعتبار). با شارژ یکی:
```bash
export LLM_API_KEY=<key>
py -3 scripts/research/llm_localize.py --results ../swebench/dev-s18a-all.jsonl --data ../swebench/devset/dev-fast.json \
  --repos ../swebench/repos --work C:/w/llm --out ../swebench/llm-dev-fast.jsonl \
  --endpoint https://inference.baseten.co/v1 --model deepseek-ai/DeepSeek-V4-Pro-0813 --max-tokens 4000 --functions
```
معیار: Acc@1 فایل و func Acc@5 در برابر engine تنها، با توکن به ازای issue (Agentless ~ده‌ها هزار توکن).

### فاز B — نزدیک‌خطاها روی split بزرگ‌تر
dense short-list (file@1 +0.017/+0.024، +10 ثانیه) و ادغام دو شاخص stem/unstemmed (func@1 +0.039/+0.026)
هر دو مثبت ولی زیر آستانه. پیشنهاد: dev بزرگ‌تر (SWE-Gym یا SWE-bench train، مخازن غیر Lite/Verified)
تا قدرت آماری کافی باشد؛ تصمیم یک‌باره، با آستانه‌ی ثابت.

### فاز C — G (مقاله‌ی main track)
holdout ≥ ۵۰ پرسش زبان ساده با annotator دوم (پارسا/تیم). بدون آن فقط workshop/industry.

### فاز D — مقاله
`paper-draft.md` با `paper_tables.py` هم‌گام؛ بخش‌های ۱، ۲ و ۸ (مقدمه، کار مرتبط، جمع‌بندی) نوشته شوند.

## ۵. موانعی که فقط پارسا باز می‌کند
- اعتبار برای یکی از کلیدهای Baseten (یا هر endpoint سازگار OpenAI).
- annotator دوم برای gold مستقل.
- ری‌استارت اپ Claude: MCP دسکتاپ هنوز پروسه‌های ۸ اکتبر (v1.1.0) را اجرا می‌کند.
