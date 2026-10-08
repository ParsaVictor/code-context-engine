# نقشه‌ی session 17 (۲۰۲۶-۱۰-۰۸) — از «بهتر از BM25» تا «قابل دفاع در مقاله»

ورودی: handoff session 16، `docs/research/contributions-log.md`، جدول ۴ مقاله‌ی LocAgent (arXiv 2503.09089).

## وضعیت واقعی در شروع (اندازه‌گیری‌شده)

| | hit@1 | hit@3 | hit@5 | منبع |
|---|---|---|---|---|
| ما (v1.1+issue mode)، Lite test، ۲۴۷ issue بدون خطا | 0.344 | 0.506 | 0.534 | `swebench/results-new-*.jsonl` |
| BM25 ساده‌ی ما، همان issueها | 0.295 | 0.506 | 0.578 | همان |
| BM25 در LocAgent (Lite) | 0.387 | 0.518 | 0.617 | جدول ۴ |
| Jina-Code-v2 (embedding) | 0.434 | 0.712 | 0.803 | جدول ۴ |
| CodeRankEmbed | 0.526 | 0.777 | 0.847 | جدول ۴ |
| Agentless + Claude-3.5 | 0.726 | 0.792 | 0.796 | جدول ۴ |
| LocAgent + Claude-3.5 | 0.777 | 0.920 | 0.942 | جدول ۴ |

نتیجه‌ی صادقانه: روی issueهای واقعی هنوز زیر retrieverهای embedding هستیم؛ در ۴۶٪ موارد فایل درست اصلاً در packet نیست.

## اصل روش (برای مقاله)

- **dev** = SWE-bench *dev split* (۲۲۵ issue، ۶ ریپو: pvlib، pydicom، sqlfluff، astroid، pyvista، marshmallow) — هیچ اشتراکی با Lite test ندارد. همه‌ی تنظیم‌ها فقط روی این.
- **holdout** = SWE-bench Lite test (۳۰۰). عدد قبلی (بالا) baseline است؛ بعد از تنظیم روی dev، **یک بار** اجرا می‌شود.
- ۲۴ issue ریپوهای کوچک Lite (flask/requests/…) که session 16 رویشان تنظیم کرد = dev-class؛ در جدول holdout جدا گزارش می‌شوند.
- هر ایده‌ی برداشته‌شده از رقبا اول روی dev با عدد سنجیده می‌شود.

## فازها

| فاز | کار | معیار پذیرش |
|---|---|---|
| P0 | merge PR #131؛ تکمیل ۳۹ issue خطادار test split | ✅ merge؛ retry در جریان |
| P1 | سرعت ایندکس (finalize_links، scan) | django سرد ≥ ۵× سریع‌تر؛ ۹ ست بدون تغییر |
| P2 | harness پژوهشی با امتیاز هر روش (bm25 / dense / ours) برای fusion آفلاین | روی dev اجرا شود |
| P3 | رتبه‌بندی issue: BM25F با tf واقعی، مسیرهای traceback، dense chunk (اگر سرعت اجازه دهد)، fusion، گسترش گراف (ایده‌ی LocAgent) | dev: hit@5 ≥ BM25 + ۱۰ واحد |
| P4 | اجرای یک‌باره روی Lite test با باینری نهایی | جدول مقاله + CI بوت‌استرپ |
| P5 | reranker v2 روی concept-holdout | فقط اگر recall نیفتد |
| P6 | انتشار v1.2.0 | CHANGELOG، measured.md، tag، ۳ باینری، MCP دسکتاپ |
| P7 | contributions-log §8 و جدول‌های مقاله | هر عدد با منبع |
