# راهنمای session 18 — از v1.3.0 تا مقاله‌ی قابل ارسال

> این سند را اول بخوان. بعد `docs/research/contributions-log.md` (§8 کامل) و `docs/research/paper-draft.md`.
> هر عدد در این سند اندازه‌گیری شده و منبعش در contributions-log است.

## ۱. وضعیت دقیق در پایان session 17 (۲۰۲۶-۱۰-۰۹)

**نسخه‌ها:** v1.2.0 و **v1.3.0** منتشر شدند (tag روی `6e94b91`). v1.3.0 در
`%LOCALAPPDATA%\Programs\neuromesh\neuromesh.exe` نصب است (پشتیبان: `neuromesh-v1.2.0-backup.exe`).
MCP دسکتاپ فقط بعد از ری‌استارت اپ Claude نسخه‌ی جدید را بار می‌کند — اول با `neuromesh_get_stats`
یا initialize نسخه را چک کن.

**اعداد اصلی (file-level Acc@k = همه‌ی فایل‌های gold در top-k):**

| بنچمارک | نسخه | Acc@1 | Acc@3 | Acc@5 | Acc@10 | BM25 همان مجموعه |
|---|---|---|---|---|---|---|
| Lite holdout سخت (۲۷۶) | v1.2.0 | 0.486 | 0.710 | 0.750 | 0.804 | 0.301 / 0.507 / 0.587 / 0.721 |
| زیرمجموعه‌ی دقیق LocAgent (۲۷۴) | v1.2.0 | 0.474 | 0.708 | 0.752 | — | 0.299 / 0.522 / 0.606 |
| Verified (۴۹۶/۵۰۰) | v1.2.0 | 0.405 | 0.669 | 0.732 | 0.804 | 0.216 / 0.391 / 0.490 / 0.641 |
| SWE dev (۲۲۵، تنظیم) | v1.3.0 | 0.308 | 0.527 | 0.621 | 0.692 | 0.160 / 0.347 / 0.427 / 0.538 |
| SWE dev سطح تابع (۱۸۹) | v1.3.0 | 0.175 | — | 0.323 | 0.402 | — |

**رقبا (LocAgent Table 4، Lite ۲۷۴):** Jina-Code-v2 0.434/0.712/0.803 · CodeRankEmbed 0.526/0.777/0.847 ·
Agentless+GPT-4o 0.672/0.745/0.745 · Agentless+Claude-3.5 0.726/0.792/0.796 · LocAgent+Qwen7B(ft)
0.708/0.847/0.883 · LocAgent+Claude-3.5 0.777/0.920/0.942. سطح تابع Acc@5/@10: BM25 0.318/0.369 ·
CodeRankEmbed 0.518/0.588 · Agentless+Claude 0.588 · LocAgent+Claude 0.734/0.774.

**Ablation (Lite ۲۷۶، v1.2.0):** بدون chunk_rank 0.388/0.569/0.652 · بدون بهداشت متن 0.467/0.707/0.743 ·
فقط ترتیب packet 0.366/0.536/0.558.

**پرسش زبان ساده (R@3، ۴ holdout):** ripgrep 0.500 (BM25 0.500) · click 0.917 (0.750) · cobra 0.875
(0.875) · axios تازه 0.833 (0.583). axios آخرین holdout دست‌نخورده بود؛ دیگر دیده شده.

**در حال اجرا هنگام پایان session (resumable):** v1.3.0 یک‌باره روی Lite و Verified:
`swebench/lite-v130-{0,1,2}.jsonl` و `swebench/verified-v130-{0,1,2}.jsonl` (باینری
`…/b137b0ea…/scratchpad/nm-v130.exe`، work dir `C:/w/l*`, `C:/w/v*`). اگر ناقص‌اند با همان دستور ادامه بده:

```bash
cd "/c/1/1_پروژه/5_neuromesh/swebench"
S=<scratchpad>; cp ../repo/target/release/neuromesh.exe $S/nm-v130.exe   # از tag v1.3.0 بساز
for i in 0 1 2; do nohup py -3 ../repo/scripts/swebench_localize.py --data lite.json --repos repos \
  --bin "$S/nm-v130.exe" --out lite-v130-$i.jsonl --shard $i/3 --work C:/w/l$i > lite-v130-$i.log 2>&1 & done
# Verified: همان با --data verified.json --out verified-v130-$i.jsonl --work C:/w/v$i
```

