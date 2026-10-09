# نقشه‌ی راه تا مقاله (۲۰۲۶-۱۰-۰۹)

هدف: یک سیستم localization محلی (CPU، متن‌باز) که در Acc@k با روش‌های LLM-محور رقابت کند، با
روش holdout صادقانه و جدول ablation کامل. هر عدد از `docs/research/contributions-log.md`.

## وضعیت (اندازه‌گیری‌شده)

| بنچمارک | ما | بهترین بدون LLM منتشرشده | بهترین با LLM |
|---|---|---|---|
| SWE-bench Lite (۲۷۶ holdout) Acc@1/3/5 | 0.486 / 0.710 / 0.750 (CPU، 2.8s) | CodeRankEmbed 0.526 / 0.777 / 0.847 (GPU) | LocAgent+Claude 0.777 / 0.920 / 0.942 |
| SWE-bench dev (۲۲۵) Acc@5 | 0.600 | — | — |
| پرسش زبان ساده، ۴ holdout، R@3 | 0.500 / 0.917 / 0.875 / 0.833 | BM25 0.500 / 0.750 / 0.875 / 0.583 | — |

## الهام از رقبا — چه برداشتیم، چه می‌ماند

| رقیب | ایده‌ی کلیدی | وضعیت نزد ما |
|---|---|---|
| LocAgent | BM25 روی محتوای entity (تابع/کلاس) | ✅ `chunk_rank` — +0.17 Acc@5 روی dev |
| LocAgent | پیمایش گراف با تصمیم LLM | ❌ رأی کور گراف رد شد؛ فقط با LLM (فاز D2) |
| Agentless | skeleton فشرده‌ی فایل‌ها | ✅ fold/skeleton در packet |
| Agentless | انتخاب سلسله‌مراتبی فایل → تابع → خط با LLM | ⏳ فاز D1 |
| Agentless | چند نمونه + رأی‌گیری (sampling + voting) | ⏳ فاز D3 |
| CodeRankEmbed / Jina | بازیابی چگال کد | ❌ روی CPU غیرعملی (۲.۶ chunk/s)؛ فقط روی short-list (فاز D4) |
| Continue / Cody | reranker پس از بازیابی | ❌ روی holdout تازه ضرر (cobra 0.792 → 0.708) |

## فازها (به ترتیب، با معیار پذیرش)

| فاز | کار | ورودی | معیار پذیرش | موازی‌پذیر |
|---|---|---|---|---|
| **A** ✅ | ترکیب لیست برای پرسش ساده، holdoutهای ۳ و ۴، اصلاح باگ ارزیابی | — | بدون افت در ۱۴ ست | — |
| **B** | Verified (۵۰۰) با v1.2.0، یک‌باره | `verified.json` | جدول با CI | ✅ (IO) |
| **C** | ablation روی Lite holdout: بدون بهداشت متن / بدون chunk_rank / فقط packet | ۳ باینری | هر جزء سهم مثبت جدا | ✅ |
| **D1** | مرحله‌ی LLM سبک Agentless روی ۱۵ نامزد + skeleton، مدل محلی Qwen2.5-Coder-3B (llama.cpp، CPU) | `llm_localize.py` | dev-fast: Acc@1 ≥ +0.10 | پس از دانلود مدل |
| **D2** | سبک LocAgent: LLM یک بار گسترش گراف را برای ۳ نامزد برتر می‌خواهد (callers/imports) | MCP trace | dev-fast: Acc@5 ≥ +0.05 | — |
| **D3** | چند نمونه با دمای > 0 و رأی (Agentless) | — | پایداری + Acc@1 | — |
| **D4** | embedding فقط روی short-list (۲۰ فایل) — dense بدون هزینه‌ی کل ریپو | jina-code | dev-fast: Acc@5 | — |
| **E** | holdout نهایی Lite با بهترین ترکیب، یک‌باره؛ توکن در هر issue در برابر Agentless | — | جدول اصلی مقاله | — |
| **F** | مقایسه‌ی رودررو روی همان ۲۷۴ instance مقاله‌ی LocAgent | لیست instanceها | — | — |
| **G** | holdout ≥ ۵۰ پرسش + annotator دوم | ⚠️ تیم | توافق بین annotatorها | — |
| **H** | پیش‌نویس مقاله | log | — | ✅ همیشه |

## موانعی که فقط پارسا می‌تواند باز کند

- کلید API (مثلاً Baseten که در فاز C قبلی کار کرد) برای مقایسه‌ی LLM قوی در D1/E.
- یک نفر دوم برای gold مستقل (فاز G).