## ۲. قواعد ثابت (نقض نشود)

1. **dev فقط** = SWE-bench dev split (`swebench/devset/swe-dev.json`، ۲۲۵) و `dev-fast.json` (۵۹). Lite و
   Verified = holdout، فقط با باینری منتشرشده و یک بار برای هر نسخه. هیچ‌وقت per-instance روی holdout نگاه نکن.
2. هر ایده: **اول prototype پایتونی آفلاین** روی نتایج ذخیره‌شده‌ی dev (`scripts/research/*.py`)، بعد Rust،
   بعد تأیید engine روی dev-fast، بعد ۱۴ ست (`scripts/benchmark-fast.sh` + concept-holdout3/4)، بعد PR.
3. هر نتیجه (مثبت یا منفی) با عدد در `contributions-log.md` ثبت شود؛ ایده‌های رد‌شده هم.
4. ادعای پرسش ساده فقط با یک holdout **تازه** که gold آن قبل از اجرا commit شده.
5. یک build در هر لحظه؛ هرگز حین build سورس را ویرایش نکن؛ `df -h /c` قبل از کار (target/debug > 10GB → پاک).
6. تست MCP: همیشه `NEUROMESH_NO_BROWSER=1`، از طریق Python subprocess (نه Git Bash برای مسیر `\\?\`).
7. harness: work dir کوتاه `C:/w/…` (MAX_PATH)، sharding با hash (پیش‌فرض)، هرگز دو اجرا روی یک فایل خروجی.
8. `README.md` و `nm.config.json` تغییرات session دیگری هستند — commit نکن؛ فایل‌ها را با نام add کن.
9. dogfood: فقط main ادغام‌شده + بنچمارک‌شده (ترجیحاً tag) نصب شود، با پشتیبان نسخه‌ی قبل.
10. گزارش کوتاه جدولی فارسی بعد از هر فاز؛ موانع واقعی (کلید API، annotator) را صریح بگو.

## ۳. ابزارهای آماده

| ابزار | کار |
|---|---|
| `scripts/swebench_localize.py` | اجرای موتور روی SWE-bench (loc_files, definitions, hits) |
| `scripts/research/eval_loc.py` | Acc@k فایل، `--ci` بوت‌استرپ، `--subset`، `--fuse` |
| `scripts/research/func_eval.py`, `func_hier.py` | Acc@k سطح تابع و ترتیب‌های سلسله‌مراتبی |
| `scripts/research/lex_variants.py`, `mention_prior.py`, `error_grep_prior.py` | prototypeهای لکسیکال/prior |
| `scripts/research/locagent_subset.py` | ساخت زیرمجموعه‌ی ۲۷۴ LocAgent (`swebench/devset/locagent-274.json`) |
| `scripts/research/llm_localize.py` (+`llm_server.sh`) | مرحله‌ی LLM روی هر endpoint سازگار با OpenAI (`--shuffle`) |
| `scripts/compare_baselines.py` | BM25/Aider روی ست‌های gold |
| `NM_ABLATE=hyg|chunk` | فقط در باینری ablation محلی (commit نشده) |
| `NM_DEFINITIONS=N` | عمق لیست definitions در `packet --json` |

## ۴. فازهای session 18 (به ترتیب، با معیار پذیرش)

موازی‌سازی: همیشه چهار مسیر همزمان — (الف) اجرای harness در پس‌زمینه، (ب) کدنویسی/build موتور،
(ج) prototype آفلاین پایتون روی نتایج ذخیره‌شده (بدون CPU سنگین)، (د) نوشتن مقاله.

### فاز 0 — بررسی وضعیت (≤۱۵ دقیقه)
- اجراهای v1.3.0 (Lite/Verified) را چک کن؛ اگر کامل‌اند، جدول نهایی فایل + تابع (روی ۲۷۶ سخت، ۲۷۴ LocAgent،
  و ۴۰۳ Verified غیر-Lite) با CI بوت‌استرپ بساز و در measured.md / contributions-log / paper-draft ثبت کن.
- نسخه‌ی MCP زنده (باید 1.3.0 باشد) و باینری‌های release v1.3.0 (سه فایل) را چک کن.

### فاز 1 — prior‌های «بدون LLM» الهام‌گرفته از ادبیات bug localization (بیشترین سود/هزینه)
هر کدام prototype آفلاین روی dev (۲۲۵)، با هر دو نیمه‌ی dev-fast / dev-rest سازگار باشد:
1. **Traceback → تابع**: framesها نام تابع دارند (`File "x.py", line N, in method`) و شماره‌ی خط → تابع محاط
   در base commit. رأی قوی برای سطح تابع (هدف: func Acc@5 0.323 → ≥0.40).
2. **تاریخچه‌ی commitها (BugLocator/Locus)**: commitهای قبل از base_commit که پیامشان با issue شبیه است
   (BM25 روی `git log` پیام‌ها) → فایل‌هایی که تغییر دادند رأی می‌دهند. فقط تاریخچه‌ی ≤ base_commit (بدون نشت).
3. **کد repro داخل issue**: identifierهای فراخوانی‌شده در بلوک‌های کد گزارش → definition همان‌ها (API تحت تست).
4. **Pseudo-relevance feedback (RM3)** روی chunk_rank: واژه‌های برجسته‌ی ۳ definition برتر → گسترش query.
5. **پیوند تست → ماژول**: نام تست/فایل تست در issue → importهای آن تست در گراف → ماژول تحت تست.
6. **dense فقط روی short-list**: embedding jina-code فقط برای ۲۰–۳۰ definition برتر (نه کل ریپو؛ CPU ۲.۶ chunk/s).
معیار: هر prior که dev Acc@1 یا func Acc@5 را ≥ +0.02 در هر دو نیمه بالا ببرد نگه داشته شود.

### فاز 2 — مرحله‌ی LLM قوی (Agentless/LocAgent) — نیازمند کلید API از پارسا
- اگر کلید هست (هر endpoint سازگار OpenAI؛ Baseten قبلاً کار کرده): `llm_localize.py` روی dev-fast با
  `where_to_look` + skeleton (سبک Agentless، یک فراخوانی) → سپس مرحله‌ی دوم تابع از `functions_to_look`.
- سبک LocAgent: حلقه‌ی کوتاه agent (≤۳ گام) که ابزارهای MCP خود ما (trace / dependencies / expand_gap) را
  صدا می‌زند. معیار: Acc@1 و توکن به ازای issue در برابر Agentless (توکن‌های ما ~۲.۲k در هر فراخوانی).
- اگر کلید نیست: این فاز را رد کن و صریح گزارش بده؛ مدل محلی ۳B قبلاً رد شده (0.200 → 0.067).

### فاز 3 — پرسش زبان ساده
- ripgrep هنوز = BM25 (0.500). ایده‌ها: وزن فیلد comment، پیوند doc/README → definition، synonymهای حوزه.
- یک **holdout تازه‌ی پنجم** (TypeScript یا Java، ۱۲ سؤال، gold قبل از اجرا commit) برای تأیید S1 و هر ایده‌ی جدید.

### فاز 4 — انتشار v1.4.0 + dogfood
- فقط آنچه در فاز ۱–۳ با عدد برنده شد. ۱۴ ست + Lite/Verified یک‌باره با باینری نهایی. نصب با پشتیبان.

### فاز 5 — مقاله
- `paper-draft.md` را کامل کن: جدول اصلی (فایل + تابع، Lite ۲۷۴/۲۷۶ و Verified)، ablation، هزینه
  (CPU، ثانیه، توکن)، نتایج منفی، تهدیدهای اعتبار. یک اسکریپت `scripts/research/paper_tables.py` که همه‌ی
  جدول‌ها را از فایل‌های نتیجه بسازد (تکرارپذیری).
- مسیر انتشار: workshop/industry (LLM4Code، ICSE SEIP، FSE industry) الان؛ main track بعد از فاز ۲ و G
  (holdout ≥۵۰ سؤال با annotator دوم).

## ۵. موانعی که فقط پارسا باز می‌کند
- کلید API برای فاز ۲ (بدون آن مقایسه با Agentless/LocAgent فقط «بدون LLM» می‌ماند).
- یک نفر دوم برای gold مستقل (توافق بین annotatorها).
- ری‌استارت اپ Claude برای بار کردن MCP v1.3.0.
