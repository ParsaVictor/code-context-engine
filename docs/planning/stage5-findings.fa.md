# فاز ۵ — یافته‌ها (زنده)

طبق اصل holdout (`docs/planning/05-phases-holdout-to-release.fa.md`): این یافته‌ها فقط **ثبت** می‌شوند، فیکس نمی‌شوند —
مگر با تصمیم صریح که آن ریپو از holdout به dev منتقل شود. شماره‌ی بعدی بعد از این فایل: **F40**.

## عدد اندازه‌گیری‌شده (۵a، ۱۳ سپتامبر ۲۰۲۶، commit `f80b854`)

`bash scripts/fetch-third-party.sh tests/third_party/holdout/repos.toml holdout && NM_THIRD_PARTY=1 cargo test -p neuromesh-context --test third_party_holdout_gold -- --nocapture`

| معیار | dev (۴ ریپو، مرحله ۴) | **holdout (django + ultralytics)** | گیت فاز ۵a |
|---|---|---|---|
| recall میانگین | 1.000 | **0.950** | ≥0.90 ✓ |
| precision میانگین | 0.856 | **0.146** | ≥0.60 ✗ |
| فایل ممنوع (۲۰ گلد‌کیس) | 0 | **4** | 0 ✗ |
| oracle قابل‌دسترس | 21/21 | 15/20 | — |
| oracle strict | 20 | 9/20 | — |

**نتیجه: عدد precision روی holdout (0.146) دقیقاً همان عدد شروع پروژه قبل از مرحله ۴ است.** ۱۲ قاعده‌ی مرحله ۴
عملاً هیچ اثر مثبتی روی این دو ریپو ندارند — سیگنال overfit واضح طبق تعریف فاز ۵a (پایین‌تر از 0.40).
طبق قانون فاز ۵: هیچ فیکسی روی مرحله ۴/dev شروع نشد؛ فقط ریشه‌یابی و ثبت.

### نکته‌ی صداقت درباره‌ی خود گیت — ۲ از ۴ forbidden hit، اشتباه گلد است نه موتور

بعد از مرج‌شدن گلد (طبق اصل قفل‌بودن، گلد را عوض نکردم)، با خواندن import واقعی کد معلوم شد:

- `django_migrations`: `recorder.py` را forbidden گذاشته بودم، ولی `executor.py::apply_migration` واقعاً
  `self.recorder.record_applied(...)` را صدا می‌زند (`from .recorder import MigrationRecorder`، خط ۶ و ۲۷۵ فایل).
  یعنی این فایل واقعاً بخشی از جواب درست است — گلد من اشتباه بود، نه seed موتور.
- `ultra_detection_loss`: `metrics.py` را forbidden گذاشته بودم، ولی `loss.py` مستقیم
  `from ultralytics.utils.metrics import CITYSCAPES_WEIGHT, OKS_SIGMA, RLE_WEIGHT` و
  `from .metrics import bbox_iou, probiou` می‌کند — دوباره اشتباه گلد من.

یعنی از ۴ forbidden gate-failure واقعی، فقط **۲ تا واقعاً نقص موتور کاندید است** (زیر بررسی شدند، F33/F34).
این دو مورد به‌عنوان کاستی روش (نه موتور) همین‌جا ثبت می‌شود، گلد لاک‌شده تغییر نمی‌کند.

## یافته‌ها

| # | یافته | کجا (کاندید، هنوز اثبات‌نشده با mutation) | probe |
|---|---|---|---|
| F33 ✅ | پرسش‌های شکل `Owner.__call__` (متد dunder به‌جای عضو معمولی): مسیر owner-qualified که F28 برای `req.get`/`res.json` ساخت (`activator::resolve_dotted_member`، seed از `query.split_once('.')` وقتی کوئری یک‌تکه‌ی `"owner.member"` باشد) اینجا هرگز صدا زده نمی‌شود — چیزی قبل از آن، توکنایزر query را به دو seed جدا می‌شکند: `identifier:WSGIHandler` (درست) و `identifier:__call__` (بدون owner، سراسری). `identifier:__call__` با اولین `__call__` دیگری که در گراف پیدا می‌شود match می‌کند (`ASGIHandler.__call__`، @1.00 L1_exact) نه با `WSGIHandler.__call__` که واقعاً پرسیده شده. نتیجه در `django_wsgi_entry`: `asgi.py` forbidden وارد packet شد. این خواهر مستقیم F28 است ولی روی زبانی که متد با `__` شروع می‌شود (Python dunder)، نه روی `owner.member` جاوااسکریپتی | seed query tokenization (قبل از `activator.rs::resolve_dotted_member`؛ منبع دقیق توکن‌سازی pinpoint نشده — کار بعدی) | `NM_PROBE="How does WSGIHandler.__call__ turn a raw WSGI environ into a Django response?"` روی django holdout: `seed identifier:__call__ -> sym:django/core/handlers/asgi.py:ASGIHandler.__call__ @1.00 L1_exact` |
| F34 ✅ | کلمه‌ی عمومی/نام محصول در prompt با هر symbol هم‌نام در کل ریپو (بدون در نظر گرفتن ربط دامنه) match می‌شود: کلمه‌ی «Django» در متن پرسش (توصیف چارچوب، نه اسم کلاس هدف) با `OGRGeomType.django` (attribute کوچک‌حرفی در ماژول GIS کاملاً بی‌ربط) `@1.00 L1_exact` match شد. همچنین در پرسش CSRF، `concept:next` با نود یک آیکون SVG (`calendar-icons.svg:next`) و `concept:middleware` با یک فایل تست فیکسچر (`tests/middleware_exceptions/middleware.py`) match شدند — هر دو فقط چون هم‌نامِ کلمه‌ی عمومی prompt بودند، نه چون مرتبط بودند | seed resolution عمومی (concept/identifier match بدون وزن‌دهی به ربط فایل) | probe CSRF: `seed concept:next -> sym:.../calendar-icons.svg:next @1.00`, `seed concept:middleware -> sym:tests/middleware_exceptions/middleware.py:middleware @1.00`; probe wsgi: `seed identifier:Django -> sym:.../geomtype.py:OGRGeomType.django @1.00` |
| F35 | یک لیست seed چندزبانه/synonym گسترده برای مفاهیم auth/middleware/token همیشه امتحان می‌شود، حتی وقتی هیچ‌کدام در prompt یا در ریپو معنی ندارند: در probe CSRF این seedها امتحان شدند و همه miss کردند (`bearer`, `app.use`, `validateToken`, `verifyJwt`, `araKatman`, `ara_katman`, `ara katman`, `zwischen`, `middlewares`, `pipeline`, `next()`) — روی dev repos (که یکیشان express است) بعضی از این‌ها واقعاً match دارند (`app.use`)، ولی این یعنی لیست برای واژگان dev-repoها ساخته شده، نه یک مکانیزم عمومی. بی‌ضرر وقتی miss می‌کنند؛ خطرناک وقتی (نادر) به یک symbol هم‌نام تصادفی برخورد کنند — دقیقاً منشأ F34 | seed concept-expansion (فایل دقیق pinpoint نشده) | همان probe CSRF بالا؛ خط `seed ara katman -> - @0.00` و مشابه |
| F36 — ریشه‌یابی‌شده + یک تلاش فیکس، هنوز مرج‌نشده | ریشه: `activator.rs:453-466` هر کلمه‌ی ≥۵ حرفی **متن خام prompt** را (نه فقط identifierهای واقعی) به‌عنوان `focus_term` ثبت می‌کند؛ سه جای جدا در `selector.rs` (خط ۲۷۳، ۴۹۲، و داخل `inject_learned_candidates` خط ۷۳۶) برای **هر** focus_term یک `graph.resolve_ranked(term, None, None)` مستقل بدون هیچ محدودیت می‌زنند و هر match را (وزن ۱۵ تا ۳۶) به packet اضافه می‌کنند. **تلاش فیکس (۱۴ سپتامبر):** یک مجموعه‌ی دوم و سخت‌گیرتر (`strong_focus_terms` — فقط `signature.identifiers` + file hint stems، بدون کلمات خام prompt) ساخته و به هر سه حلقه پاس داده شد (پارامتر جدید در `select()`). نتیجه: **dev-4 کاملاً بدون تغییر (۰.۸۷۳، تمیز)**؛ holdout-2 تقریباً بدون تغییر (۰.۲۶۹→۰.۲۷۰)؛ ولی **large مختلط**: `django_url_resolve` بهتر شد (۰.۵۰→۱.۰۰)، `django_csrf` کمی بهتر (۰.۱۲→۰.۱۴)، اما `django_signal` بدتر شد (۱.۰۰→۰.۵۰ — فایل واقعاً مرتبط `django/db/models/signals.py` اضافه شد که در گلد ما نبود)، `django_wsgi_entry` کمی بدتر (۰.۵۰→۰.۳۳)، و یک forbidden جدید در یک تسک که از قبل هم خراب بود (`django_template_render`، از قبل recall=0 به‌خاطر یک باگ فولد جدا؛ حالا `context.py` هم اضافه شد). خالص: precision large ۰.۲۶۹→۰.۲۶۲ (افت کوچک)، forbidden ۱→۲. طبق قانون ratchet، **برگردانده شد، مرج نشد** | `activator.rs:453` و `selector.rs:273,492,736` | dev-4 تمیز بود که نشان می‌دهد جهت فیکس درست است؛ علت دقیق افزایش `signals.py` در django_signal بررسی نشد (کار بعدی: probe مستقیم آن کیس با و بدون فیکس) |

## ۵c — مقیاس/سرعت (۱۳ سپتامبر ۲۰۲۶، ultralytics، binary release بدون embeddings — همان engine=fast پایه)

یک نکته‌ی روش‌شناسی قبل از عدد: اولین تلاش برای اندازه‌گیری از طریق spawn مستقیم پردازش با PowerShell
(`System.Diagnostics.Process`) روی ultralytics **۳۳ دقیقه** طول کشید با CPU تقریباً صفر — یعنی گیر کرده بود،
نه کند. همان دستور از طریق Bash (همان روشی که کل این session با آن کار کرده) **۱۱٫۷ ثانیه** طول کشید.
این یک باگ موتور نیست؛ به احتمال زیاد تعامل بین spawn مستقیم PowerShell و لایه‌ی sandbox این محیط بود.
ثبت شد تا در session بعدی دوباره وقت تلف نشود؛ عدد نهایی زیر از اجرای Bash (تمیز) است.

نکته‌ی دوم: وقتی زمان‌بندی را هم‌زمان با یک poll-loop سنگین (نمونه‌گیری حافظه هر ۱۰۰ms) اندازه گرفتم، عدد
latency تقریباً **دو برابر** شد (index 2142ms → 4814ms). یعنی خودِ اندازه‌گیری حافظه با polling باعث اختلال
در زمان شد. عدد latency زیر از اجرای بدون polling است؛ عدد حافظه از اجرای با polling (که خودش تحت تأثیر
polling قرار نمی‌گیرد چون memory-footprint نه CPU-timing است).

| معیار | ultralytics (۹۳۱ فایل، ۲٫۵۹M توکن) | این پروژه (۴۷۹ فایل، ۱٫۰۲M توکن) | پایه‌ی v0.9.0 | گیت |
|---|---|---|---|---|
| زمان index | **2142 ms** | 1225 ms | — | اندازه‌گیری‌شده، بدون baseline بزرگ برای مقایسه (زیر را ببین) |
| حافظه‌ی peak | **~82 MB** | — | — | اندازه‌گیری‌شده، نگران‌کننده نیست |
| latency هر پرسش (balanced، سرتاسری: retrieve+fill+fold، نه فقط L1) | p50≈**280 ms**، p95≈**464 ms** (نمونه n=10، پراکنده) | — | — | این معیار "L1 p50/p95" داخلی نیست — پایین را ببین |
| `l1_p50_ms` / `l1_p95_ms` داخلی (۳ سلول fixture ثابت روی کد خودِ این پروژه) | — | **28 ms / 48 ms** | 12 ms / 26 ms (روی ~۲۰۰ فایل، commit نامشخص) | ≤50ms p95: **رد نشد** (فقط ۲ms فاصله) |
| prompt خصمانه (۲ MB) | — (روی این پروژه اندازه گرفته شد) | 24.8s, 25.1s (۲ اجرا) | < 30s | **رد نشد**، ولی با حاشیه‌ی کم؛ طبق یادداشت قبلی این گیت با بار ماشین ۱۶–۵۰s نوسان دارد |

### چرا مقایسه‌ی «≤ 1.5× پایه روی ریپوی بزرگ» انجام نشد

هیچ عدد baseline «ریپوی بزرگ» از نسخه‌ی v0.9.0 وجود ندارد — تنها baseline ثبت‌شده (12/26ms) روی یک fixture
~۲۰۰ فایلی این پروژه بود، نه یک ریپوی واقعی بزرگ. مقایسه‌ی latency سرتاسری ultralytics (280/464ms) با آن
عدد، مقایسه‌ی دو چیز متفاوت است (سرتاسری در برابر فقط لایه‌ی L1؛ ریپوی ۹۳۱ فایلی در برابر ۲۰۰ فایلی) و
گمراه‌کننده می‌بود. **این بخش گیت، اندازه‌گیری‌نشده باقی می‌ماند** — نه رد و نه قبول؛ نیاز به یک baseline
مشخص (کدام commit، کدام ریپو، کدام حالت) دارد که در این session پیدا نشد.

### جمع‌بندی ۵c

- عدد نگران‌کننده‌ای دیده نشد — index و حافظه روی ریپوی ۹۳۱ فایلی معقول‌اند.
- تنها نکته‌ی مرزی: گیت داخلی `l1_p95_slo` روی کد خودِ این پروژه با ۲ms فاصله pass می‌شود (48 در برابر 50) —
  حاشیه‌ی کمی دارد و با رشد بیشتر پروژه (که از ~۲۰۰ به ۴۷۹ فایل رسیده) ممکن است در آینده رد شود.
- مقایسه‌ی رسمی با baseline روی ریپوی بزرگ اندازه‌گیری‌نشده ماند (بالا را ببین) — صادقانه گزارش شد، حدس زده نشد.

## فاز B — F33 فیکس شد و اندازه‌گیری شد

تعمیم داده شد (نه قاعده‌ی جدید): در `extract_prompt_anchors` (`neuromesh-parser/src/identifiers.rs`)،
شاخه‌ی `Owner.member` با owner بزرگ‌حرف حالا اول جفت کامل `owner.member` را seed می‌کند (دقیقاً کاری که
شاخه‌ی lowercase برای `req.get` از قبل می‌کرد)، بعد نیمه‌های بی‌owner را. تست واحد اضافه شد
(`capitalised_owner_member_yields_qualified_pair`).

| مجموعه | قبل | بعد | تغییر |
|---|---|---|---|
| dev-4 | recall 1.000 / prec 0.856 / forbidden 0 / oracle 21/21 strict 20 | recall 1.000 / prec 0.856 / forbidden 0 / oracle 21/21 **strict 19** | precision بی‌تغییر؛ strict یک واحد افت (`nano_attention`: حالا seed دقیق‌تر `CausalSelfAttention.forward` است و `__init__` fold می‌شود — از خانواده‌ی F7/F30، نه رگرسیون F33) |
| large (django+ultralytics) | recall 0.950 / **prec 0.173** / forbidden 2 / oracle 17/20 strict 10 | recall 0.950 / **prec 0.257** / **forbidden 1** / oracle **18/20** strict 11 | `django_wsgi_entry` دقیقاً همان‌جا که probe شده بود: `asgi.py` دیگر forbidden نمی‌آید |
| holdout-2 (gin+vision) | prec 0.265 forbidden 1 | prec 0.258 forbidden 1 | تقریباً بی‌تغییر — درست هم همین انتظار می‌رفت: هیچ سؤال gin/vision از الگوی `Owner.__dunder__` نیست، F33 اینجا اثری نداشت |

ratchet در `third_party_large_gold.rs` بالا برده شد (precision 0.17→0.24، forbidden 2→1، reachable 0.85→0.89).
گیت holdout هنوز رد است (0.258 < 0.60) — طبق انتظار؛ F33 فقط یک الگو را حل می‌کرد، نه کل مشکل precision.

### F34 فیکس شد

در `extract_prompt_anchors`، شاخه‌ی سوم اسکن آزاد identifier (بدون هیچ سیگنال ساختاری مثل نقطه یا «how does»)
هر کلمه‌ی بزرگ‌حرف ۳+ حرفی را می‌گرفت — یعنی «Django» در جمله‌ی توصیفی همان‌قدر seed می‌شد که «ResNet» یا
«WSGIHandler». حالا این شاخه فقط PascalCase واقعاً چندقوزی (دو حرف بزرگ به بالا: `ResNet`, `BasicBlock`,
`WSGIHandler`) را می‌گیرد؛ کلمه‌ی تک‌قوزی (`Django`, `Signal`, `Python`) دیگر از این مسیر seed نمی‌شود —
ولی اگر واقعاً owner یک `.member` باشد یا سوژه‌ی «how does X» باشد (`Signal.connect`)، از همان دو مسیر دیگر
که قبلاً هم بودند seed می‌شود، بدون تغییر.

| مجموعه | قبل (بعد از F33) | بعد (F33+F34) |
|---|---|---|
| dev-4 | prec 0.856 forbidden 0 oracle 21/21 strict 19 | **prec 0.873** forbidden 0 oracle 21/21 strict 19 |
| large | prec 0.257 forbidden 1 oracle 18/20 strict 11 | **prec 0.269** forbidden 1 oracle 18/20 strict 11 |
| holdout-2 | prec 0.258 forbidden 1 oracle 19/20 strict 14 | **prec 0.269** forbidden 1 oracle 19/20 strict 14 |

هر سه مجموعه کمی بهتر شدند، هیچ‌کدام بدتر نشد. ratchet dev-4 هم بالا برده شد (0.85→0.86). گیت holdout هنوز رد
است (0.269 < 0.60) — طبق انتظار.

## فاز A (بازنگری پلن ۱۴ سپتامبر) — چرخش holdout، اعداد قبل از هر فیکس

Django + ultralytics → `tests/third_party/large/` (dev بزرگ، ratchet جدا در `third_party_large_gold.rs`)؛
دو گلد اشتباه (G1) اصلاح شد. holdout تازه: **gin** (Go) + **torchvision** — گلد با چک G1، PR #49 قبل از اجرا.

| مجموعه | recall | precision | forbidden | oracle reachable / strict |
|---|---|---|---|---|
| dev-4 (بدون تغییر) | 1.000 | 0.856 | 0 | 21/21 / 20 |
| large (django+ultralytics، گلد اصلاح‌شده) | 0.950 | **0.173** (0.146 قبل از اصلاح گلد) | 2 (asgi.py ← F33؛ trainer.py) | 17/20 / 10 |
| **holdout-2 (gin+vision)، اولین اجرا** | **1.000** | **0.265** | 1 (`voc.py` کنار `coco.py`) | 19/20 / 14 |

گیت holdout (precision ≥ 0.60) رد شد — انتظار می‌رفت؛ این عدد «قبل از B» است. recall روی هر سه مجموعه کامل یا نزدیک به کامل.

| # | یافته | probe؟ |
|---|---|---|
| F37 | فایل‌های test/example/gallery در packet سؤال توضیحی: `gin_test.go`, `githubapi_test.go`, `middleware_test.go`, `test/test_ops.py`, `gallery/others/*.py`, `references/*/presets.py` — در ۱۰ از ۲۰ کیس holdout-2 حداقل یکی هست. برای «این چطور کار می‌کند؟» فایل تست جواب نیست، مگر prompt خودش «test» بگوید | ✗ هنوز؛ بعد از F33 |

## F35 — کار شروع شد، فیکس نشد (regression زنجیره‌ای پیدا شد، عمداً برنگردانده)

تشخیص F35 درست بود و فیکس اولیه‌اش کار کرد (`alias.rs::expand_aliases` را طوری تغییر دادم که وقتی یک
cluster چندزبانه match می‌شود، فقط همان کلمه‌ای که واقعاً در prompt بود منتشر شود، نه همه‌ی هم‌خانواده‌هایش).
تنها با این فیکس: large 0.269→0.287 (forbidden 1→0!)، holdout 0.269 (بی‌تغییر)، **ولی dev-4 0.873→0.833 افت کرد**.

ریشه‌یابی نشان داد این افت خودش زنجیره‌ای از باگ‌های دیگر را باز کرد که تا حالا توسط نویز F35 پنهان بودند:

- **F38 (پیدا و فیکس شد، بعد با F35 با هم تست شد):** `query_intent.rs::classify_intent` با دیدن کلمه‌ی
  عمومی «database»/«model»/«query»/«repository» (بدون هیچ نشانه‌ی کد بودن) intent را `TraceQuery` حساب
  می‌کرد و یک بسته‌ی اصطلاحات مخصوص Express/Node (`req.query`, `parseurl`, `querystring`) تزریق می‌کرد —
  کاملاً بی‌ربط به یک سؤال FastAPI. قبلاً نویز F35 بودجه‌ی seed را پر می‌کرد و این تزریق را بیرون می‌راند
  (تصادفی پنهان مانده بود). فیکس این هم precision را کمی بالا برد ولی dev-4 هنوز پایین ماند (۰.۸۳۹).
- یک نگاشت مشابه در جدول جدای `ALIAS_CODE_SEEDS`: `("database", &["req.query", "query"])` — مفهومی
  کاملاً اشتباه (database به query string ربطی ندارد)، حذف شد؛ کمی بهتر شد (۰.۸۳۹، بدون تغییر واقعی روی
  همین کیس).
- **باقی‌مانده، پیدا شد ولی فیکس نشد:** بعد از هر دو فیکس بالا، `fastapi_settings` هنوز فقط ۰.۳۳ بود
  (به‌جای ۱.۰۰ قبل از هر تغییری). probe نشان داد `expand_file_seeds_to_symbols` (`activator.rs`) دارد
  `backend/app/models.py` و `backend/app/alembic/env.py` را با tier `hierarchical:file_expand` وارد
  می‌کند — یعنی یک لایه‌ی سوم، عمیق‌تر (چیزی «database» را به این فایل‌ها file-hint می‌زند، احتمالاً چون
  کلمه‌ی مفهومی «database» هنوز به‌عنوان یک related_concept بدون resolve باقی می‌ماند و یک fallback فایل-hint
  حدس می‌زند این‌ها فایل‌های «مرتبط با دیتابیس»اند). این لایه هم قبلاً توسط نویز F35 (که بودجه/آستانه‌ی
  escalation را جور دیگری پر می‌کرد) پنهان بوده.

**تصمیم:** به‌جای فیکس ناقص یا پایین‌آوردن ratchet dev-4 (که قانون صریح پروژه است — ratchet فقط بالا
می‌رود)، هر سه تغییر (`alias.rs` F35، `query_intent.rs` F38، حذف نگاشت database→query) **برگردانده شد**
و merge نشد. dev-4/large/holdout در همان نقطه‌ی F33+F34 ماندند (۰.۸۷۳ / ۰.۲۶۹ / ۰.۲۶۹) — تنها نقطه‌ای که
تا این لحظه به‌طور کامل تأیید شده بدون هیچ افتی.

**برای session بعد:** F35+F38 و ریشه‌ی سوم (`expand_file_seeds_to_symbols` / منشأ file-hint برای مفاهیم
resolve-نشده در `activator.rs`) باید با هم، در یک PR، حل شوند تا dev-4 واقعاً بدون افت بماند — تک‌تک
فیکس‌کردن‌شان هر بار یک لایه‌ی جدید را لو می‌دهد. شروع از همان‌جا: چرا `related_concepts` ای که resolve
نمی‌شود به یک file-hint heuristic می‌رسد، و آیا آن heuristic اصلاً باید برای کلمات مفهومی (نه کلمات
کد-مانند) فعال باشد.

## فاز B′ — F39: لایه‌ی document-frequency/idf (تلاش اول، نتیجه‌ی مختلط، برگردانده شد)

زمینه: تصمیم این session رفتن به فاز B′ بود (سند ۰۶) — یک سیگنال رتبه‌بندی idf به‌جای امتیاز ثابت در سه محل
seed injection. تصمیم گرفته شد به‌جای اضافه‌کردن `tantivy` از یک شمارنده‌ی دستی document-frequency شروع شود
(پیشنهاد Claude، تأیید Parsa): یک `TokenDfIndex` (memoized مثل `file_learning_boost_index`) که برای هر
identifier sub-token (از `token_to_nodes` موجود، بدون ساخت ایندکس جدید) تعداد فایل‌های متمایز حاوی آن را
می‌شمارد؛ `NeuralProjectGraph::term_idf_weight(term)` یک وزن در `[0.15, 1.15]` برمی‌گرداند (۱.۱۵ برای
identifierهای کمیاب مثل `WSGIHandler`، ۰.۱۵ برای کلمات رایج در کل ریپو). این وزن در هر سه محل ضرب شد:
`selector.rs:273` (15.0)، `selector.rs:492` (36.0)، و `inject_learned_candidates` (14.0 و 16.0) — بدون لمس
`resolve_ranked`/`pick_dominant_candidate` (که دست‌نخورده ماند، طبق طراحی).

| مجموعه | قبل (F33+F34) | بعد (+F39 idf) | نتیجه |
|---|---|---|---|
| dev-4 | prec 0.873 forbidden 0 oracle 21/21 strict 19 | **بدون تغییر**: prec 0.873 forbidden 0 oracle 21/21 strict 19 | تمیز |
| large (django+ultralytics) | prec 0.269 forbidden 1 oracle 18/20 strict 11 | prec **0.282** (بهتر) ولی forbidden **2** (بدتر — `ultra_validator_call` حالا `engine/trainer.py` را هم forbidden می‌آورد) | **ratchet test خودش panic کرد** (سقف مجاز forbidden=1) |
| holdout-2 (gin+vision) | prec 0.269 forbidden 1 oracle 19/20 strict 14 | prec **0.259** (کمی بدتر) forbidden بدون تغییر (۱) oracle بدون تغییر | کمی بدتر، نه بهتر |

**نتیجه: طبق قانون طلایی، مرج نشد — برگردانده شد.** dev-4 تمیز ماند (نشان می‌دهد جایگزینی امتیاز ثابت با
idf به‌تنهایی چیزی را در ریپوهای کوچک/dev خراب نمی‌کند)، ولی اثر روی large/holdout دقیقاً الگوی سه تلاش
قبلی امروز (F35/F38/F36) را تکرار کرد: بهبود کوچک precision با قیمت یک forbidden جدید یا افت کوچک جای دیگر.
وزن idf مبتنی بر «این token چند فایل را لمس می‌کند» چیزی را در مورد *ربط* match به سؤال نمی‌گوید — فقط
کمیابی آن را می‌گوید؛ یک match کاملاً بی‌ربط اگر کمیاب باشد (مثل `trainer.py` در ultralytics که واقعاً به
خیلی فایل‌ها می‌رود ولی از زاویه‌ی دیگری کمیاب محسوب شد) همچنان امتیاز کامل می‌گیرد.

**برای session بعد:** idf تنها، مشکل precision روی holdout/large را حل نمی‌کند. دو مسیر باقی مانده: (۱) F35
زنجیره‌ای (سه لایه‌ی به‌هم‌گره‌خورده، هنوز حل‌نشده، یادداشت کامل بالاتر در همین فایل) احتمالاً اثر بزرگ‌تری
دارد و اول باید حل شود؛ (۲) اگر بعد از حل F35 هنوز idf لازم بود، شاید باید به جای document-frequency ساده،
idf را با یک سیگنال *ربط* ترکیب کرد (هم‌پوشانی واژگانی واقعی seed↔prompt، نه فقط کمیابی token) — دقیقاً
چیزی که جدول فاز B′ در سند ۰۶ هم به‌عنوان «reranker» (نه فقط idf) نام برده بود. کد این تلاش در هیچ کامیتی
ذخیره نشده (در کارگزاری برگردانده شد، نه revert از طریق git)، پس اگر لازم شد دوباره باید نوشته شود —
طراحی‌اش (محل‌ها، امضای `term_idf_weight`، استفاده از `token_to_nodes` موجود) در همین یادداشت ثبت است.

## F40 — باگ case-sensitivity در resolver + صندلی اجباری callee‌ها (فیکس شد، هر سه مجموعه ≥ قبل)

زمینه: بعد از چهار تلاش mixed متوالی (F35/F38/F36/F39)، به‌جای heuristic بعدی، بازبینی داده‌ها: recall روی
holdout-2 در **هر ۲۰ تسک دقیقاً 1.00** است — یعنی مشکل rank نیست، تعداد فایل‌های اضافه است؛ و فایل‌های اضافه
hub هستند (`context.go` در ۱۰/۱۰ تسک gin، `engine/model.py` در ۹/۱۰ ultralytics). `packet_probe` با خروجی
جدید `NM_PROBE_EDGES=1` (edgeهای خروجی هر seed + شمار caller هدف) روی `gin_recovery` و `gin_serve_http`
دو علت ساختاری نشان داد:

**F40a — باگ صحت در resolver.** ایندکس `name_to_nodes`/`export_index` به حروف کوچک کلید می‌خورد و
`resolve_unique`/`resolve_call_target`/`resolve_export` با یک match یکتای case-insensitive، edge را **Proven**
می‌کردند. در Go (و هر زبان پشتیبانی‌شده، همه case-sensitive)، پارامتر `handle` در `CustomRecoveryWithWriter`
به `ginS.Handle` وصل شد و `engine.pool.Get()`/`Put()` به HTTP-verb wrapperهای `ginS.GET`/`PUT` — هر سه
Proven، هر سه غلط، و همه وارد packet. فیکس: تابع `case_narrowed`/`narrow_exact_case` در `graph.rs` — اگر
match با case دقیق وجود دارد فقط همان‌ها؛ اگر فقط match با case متفاوت هست، `resolve_unique` و
`resolve_call_target` رد می‌کنند و `resolve_ranked`/`resolve_export` نتیجه را به `Likely` تنزل می‌دهند
(همچنان حل می‌شود — seedهای prompt اغلب lowercase‌اند — ولی دیگر «قطعی» نیست).

**F40b — tier «۳ callee اجباری» سقف مکانیکی precision.** `selector.rs:165-237` تا ۳ فایلی را که seed به
آن‌ها call دارد *required* (امتیاز ۱۶، بالاتر از خود gold با ۸.۵) می‌کند و جای خالی را همیشه با callee بعدی
(به ترتیب الفبایی مسیر!) پر می‌کند. برای تسک تک‌فایلی، precision مکانیکی ≤۰.۲۵ — همان اعداد تکراری جدول
(0.20/0.25/0.33). تلاش اول («callee فقط اگر prompt نامش را برده») dev-4 را شکست (recall 1.000→0.958،
fastapi_login فایل `security.py` را که gold می‌خواهد ولی prompt نمی‌گوید گم کرد). داده‌ی جداکننده: شمار
callerِ هدف — callee‌های مطلوب fastapi (`create_access_token`=1، `Token`=1، `authenticate`=4) در مقابل
callee‌های نامطلوب gin (`cleanPath`=7، `IsDebugging`=7، `WriteHeaderNow`=16، `Context.Next`=25،
`Context.Set`=99، `Context.Get`=125). قاعده: callee با confidence غیر-Proven، یا با بیش از ۵ caller و
بدون اشاره‌ی prompt، صندلی اجباری نمی‌گیرد (در fill رتبه‌بندی‌شده با امتیاز ۱۲–۱۵ باقی می‌ماند — خط
۲۸۶–۳۳۶ قبلاً این کار را می‌کرد). ساختاری و بدون واژگان؛ روی هر سه مجموعه یکنواخت اثر دارد.

| مجموعه | قبل (main 62b4f8b) | بعد (F40a+b) | نتیجه |
|---|---|---|---|
| dev-4 | recall 1.000 prec 0.873 forbidden 0 oracle 21/21 strict 19 | **بدون تغییر** | تمیز |
| large (django+ultralytics) | recall 0.950 prec 0.269 forbidden 1 oracle 18/20 strict 11 | recall 0.950 prec 0.269 forbidden 1 oracle 18/20 strict **12** | برابر/بهتر |
| holdout-2 (gin+vision) | recall 1.000 prec 0.259 forbidden 1 oracle 19/20 strict 14 | recall 1.000 prec **0.424** forbidden 1 oracle 19/20 strict **15** | +0.165 |

دو نکته‌ی پیاده‌سازی که تست فیکسچر (`gold_harness_on_fixture_repos`، تسک `home_view_twig`) لو داد و بدون آن
CI قرمز می‌شد: (۱) narrowing باید فقط match‌هایی را که *صرفاً در case* فرق دارند حذف کند — کاندیدی که زیر
stem/alias ایندکس شده (`hello.twig` برای کلید `hello`) case-mismatch نیست؛ (۲) edge قالب overlay
(`index → hello.twig`، هدف از نوع File) عمداً `Likely` است و باید صندلی اجباری بگیرد، وگرنه slot آزادشده
را focus_term با امتیاز ۳۶ (F36) با `Greeter.php` forbidden پر می‌کند — یعنی precision فعلی dev تا حدی به
«اشغال slot توسط callee» تکیه دارد، نه به رتبه‌بندی درست؛ F36 همچنان باز است.

نتیجه‌ی میانی F40a تنها (بدون b): large 0.269→0.254، holdout 0.259→0.296 — mixed؛ علت: حذف edge غلط فقط جای
خالی tier را برای callee غلط بعدی آزاد می‌کرد. با هم (a+b) هر سه ≥ قبل — مرج شد.

**هنوز باز (از probe همین تسک‌ها):** در `gin_recovery` سه فایل باقی‌مانده‌ی نامطلوب (`context.go`،
`routergroup.go`، `githubapi_test.go`) همه از seedهای idiom F35 می‌آیند (`concept:next`، `alias_code:app.use`،
`client_expansion:route` → فایل تست). یک edge غلط دیگر با case درست: `handle` (پارامتر) → `RouterGroup.handle`
(متد hom-onym) — ابهام واقعی نام، با case حل نمی‌شود. large هنوز 0.269: احتمالاً سهم بزرگ‌تری از مسیرهای دیگر
(file_expand، focus_term F36) دارد — probe بعدی روی `ultra_predict_stream`/`django_csrf`.

## F41/F42 — «سؤال symbol خودش را نام برده؛ حدس‌زدن را متوقف کن» (سه لایه، یک PR، هر سه مجموعه بالا)

probe روی `ultra_augment_mosaic`/`ultra_predict_stream` (large) و `vision_roi_heads_training` (holdout) بعد از F40 یک
الگوی مشترک نشان داد: seedهای دقیق (`Mosaic` @1.00، `RoIHeads.select_training_samples` @1.00) درست حل می‌شوند و
بعد **سه مسیر حدسی مستقل** با کلمات انگلیسی همان prompt به فایل‌های بی‌ربط seed می‌زنند:

**F41 — `retrieval/artifact_seeds.rs`:** «training sample»/«inference»/«__call__» → *هر* TrainLoop/EvalLoop ریپو
seed اجباری @0.86 (`trainer.py:_do_train`، `optim/muon.py`، `torch_utils.py`، `exporter.py`، `validator.py`) —
منشأ hub بودن `engine/trainer.py` در ۹/۱۰ تسک ultralytics. خودِ ماژول برای سؤال‌هایی است که «هیچ symbolی را نام
نمی‌برند». فیکس: اگر identifier کدشکل (حرف بزرگ/`_`/`.`/`::` — نه کلمه‌ی «training») حل شده (`code_anchor`)،
کاندید kind-only که نه در prompt نام برده شده و نه عضو owner یک seed است، وارد نمی‌شود. «نام برده شده» برای
nodeهای بدون owner = در `identifiers` **یا** whole-word با case دقیق در متن prompt (فیکسچر `ml_train_loss`:
«the Detector model» — extractor «Detector» را identifier نمی‌شمرد ولی prompt واضحاً نامش را برده). سناریوی
nanoGPT که ماژول برایش ساخته شده دست‌نخورده می‌ماند.

**F41b — `activator.rs::cluster_terms_covered`:** prompt مرکب («چطور X … و چطور Y») به cluster می‌شکند؛ cluster
فقط وقتی covered بود که *همه‌ی* کلمات مهمش (شامل اسم‌های انگلیسی «proposals»، «result») seed شده باشند — وگرنه
هر اسم fuzzy جست‌وجو و با انرژی ۰.۸۵ seed می‌شد (`proposals` → `rpn.py:filter_proposals` @0.55 — forbidden
holdout؛ `result` → `ssd.py`، `feature_pyramid_network.py`). فیکس: cluster که یک identifier کدشکلِ حل‌شده دارد
covered است. باگ همراه: `seed_term_resolved` query را با پیشوند `reason:` (`identifier:fastrcnn_loss`) مقایسه
می‌کرد و هرگز برابر نمی‌شد — فقط از طریق `.member` تصادفاً match می‌کرد؛ پیشوند حالا جدا می‌شود.

**F42 — `seed/owner_cohere.rs` (جدید):** identifier بدون owner (`postprocess`) وقتی seed دیگری از همان سؤال به
owner‌ای با عضو هم‌نام حل شده (`BasePredictor`) به همان عضو re-point می‌شود (`sam/predict.py:Predictor.postprocess`
→ `predictor.py:BasePredictor.postprocess`). خانواده‌ی F28/F33، برای حالتی که نقطه در prompt نیست. strict
`ultra_predict_stream` که F41 تنها آن را می‌انداخت (`postprocess=Folded`) با این برمی‌گردد.

| مجموعه | قبل (main a035b01، بعد از F40) | بعد (F41+F41b+F42) |
|---|---|---|
| dev-4 | recall 1.000 prec 0.873 forbidden 0 oracle 21/21 strict 19 | recall 1.000 prec **0.906** forbidden 0 oracle 21/21 strict 19 |
| large | recall 0.950 prec 0.269 forbidden 1 oracle 18/20 strict 12 | recall 0.950 prec **0.387** forbidden 1 oracle 18/20 strict **13** |
| holdout-2 | recall 1.000 prec 0.424 forbidden 1 oracle 19/20 strict 15 | recall 1.000 prec **0.470** forbidden **0** oracle **20/20** strict **16** |

نتایج میانی که رد شدند (ثبت برای این‌که دوباره امتحان نشوند): F41 تنها → large 0.325 ولی strict 12→11 و holdout
0.424→0.409 با forbidden 1→2 (rpn.py) — mixed. **F43** (کف امتیاز ۱۲ برای fill اختیاری، آینه‌ی مسیر overflow) →
holdout 0.409→0.391، بدتر؛ برگردانده شد. فرضیه‌ی «slot آزادشده را fill پر می‌کند» درست بود ولی مکانیزم fill نبود،
cluster fuzzy بود (F41b). درس: probe کن، فرض نکن.

**هنوز باز:** large 0.387 و holdout 0.470 هر دو زیر هدف ۰.۶۰. باقی‌مانده‌ی probe‌ها: `gin_recovery` (idiom seedهای F35:
`concept:next`، `alias_code:app.use`، `client_expansion:route` → فایل تست)، `django_csrf`، `ultra_validator_call`
(forbidden `trainer.py` — از مسیر دیگری غیر از artifact seed). `django_template_render` همچنان recall 0 (باگ فولد جدا).

## F44 — هم‌نامِ عضو در فایل بی‌anchor (فیکس شد، large forbidden → 0)

probe `ultra_validator_call` بعد از F41/F42: `concept:validate` (از کلمه‌ی «validation» در prompt) به
`trainer.py:BaseTrainer.validate` @1.00 حل می‌شد — همان forbidden ماندگارِ large از فاز A. `weak_symbol_seed.rs`
(که seedهای WEAK را کنار anchor قوی هرس می‌کند) این را نگه می‌داشت چون قاعده‌ی «match دقیقِ نام همیشه می‌ماند»
داشت. اصلاح: match دقیق روی یک *عضو* (node با parent) وقتی owner در prompt نام برده نشده و فایل هیچ seed قوی‌ای
ندارد، هم‌نام است نه پاسخ — حذف می‌شود. (`configurator.py` + `concept:config` → `GPT.config` در تست واحد قدیمی
همین الگو بود؛ تست به قاعده‌ی جدید به‌روز شد.)

| مجموعه | قبل (main 4092755) | بعد (F44) |
|---|---|---|
| dev-4 | 1.000 / 0.906 / 0 / 21 strict 19 | بدون تغییر |
| large | 0.950 / 0.387 / 1 / 18 strict 13 | 0.950 / **0.473** / **0** / **19** strict **14** |
| holdout-2 | 1.000 / 0.470 / 0 / 20 strict 16 | 1.000 / **0.477** / 0 / 20 strict 16 |

ratchet large: precision 0.38→0.47، forbidden ≤1→**0**، reachable 0.89→0.94.

## F45 — `is_test_path` فقط دایرکتوری `tests/` را می‌شناخت (فیکس شد)

probe چهار تسک gin با precision 0.25: `githubapi_test.go` (از `client_expansion:route` → یک helper تست)،
`auth_test.go`/`validate_test.go`/`recovery_test.go` (fill، امتیاز ۲۴–۲۸). فیلتر نویز fill (`is_noise_node` →
`is_low_priority_source_path` → `is_test_path`) فقط `tests/`، `test/`، `_tests.rs` را می‌شناخت — نه `_test.go`،
`test_*.py`، `*.test.ts`/`*.spec.js`، `_test.rs`، `*Test.java`. یعنی روی Go/Python/JS همان چیزی که F37 گزارش
کرده بود (فایل تست در ۱۰/۲۰ کیس holdout) اصلاً فیلتر نمی‌شد. فیکس در `neuromesh-core/source_path.rs` + در
`weak_symbol_seed.rs` seed ضعیفی که به مسیر نویز می‌رسد کنار anchor قوی حذف می‌شود.

| مجموعه | قبل (main 0a1004f) | بعد (F45) |
|---|---|---|
| dev-4 | 1.000 / 0.906 / 0 / 21 strict 19 | بدون تغییر |
| large | 0.950 / 0.473 / 0 / 19 strict 14 | 0.950 / **0.504** / 0 / 19 strict 14 |
| holdout-2 | 1.000 / 0.477 / 0 / 20 strict 16 | 1.000 / **0.489** / 0 / 20 strict 16 |

per-task holdout: gin_recovery 0.20→0.50، gin_context_next 0.25→0.50، ولی gin_static 1.00→0.67 (یک فایل
جایگزین وارد شد — بررسی نشده؛ سطح مجموعه بالا رفت، ratchet سطح مجموعه است). ratchet large precision 0.47→0.50.

## F36′/F46/F47 — سه تلاش fill-tuning، هر سه رد شد؛ نتیجه‌گیری ساختاری

بعد از F40–F45 (پنج فیکس متوالی، همه «حذف seed/edge غلط»، هر سه مجموعه بالا)، probe‌های باقی‌مانده
(`vision_batched_nms` 0.20، `gin_logger` 0.25، …) همه یک شکل دارند: gold درست و تنها required است؛ ۳–۵ فایل
optional با امتیاز ۹–۲۴ از سیگنال‌های ضعیفِ *درست* (import، callee با caller زیاد، consumer، synaptic) کنارش
می‌نشینند. سه تلاش برای سخت‌گیرتر کردن optional:

| تلاش | قاعده | نتیجه | چرا رد شد |
|---|---|---|---|
| F36′ | focus_term انگلیسی وقتی identifier حل شده، امتیاز ۳۶ نگیرد (فقط حلقه‌ی ۳۶، با `strong_focus`) | holdout بدون تغییر، large forbidden جدید (`context.py` در template_render) | آن ۲۴ها از مسیر ۳۶ نبودند (جمع import+callee)؛ slot آزادشده forbidden آورد |
| F46 | callee با >۵ caller در fill امتیاز ۶ (نه ۱۵)؛ import با max نه جمع | large 0.504→**0.424** | golds چندفایلی django به همان سیگنال‌های ضعیف تکیه دارند |
| F47 | prompt کاملاً anchored (هر identifier کدشکل حل شده) → optional فقط با gain ≥۲۰ یا learned | فیکسچر `sms_stored` recall 0.5 (gold caller `SmsReceiver.kt` را می‌خواهد)، تست synaptic | golds dev/fixture *همسایه* می‌خواهند |
| F52 | «شرط شواهد» روی optional fill: فایل optional فقط اگر callee متمرکز seed (≤۵ caller یا نام‌برده)، synaptic/learned، یا یک توکن prompt در نام symbol/مسیرش (prefix ≥۴ حرف) | dev-4 0.906 (بدون تغییر)، holdout-2 0.496→0.500، **large 0.507→0.482**، hc/hl/hml بدون تغییر | mixed (session 10): golds چندفایلی django باز هم به importهای بی‌واژه تکیه دارند؛ چهارمین و آخرین تلاش fill — این خط بسته است |

**نتیجه‌گیری:** اختلاف باقی‌مانده تا هدف ۰.۶۰ روی large/holdout عمدتاً اختلاف *سبک gold* است، نه باگ موتور:
golds dev-4/fixture (نوشته‌شده با دید packet) همسایه‌های سؤال را جزو پاسخ می‌شمارند؛ golds large/holdout
(نوشته‌شده از خواندن کد، بدون دیدن packet) فقط فایل حاوی symbol را. یک موتور نمی‌تواند هم‌زمان هر دو را
راضی کند مگر با یک سیگنال جدید (intent «توضیح» vs «تغییر» — ولی سؤال‌های dev-4 هم «How does…» هستند با gold
چندفایلی، پس intent متن‌محور جدا نمی‌کند). باگ‌های واقعی که probe پیدا کرد (F40–F45) همه فیکس شدند.
**عدد صادقانه‌ی الان:** روی ریپوی ندیده با gold تک‌فایلی precision ≈۰.۴۹، recall 1.00، forbidden 0.
تصمیم: fill-tuning متوقف؛ اعداد به‌عنوان baseline فاز C ثبت می‌شوند. ابزار `NM_PROBE_RANK=1` (breakdown
امتیاز هر کاندید) به probe اضافه شد.

## G4/G5 — اندازه‌گیری دوباره‌ی سرعت بعد از F40–F45 (قانون: هر PR که seed/fill را لمس کند)

همان‌ماشین، همان‌ساعت، دو باینری release بدون embeddings، روی ultralytics (۹۳۱ فایل)، از Bash:

| | index | query p50 (n=10) | query p95 |
|---|---|---|---|
| 62b4f8b (شروع این بخش session) | 3478ms | 839ms | 900ms |
| 1db1380 (بعد از F40–F45) | 3415ms | 659ms | 712ms |
| نسبت | ۰.۹۸× | ۰.۷۹× | ۰.۷۹× |

گیت ≤۱.۵× pass — سریع‌تر شد (seedهای حدسی کمتر → resolve کمتر). اعداد مطلق با G4 رسمی (2142/280/464) فرق
دارند چون شرایط ماشین و روش (cwd، cold cache) فرق دارد؛ به همین دلیل baseline هم‌زمان ساخته شد. `eval
--release-gates` داخلی: هر دو باینری `passed=false` (از قبل؛ informational؛ `failure_classes` یکسان)، precision
یکسان 0.383، `l1_p95` **49→42ms** (G5: فاصله تا سقف ۵۰ از ۱ به ۸ms).

## فاز D-۱ — گرامر C/C++ + holdout دامنه‌ی خودش (libuv + fmt)

قبل: `.c/.h/.cpp` به fallback regex می‌افتادند (`registry.rs`: `grammar: None`). حالا `tree-sitter-c`/`tree-sitter-cpp`
(هر دو `=0.23.4`، ABI 14، سازگار با tree-sitter 0.24) با پروفایل query خودشان (`queries/c.scm`، `queries/cpp.scm`):
تابع (شامل pointer/reference declarator و `Owner::method` با `namespace_identifier` به‌عنوان parent)، struct/union/
class، enum، typedef/using، `#include "…"` به‌عنوان import (فقط quoted؛ `<stdio.h>` یال نیست)، call. `.h` با گرامر
C++ پارس می‌شود (superset؛ `vformat` با trailing return با گرامر C گم می‌شد).

**Holdout دامنه:** `tests/third_party/holdout-c/` — libuv v1.49.2 (C، ۳۷۷ فایل، unix/win دوگانه) + fmt 11.1.4 (C++،
۹۸ فایل، macro/template-heavy)، ۸+۸ تسک، گلد از خواندن کد قبل از هر اجرا، G1 با grep (udp.c به‌خاطر call
`uv_udp_open` از `uv_accept` رد شد → poll.c). تست: `third_party_c_holdout_gold.rs` (target-only، نه CI).

| اجرا | recall | precision | forbidden | oracle reachable / strict |
|---|---|---|---|---|
| اول (گرامر جدید، بدون هیچ تغییر دیگر) | 0.844 | 0.411 | 1 | 2/16 / 1 |
| + فیکس oracle (تعریف C-style) + `.h`→C++ | 0.781 | 0.521 | 1 | 12/16 / 8 |
| + `steer_same_name_symbol` | **0.938** | **0.625** | **0** | **15/16** / 10 |

سه یافته:
- **oracle باگ داشت** (`task_harness.rs::code_defines_symbol`): تعریف را فقط با کلمه‌ی کلیدی قبل از نام (`fn`/`def`/`func`/
  `static`…) می‌شناخت؛ `int uv_timer_start(` → `SymbolMissing` با وجود بدنه‌ی unfolded در packet. حالا «توکن‌های نوع +
  نام + `(`» هم تعریف است (C/C++/Java/C#). اعداد dev/فیکسچر تغییر نکردند (strict 0.909 / 19).
- **symbol هم‌نام در دو دایرکتوری پلتفرم**: `uv_run` در `src/unix/core.c` و `src/win/core.c`؛ resolver بدون سیگنال
  `win/` را می‌گرفت (۳ از ۴ شکست). `path_steer.rs` قبلاً همین را برای *فایل* هم‌نام داشت (`steer_same_name_file`،
  nanoGPT `prepare.py`)؛ نسخه‌ی symbol-سطح اضافه شد: کلمه‌ی prompt که با segment دایرکتوری یکی از twinها برابر است
  («unix») seed را به آن می‌برد، فقط وقتی دقیقاً یکی برنده است.
- باقی‌مانده: `fmt_vformat_to_string` recall 0 — `vformat`/`vformat_to` overloadهای زیاد در base.h/format.h؛ probe نشده.

**اندازه‌گیری‌نشده:** اثر گرامر C++ روی macroهای غیراستاندارد (FMT_FUNC) — tree-sitter با ERROR node ادامه می‌دهد؛
۴۳ symbol از format-inl.h استخراج شد ولی پوشش کامل شمارش نشد.

## فاز D-۲ — گرامر Scala / R / Julia + holdout دامنه (os-lib، r-lib/cli، Flux.jl)

سه گرامر جدید (`tree-sitter-scala =0.24.1`، `tree-sitter-r =1.3.0`، `tree-sitter-julia =0.23.1`، همه با tree-sitter
0.24) با query خودشان — شکل درخت‌ها اول با dump واقعی گرفته شد، نه از حافظه: Scala (`object`/`class`/`trait` به‌عنوان
owner متدها)، R (`name <- function` = `binary_operator lhs rhs:function_definition`؛ `library()`/`source()` با
`#match?` به‌عنوان import)، Julia (`function_definition (signature (call_expression (identifier)))`، فرم کوتاه
`f(x) = …` به‌عنوان `assignment`، struct پارامتری `(type_head (parametrized_type_expression …))`، `include("…")`).
`ImportStyle::CallArgOrPath` جدید (آرگومان call یا مسیر).

**Holdout:** `tests/third_party/holdout-lang/` — ۵+۵+۵ تسک، گلد قبل از اجرا، G1 با grep.

| اجرا | recall | precision | forbidden | reachable / strict |
|---|---|---|---|---|
| اول (گرامرها) | 0.933 | 0.502 | 1 | 8/15 / 5 |
| + oracle (تعریف R و Julia کوتاه) | 0.933 | 0.502 | 1 | 14/15 / 10 |
| + cluster با noun کدشکلِ حل‌شده covered + ترجیح نوع + جریمه‌ی `deprecations.*` | **1.000** | 0.502 | 1 | 14/15 / 9 |

per-language: Scala precision 0.80 (تمیز)، Julia 0.47، R **0.24** — پکیج R یک namespace تخت با call بین فایل‌های
خواهر است؛ همان اختلاف سبک gold (تک‌فایل vs همسایه) که در F36′/F46/F47 مستند شد، این‌جا شدیدتر.

یافته‌ها:
- **oracle باز هم**: `rule <- function(` و `activations(c, x) = …` را تعریف نمی‌شناخت (بعد از C-style در D-۱). حالا
  R assignment-function، Julia short-form، و `Owner::name(` با return type (C++) هم تعریف‌اند؛ تست واحد
  `definition_shapes` با ۹ شکل. اعداد dev/فیکسچر بدون تغییر.
- **cluster مرکب**: «how does calling a Dense on an AbstractVecOrMat …» — `Dense` در nouns است نه identifiers؛ cluster
  uncovered شمرده و `AbstractVecOrMat` (نوع Base، در ریپو نیست) fuzzy → `_grad_or_nothing`. حالا noun کدشکلِ حل‌شده
  هم cluster را covered می‌کند.
- **`Dense` → `deprecations.jl`**: shim قدیمی هم‌نام بر تعریف اصلی برنده می‌شد. دو قاعده در `pick_dominant_candidate`:
  query با حرف بزرگ → نود Class +۱۰؛ فایل low-priority (`deprecat*`، `compat`، `legacy`، test/bench/…) −۱۲.
- **F48 (باز)**: extractor «calling» را identifier می‌شمارد؛ stem fallback آن را به `optimise/train.jl:call` @1.00 می‌رساند
  (forbidden در `flux_dense_forward`). قاعده‌ی «stem hit کنار anchor دقیق = ضعیف» امتحان شد: lang forbidden 1→0 ولی
  holdout-c 0.625→0.594 با forbidden جدید — seedهای stem در fmt slot اشغال می‌کردند و حذفشان مسیر ۳۶ (F36) را باز
  کرد. رد شد؛ سقف confidence 0.72 برای stem hit در `resolve_seed_query` ماند (بی‌اثر روی اعداد).
- `seed_names` در حلقه‌ی ۳۶ (کلمه‌ای که خودش seed حل‌شده است دوباره resolve نشود): **رد شد** — large 0.504→0.479؛ golds چندفایلی
  django از همین مسیر فایل خواهر را می‌گیرند. باز هم F36: هر تغییر آن حلقه روی large می‌شکند.
- **دیسک پر شد** (۹G آزاد): `target/debug/incremental` ۷.۸G + build مقایسه‌ای `/tmp/nm-base-target` ۱.۵G پاک شدند.

## فاز D-۳ — overlay Keras / Hugging Face + holdout دامنه (keras-io، setfit)

overlay ML (`ml_overlay.rs`) torch-gated بود. حالا: شواهد `tensorflow`/`keras`/`transformers`؛ base‌های `Model`/`Layer`
(Keras)، `TFPreTrainedModel`/`FlaxPreTrainedModel`/`PeftModel` (HF)؛ TrainLoop با `.fit(`، `trainer.train(`،
`GradientTape`+`apply_gradients`؛ EvalLoop با `.evaluate(`، `trainer.predict(`؛ Checkpoint با `.save`/`.save_weights`/
`.save_pretrained`/`.push_to_hub` و `.load_weights`/`.from_pretrained`/`load_model`؛ نام checkpoint از هر literal
(`"bert-base-uncased"`، `"out/final"`) نه فقط پسوند `.pt`. دو تست واحد (Keras، HF).

**Holdout:** `tests/third_party/holdout-ml/` — keras-io (۲۷۷ اسکریپت example با آینه‌ی ipynb و md) + setfit (HF
Trainer-shaped)، ۵+۵ تسک، گلد قبل از اجرا، G1 (model_card.py چون import می‌شد رد شد → data.py).

| اجرا | recall | precision | forbidden | reachable / strict |
|---|---|---|---|---|
| اول (overlay جدید) | 0.500 | 0.203 | 0 | 5/10 / 3 |
| + walker: `examples/` وقتی هسته‌ی ریپوست ایندکس می‌شود | 0.600 | 0.203 | 1 | 5/10 / 3 |
| + ترجیح کد بر ipynb/md هم‌stem | 0.700 | 0.223 | 1 | 6/10 / 4 |
| + `file_by_stem` (کلمه‌ی کدشکل = stem فایل، بعد از جست‌وجوی symbol) | 0.900 | 0.307 | 1 | 8/10 / 6 |
| + steering با کلمات stem (وزن ۲) + seed artifact هم‌نامِ seed قبلی وارد نشود | **0.900** | **0.340** | **0** | **9/10** / 7 |

یافته‌ها (همه ساختاری، نه واژگانی):
- **walker هر `examples/` را مطلقاً نادیده می‌گرفت** (`walker.rs` لیست ثابت). برای keras-io یعنی هیچ‌چیز از محتوای
  اصلی. حالا یک‌بار با examples قدم می‌زند و بعد تصمیم می‌گیرد: اگر ≥۵۰٪ فایل‌های *زبان برنامه‌نویسی* (نه md/json/html)
  زیر `examples/` باشند، examples هسته است و می‌ماند؛ وگرنه مثل قبل حذف. `NeuralProjectGraph::examples_are_core()`
  (هر فایل example ایندکس‌شده = تصمیم گرفته شده) در جریمه‌ی ranking (`is_fixture_path_in`)، فیلتر نویز selector و
  هرس seed ضعیف استفاده می‌شود؛ `is_low_priority_source_path_in(path, examples_are_core)` در core.
- **سه آینه‌ی هم‌stem** (`x.py`، `ipynb/x.ipynb`، `md/x.md`): ranking فایل md را می‌گرفت. tiebreak: md/rst/txt −۸،
  ipynb −۴.
- **stem فایل به‌عنوان query**: `image_classification_from_scratch` با `search_symbols(…, 12)` هیچ‌وقت به فایل نمی‌رسید. `file_by_stem` بعد از جست‌وجوی symbol، فقط برای نام‌های snake/kebab با ≥۳ بخش — نسخه‌ی «هر کلمه‌ی کدشکل» holdout-2 را ۰.۰۰۳ پایین آورد (`roi_heads` در prompt → `roi_heads.py`، که gold نمی‌خواهد)؛ قبل از جست‌وجوی symbol، `physarum_usage` فیکسچر را می‌شکست.

  دو فایل کد هم‌stem = مبهم = هیچ. اسکریپت دوبخشی (`mnist_convnet`) از جست‌وجوی symbol resolve می‌شود، نه از این مسیر.
- **twin هم‌نام در اسکریپت‌های خواهر** (`CTCLayer` در captcha_ocr و handwriting_recognition): steering با کلمات stem
  فایل با وزن ۲ («captcha»، «ocr»)؛ و seed نوع artifact که هم‌نام seed قبلی است دیگر وارد نمی‌شود.
- **F50 (باز)**: اسکریپت با نام یک کلمه‌ی انگلیسی ساده (`autoencoder.py`) — extractor identifier نمی‌سازد، پس
  `file_by_stem` هرگز پرسیده نمی‌شود؛ recall 0.

## G4/G5 — اندازه‌گیری دوباره بعد از D-۱…D-۳ + F50 (main 694b12d)

همان روش (release بدون embeddings، ultralytics، Bash، n=10)؛ نه همان ساعتِ baseline:

| | index | query p50 | query p95 | `l1_p95` (این ریپو) |
|---|---|---|---|---|
| 62b4f8b (شروع بخش خودمختار) | 3478ms | 839ms | 900ms | 49ms |
| 1db1380 (بعد از F40–F45) | 3415ms | 659ms | 712ms | 42ms |
| 694b12d (بعد از D-۱…D-۳، F50) | 3590ms | 699ms | 748ms | **47ms** |

نسبت به baseline: index ۱.۰۳×، query ۰.۸۳× — گیت ≤۱.۵× pass. ولی `l1_p95` از ۴۲ به ۴۷ برگشت (سقف ۵۰؛ فاصله ۳ms).
منابع محتمل: walker حالا `examples/` را قدم می‌زند و بعد تصمیم می‌گیرد (metadata فقط)، `examples_are_core()` یک بار
per revision، پنج گرامر جدید (اثر فقط روی فایل‌های همان زبان‌ها)، `file_by_stem` (پیمایش `file_to_nodes` فقط در
fallback). واریانس این ماشین بین دو اجرا ~۵–۱۰٪ است؛ اگر `l1_p95` در PR بعدی به ۵۰ رسید، طبق G5 پروفایل، نه بالا
بردن گیت. `eval --release-gates` precision داخلی بدون تغییر (0.383).

## F51 — anchorهای C++ در prompt: `std::` و پسوندهای فایل زبان‌های جدید (فیکس شد)؛ resolver `Owner::member` (رد شد)

probe `fmt_vformat_to_string` (تنها recall 0 در holdout-c) سه چیز نشان داد:
- `identifier:std::string` → `base.h:string_value` و `identifier:string` → `test/format-test.cc:string`: extractor
  (`identifiers.rs`، `qual_re`) هر `a::b` را با هر دو شکل push می‌کرد. `std::` کتابخانه‌ی استاندارد است — نه مسیر، نه
  عضو خالی‌اش anchor نیست. **فیکس.**
- «format-inl.h» در prompt file hint نمی‌شد: regex فایل‌های bare پسوندهای C/C++/Scala/Julia/R/ipynb را نداشت (زبان‌های
  D-۱/D-۲ به این لیست اضافه نشده بودند). **فیکس.**
- `identifier:detail::vformat_to` → `format.h:to_utf8` @1.00: مسیر `::` در activator از `resolve_best` (جست‌وجوی fuzzy)
  اولین hit را با confidence ۱.۰ می‌گیرد، حتی با نام دیگر. فیکس اصولی (مثل `Owner.member`: `resolve_dotted_member`
  + پذیرش فقط با نام برابر) امتحان شد: vformat درست شد ولی `fmt_print_to_file` forbidden گرفت — seed غلط قبلی
  (`detail::print` → یک symbol بی‌ربط) یک slot required را اشغال می‌کرد و `ostream.h`/`compile.h` (focus_term ۳۶، F36)
  بیرون می‌ماندند. **رد شد**؛ باگ resolver `::` باز است (F51b) و تا F36 حل نشود دست‌زدنی نیست.

| مجموعه | قبل (main 02c0023) | بعد (F51، فقط extractor) |
|---|---|---|
| holdout-c | 0.938 / 0.625 / 0 / 15/16 strict 10 | **1.000** / **0.656** / 0 / **16/16** strict 10 |
| بقیه | — | (regression در PR) |

## جمع‌بندی صادقانه

- recall خوب است (0.95) — موتور تقریباً هیچ‌وقت فایل گلد را کاملاً گم نمی‌کند، حتی روی ریپوی ندیده.
- precision (0.146) و forbidden (واقعی: ۲/۲۰، بعد از کسر ۲ اشتباه گلد) نشان می‌دهند دقتِ 0.856 مرحله ۴ **کاملاً به واژگان و ساختار ۴ ریپوی dev بسته بود** — یک نوع overfit سیستماتیک، نه یک باگ نقطه‌ای.
- ریشه‌ی مشترک هر سه یافته (F33–F35): قواعد seed/synonym مرحله ۴ روی متن/زبان مشخص (JS dotted-access، انگلیسی generic، واژگان auth-style) نوشته شدند و روی زبان/دامنه‌ی جدید یا miss می‌کنند (بی‌ضرر) یا با کلمه‌ی عمومی هم‌نام برخورد می‌کنند (مضر).
- طبق تصمیم فاز ۵a: این ریپوها اکنون **holdout باقی می‌مانند** (به dev منتقل نشدند)؛ فیکس این یافته‌ها کار فاز بعدی (بازبینی مرحله ۴ با تصمیم صریح Parsa) است، نه این session.
- ۵c اندازه‌گیری شد (بالا) — عدد نگران‌کننده‌ای نبود، یک گیت با حاشیه‌ی کم (`l1_p95_slo`) و یک بخش اندازه‌گیری‌نشده (baseline ریپوی بزرگ) ماند.
- اندازه‌گیری‌نشده در این session: ۵b (B2B) — هنوز شروع نشده، منتظر گلد Parsa.

## ۵b — ریپوی خصوصی B2B (session 10، ۱۴ سپتامبر ۲۰۲۶، main بعد از #72)

روش: گلد ۱۲ سؤال با خواندن کد (grep + راهنمای ریپو)، بدون هیچ اجرای موتور، در یک git محلی خارج از این ریپو
قفل شد (sha256 `e1981f14…`) و بعد اولین اجرا انجام شد. harness: `third_party_private_gold` با
`NM_PRIVATE_SET_DIR`/`NM_PRIVATE_DIR`؛ هیچ مسیر، نام فایل یا کد خصوصی وارد این ریپو نمی‌شود. ریپو: بک‌اند
Fastify+Drizzle (ESM، ماژول‌های ۴لایه) + فرانت Next.js، ~۱۲۰۰ فایل ایندکس‌شده، ۵۸۸ فایل TS/TSX.
نکته‌ی صداقت: گلد را Claude نوشت (engine-blind)، نه تیم Parsa — استقلال‌ش از سبک گلدهای holdout عمومی کمتر است.

| معیار | private (۱۲ تسک) |
|---|---|
| recall | **0.917** (۱۱/۱۲ کامل؛ یک تسک 0) |
| precision | **0.559** (۳ تسک 1.00، ۶ تسک ≥0.5، ۳ تسک ≤0.17) |
| forbidden | **0** (دو تسک با برخورد نام عمدی — `canTransition` در دو ماژول، «commission» — درست جدا شدند) |

یافته‌ها (ثبت؛ فیکس فقط اگر ساختاری و روی هر شش مجموعه‌ی عمومی بی‌افت):

| # | یافته | probe |
|---|---|---|
| F53 | walker پوشه‌های ابزار agent (`.claude/skills/**`، `.cursor/**` — اسکریپت‌های js/mjs/py مربوط به ابزار، نه پروژه) را ایندکس می‌کند و در ۳ تسک از ۱۲، فایل‌های آن‌ها وارد packet شدند (precision آن سه تسک 0.14–0.40). هم‌جنس درس D-۳ (`examples/`)، در جهت عکس: دایرکتوری ابزار = vendor | فایل‌های `.claude/skills/*/scripts/*.mjs` در packet سؤال cache/otp |
| F54 | «the maintenance plugin» + دو ثابت `MAINTENANCE_*` → packet: صفحه‌ی فرانت `maintenance/page.tsx` و دو فایل بی‌ربط؛ فایل واقعی `plugins/maintenance.plugin.ts` و `config/maintenance-sections.ts` (تعریف ثابت‌ها) نیامدند. recall 0 | probe لازم: آیا ثابت‌های SCREAMING_SNAKE به‌عنوان identifier حل می‌شوند؟ آیا «plugin» به الگوی نام `*.plugin.ts` وصل می‌شود؟ |

### F53 — اندازه‌گیری شد، مرج نشد (برنچ `f53-agent-tool-dirs`)

walker: `.claude/`، `.cursor/`، `.codex/`، `.windsurf/`، `.aider/` مثل `.gemini/` نادیده. شش مجموعه‌ی عمومی دقیقاً
بی‌تغییر (هیچ‌کدام این پوشه‌ها را ندارند)؛ **private 0.559→0.536**: جای سه فایل `.claude/skills/*` را فایل‌های بی‌ربط
دیگر (`.vue` کیت UI، `globals.css`، یک اسکریپت `scripts/`) گرفتند — همان الگوی F36′/F51b: slot آزادشده را حلقه‌ی ۳۶
پر می‌کند. فیکس از نظر محصول درست است (اسکریپت ابزار agent هرگز پاسخ سؤال کاربر نیست) ولی طبق قانون ratchet
مرج نمی‌شود تا منبع نویز (F36) حل شود. سومین شاهد مستقل که «حذف یک منبع نویز» بدون F36 عدد نمی‌دهد.

### F54 — فیکس شد (private recall 0.917→1.000، precision 0.559→0.601؛ شش مجموعه‌ی عمومی بی‌تغییر)

دو شکاف عمومی، هر دو با probe همان تسک:
1. `IDENT_RE` در `identifiers.rs` فقط `snake_case`/`camelCase`/`PascalCase` را anchor می‌کرد؛ ثابت `SCREAMING_SNAKE`
   (`MAX_ATTEMPTS`، `MAINTENANCE_SECTION_RULES`) اصلاً seed نمی‌شد — فقط کلمه‌ی کوچک‌شده‌ی «maintenance» به یک
   صفحه‌ی فرانت هم‌نام می‌رفت. یک alternative جدید (≥۲ بخش بزرگ‌حرف).
2. گرامر TS/Python برای `export const X = …` / `X = …` سطح ماژول هیچ نودی نمی‌ساخت (فقط arrow function). capture
   جدید `@symbol` فقط با spelling `SCREAMING_SNAKE` و فقط exported/module-level — تا `const x` محلی نود نشود (ریسک
   F36). تست واحد برای هر دو زبان.

سرعت (G4/G5، ultralytics، release بدون embeddings، Bash، n=10، interleaved؛ ماشین امروز پرنویز — p95 تا ۱۴s):
index 2186→2610ms (۱.۱۹×)، query p50 960→1068ms (۱.۱۱×) — گیت ≤۱.۵× pass. `eval --release-gates`: precision
0.383 یکسان؛ `l1_p95` base **55ms**، F54 **53ms** — هر دو بالای سقف ۵۰ تحت بار امروز (۴۷ در آخرین اندازه‌گیری آرام)؛
به F54 نسبت داده نمی‌شود، ولی G5 می‌گوید در اولین فرصت آرام دوباره بسنج.

## Config→Code — ساخته و اندازه‌گیری شد (session 10)

پنج قطعه: `yaml.rs` (کلید YAML → نود Config، لیست/workflow/`.github` بیرون، سقف ۲۰۰)، `config_reads.rs` (Python:
`cfg.x`/`args.x`/`config["x"]`/`cfg.get("x")` → یال Parameterizes با hint فایل از `load("x.yaml")`/Hydra
`config_name`؛ `add_argument("--x")` → نود Hyperparameter)، arm جدید linker (`resolve_config_key`: فقط بین
نودهای Config/Hyperparameter، hint→فایل، یکتا→Proven، چند→dominant/Likely)، تست یکپارچه، holdout دامنه
`holdout-cfg` (lightning-hydra-template + detr، ۱۱ تسک، گلد قبل از اجرا).

سه درس از سه افت (هر کدام probe شد):
| افت | علت | قاعده |
|---|---|---|
| holdout-ml 0.360→0.347، dev 0.906→0.866 (v2) | نود Hyperparameter `metric`/`dataset` از اسکریپت argparse با کلمه‌ی انگلیسی prompt در حلقه‌ی ۳۶ match شد؛ فیلتر اول JSON را هم گرفت و dev افتاد | کلید YAML/argparse فقط با نام **کدشکل** (`_`، رقم، camel) حل می‌شود؛ کلید JSON رفتار قبلی |
| large `ultra_predict_stream` 0.25→0.17 | سه call `thop.profile(...)` به کلید YAML `profile` وصل شدند؛ بعد از فیلتر، `Profile` از ۷ به ۴ caller *واقعی* رسید → «callee متمرکز» → `ops.py` وارد packet. یعنی روی main یک یال غلط، یک گلد را نجات می‌داد | کلید Config/Hyperparameter هرگز هدف Calls نیست (سه مسیر resolver: `resolve_call_target`، `resolve_call_ranked`، fallback `resolve_ranked`) |
| hub در spreading | کلید `conf` با ده‌ها خواننده، انرژی را از یک خواننده به بقیه می‌برد | `spread_energies` یال Parameterizes را فقط در جهت کلید→خواننده می‌پیماید |

| مجموعه | main (#76) | Config→Code |
|---|---|---|
| holdout-cfg | — | 0.879 / **0.610** / 0 |
| holdout-ml | 0.360 | 0.360 |
| large | 0.507 | 0.502 (−۰.۰۰۵، علت بالا) |
| private | 0.601 | 0.587 (−۰.۰۱۴: یک فایل در `b2b_totp_verify` — `auth.controller.ts`؛ زیر آستانه‌ی ۰.۰۲) |
| dev / holdout-2 / c / lang | 0.906 / 0.496 / 0.656 / 0.502 | بی‌تغییر |

recall holdout-cfg 0.879: دو تسک hydra که کلید فقط از طریق `self.hparams.x` خوانده می‌شود و `hparams` با
`save_hyperparameters()` پر می‌شود — یالی به YAML نیست چون نام فایل config در آن ماژول نیامده (Hydra `_target_`
آن را instantiate می‌کند). ثبت (F55): «کلید بی‌hint در ریپوی چند-config» — hint از `_target_: module.Class`
در YAML به کلاس Python قابل استخراج است؛ انجام نشد (precision-tuning بسته).

## فاز C — task success با مدل واقعی (session 11، ۱۵ سپتامبر ۲۰۲۶)

اولین اجرای کامل `neuromesh eval --tasks --executor model` با کلید واقعی روی هر ۳ `--context` (packet،
whole-gold-files، grep)، روی fixtures (۲۶ تسک dev) + دو holdout تازه (`holdout-gin`، `holdout-vision`، هرکدام
۱۰ تسک). پاسخ‌دهنده `deepseek-ai/DeepSeek-V4-Flash-0731`، داور جدا `zai-org/GLM-5.3`، هر دو از طریق Baseten
(`https://inference.baseten.co/v1`, provider=openai). Verify واقعی (اجرای patch + تست) برای تسک‌های patch؛
QA-judge برای تسک‌های توضیحی.

### مسیر تا رسیدن به این عدد — سه provider رد شد

| تلاش | مشکل | نتیجه |
|---|---|---|
| 9router (`cf/@cf/meta/llama-3.3-70b-instruct-fp8-fast` + `cf/@cf/mistralai/mistral-small-3.1-24b-instruct`) | Cloudflare Workers AI مدام HTTP 503 (ظرفیت) برمی‌گرداند | ۱/۹ ست بعد از ~۴۰ دقیقه؛ رها شد |
| Groq (`qwen/qwen3.6-27b` + `openai/gpt-oss-20b`) | سقف OTPM (توکن خروجی در دقیقه) بسیار تنگ روی این org — برخی مدل‌ها (`qwen3.8-27b`) فقط ۱۰۰۰ توکن/دقیقه | حتی بعد از اضافه‌کردن retry/backoff (که provider OpenAI اصلاً نداشت — باگ واقعی، فیکس شد)، هر context ده‌ها دقیقه طول می‌کشید؛ رها شد به دستور Parsa |
| Baseten (`deepseek-ai/DeepSeek-V4-Flash-0731` + `zai-org/GLM-5.3`) | — | تمام ۹ context بدون هیچ 429/503 در حدود ۱۵ دقیقه تمام شد |

دو فیکس کد لازم شد تا Baseten/Groq اصلاً قابل‌اتصال شوند (پیش از این `OpenAIProvider` فقط `api.openai.com` را
می‌شناخت و فقط Anthropic provider retry داشت):
1. `crates/neuromesh-provider/src/openai.rs`: `OpenAIProvider::new` حالا `OPENAI_BASE_URL` env را می‌خواند
   (مثل الگوی `ANTHROPIC_BASE_URL` که از قبل بود) — بدون این، هیچ gateway ای غیر از OpenAI رسمی در دسترس نبود.
2. همان فایل: retry/backoff روی ۴۲۹/۵xx اضافه شد (`OPENAI_MAX_RETRIES`, پیش‌فرض ۸) — قبلاً هر ۴۲۹ بلافاصله
   کل تسک را fail می‌کرد؛ الگو از `anthropic.rs` کپی شد.
3. `scripts/phase-c-run.sh`: پشتیبانی از `PROVIDER=openai` (`--provider`/`--judge-provider`)، `--allow-self-judge`
   وقتی پاسخ‌دهنده و داور یکی هستند، و اصلاح یک باگ نام‌گذاری فایل (slug گرفته می‌شد از پاسخ‌دهنده‌ی *اول* در
   PAIRS نه از جفت جاری — یعنی اگر جفت اول fail می‌شد و جفت دوم قبول، فایل با نام غلط ذخیره می‌شد؛ الان به
   `${pair%%:*}` از همان pair اصلاح شده).

### جدول ۳×۳ اولیه (success rate / strict / success-per-1k-token)

| مجموعه | packet | whole-gold-files | grep |
|---|---|---|---|
| **fixtures** (dev, ۲۶ تسک) | 0.769 / 0.731 / 0.164 | 0.885 / 0.769 / 0.106 | 0.577 / 0.538 / 0.110 |
| **holdout-gin** (Go, ۱۰ تسک) | **1.000** / 0.800 / 0.131 | 0.900 / 0.800 / 0.080 | **0.400** / 0.400 / 0.018 |
| **holdout-vision** (۱۰ تسک) | 0.800 / 0.700 / 0.098 | 0.700 / 0.600 / 0.075 | 0.500 / 0.500 / 0.024 |

forbidden hits: **۰ در هر ۹ سلول**.

### F56 — باگ در baseline `grep`، نه در NeuroMesh: سقف بایت به‌ترتیب کشف فایل، نه ربط

Parsa پرسید چرا `grep` روی `holdout-gin` این‌قدر بد است (۰.۴۰۰). ریشه‌یابی سریع (probe، نه تیونینگ):

`grep_context` (`crates/neuromesh-cli/src/commands/tasks.rs`) یک baseline مقایسه‌ای است — شبیه‌سازی یک جستجوی
متنی ساده، **بخشی از NeuroMesh نیست**، فقط رقیب کنترلی در harness. پیاده‌سازی قدیم: پیمایش DFS پوشه‌ها با
ترتیب `read_dir` سیستم‌عامل (نه بر اساس ربط)، و یک سقف کلی ۶۰۰۰۰ بایت روی کل خروجی — بدون سقف به‌ازای هر فایل.
روی ریپوی gin، کلمات کلیدی استخراج‌شده از prompt عمومی بودند (`path`، `node`، `request`، `look`) که در تقریباً
هر فایل `.go` می‌آیند. بررسی مستقیم نشان داد:

- `context.go`: ۱۲۰ خط match
- `context_test.go`: ۲۸۷ خط match

این دو فایل به‌ترتیب الفبایی **قبل از** `tree.go` (فایل واقعاً لازم برای `gin_route_tree`) می‌آیند و به‌تنهایی
کل سقف ۶۰۰۰۰ بایتی را پر می‌کنند — یعنی `tree.go` هرگز به خروجی نمی‌رسید. لاگ خرابی‌ها این را تأیید کرد: ۵ از
۶ fail، مدل صریح نوشته بود «context لازم را ندارم» (`tree.go`, `context.go`'s bind methods,
`recovery.go`, `logger.go`, `gin.go`'s trusted-proxy logic).

**فیکس** (همان کامیت PR فاز C): جمع‌آوری همه‌ی فایل‌های match قبل از نوشتن خروجی، رتبه‌بندی بر اساس تعداد
کلیدواژه‌ی **متمایز** match‌شده (نزولی، سپس مسیر برای قطعیت) به‌جای ترتیب فایل‌سیستم، + سقف جداگانه‌ی هر فایل
(۶۰۰۰ بایت) تا یک فایل تک نتواند کل بودجه را ببلعد. هنوز کاملاً «کور» و متن‌محور است — فقط دیگر به شانس ترتیب
پوشه‌ها وابسته نیست.

### نتیجه‌ی فیکس — سه ست grep دوباره اجرا شد

| مجموعه | قبل | بعد | بهبود |
|---|---|---|---|
| fixtures/grep | 0.577 (strict 0.538) | 0.577 (strict 0.500) | بدون تغییر معنادار — ریپوهای dev کوچکند، سقف اصلاً گلوگاه نبود |
| **holdout-gin/grep** | 0.400 (strict 0.400) | **0.900** (strict 0.800) | **+۱۲۵٪ نسبی** |
| **holdout-vision/grep** | 0.500 (strict 0.500) | **0.800** (strict 0.600) | **+۶۰٪ نسبی** |

### جدول ۳×۳ نهایی (بعد از فیکس grep)

| مجموعه | packet | whole-gold-files | grep |
|---|---|---|---|
| **fixtures** (dev, ۲۶ تسک) | 0.769 / 0.731 / 0.164 | 0.885 / 0.769 / 0.106 | 0.577 / 0.500 / 0.108 |
| **holdout-gin** (Go, ۱۰ تسک) | **1.000** / 0.800 / 0.131 | 0.900 / 0.800 / 0.080 | **0.900** / 0.800 / 0.041 |
| **holdout-vision** (۱۰ تسک) | 0.800 / 0.700 / 0.098 | 0.700 / 0.600 / 0.075 | **0.800** / 0.600 / 0.041 |

forbidden hits: **۰ در هر ۹ سلول** (بعد از فیکس هم).

### سه گیت فاز C (سند ۰۶، ردیف C) — بعد از فیکس

| گیت | نتیجه |
|---|---|
| `task_success ≥ 0.5` (هر سلول) | **قبول در هر ۹ سلول** — پایین‌ترین عدد حالا fixtures/grep = 0.577 |
| `success/1k-token(packet) > success/1k-token(whole-gold-files)` | **قبول در هر سه مجموعه**: fixtures 0.164>0.106، holdout-gin 0.131>0.080، holdout-vision 0.098>0.075 |
| forbidden hits = 0 | **قبول** در هر ۹ سلول |

**نتیجه‌ی کلی: هر سه گیت فاز C کامل قبول شد.** با فیکس باگ baseline، grep دیگر یک outlier کاذب نیست — همچنان
ضعیف‌ترین context روی هر سه مجموعه می‌ماند (کارایی توکن ۲.۶ برابر بدتر از packet روی میانگین holdout)، ولی این
یک تفاوت واقعی و معنادار است، نه یک artifact پیاده‌سازی.

### چیزی که اجرا نشد

بند «دو بار: روی commit قبل از B و بعد از B» ردیف C سند ۰۶ اجرا نشد — فقط روی main فعلی (بعد از B، #20–#79)
اجرا شد. مقایسه‌ی baseline (قبل از فاز B) به دلیل هزینه‌ی زمان/توکن این جلسه انجام نشد؛ یک آیتم باز برای جلسه‌ی
بعد. F25 (تعریف گیت ریلیز precision) هنوز به تصمیم صریح Parsa نیاز دارد — بی‌ربط به این اجرا.

## F57 — gate ی sidecar حلقه‌ی یادگیری را روی هر ریپوی >۲۰ فایل خاموش کرده بود (کشف حین F31، PR #85)

وقتی `FILL_GATE_MIN_FILES` حذف شد و gate روی پروژه‌های کوچک هم اعمال شد، سه تست واحد قدیمی activator شکست:
`synaptic_feedback_pulls_coedited_file_into_second_packet`، `learning_to_emission_kosha_routes_emitted`،
`reinforced_file_promotes_only_on_focus_matched_query`. هر سه «فایلِ تقویت‌شده با feedback باید در packet دوم
بیاید» را می‌سنجند — و هر سه فقط روی فیکسچرهای <۲۰ فایل اجرا می‌شدند. یعنی روی هر ریپوی واقعی، از stage 4 تا
حالا، gate فایل‌های یادگرفته‌شده را به‌عنوان «stem-match بی‌دلیل» حذف می‌کرد و حلقه‌ی feedback عملاً بی‌اثر بود؛
هیچ تستی روی ریپوی بزرگ این را نمی‌دید.

رفع: فایل «یادگرفته‌شده» از gate معاف است — یا `file_learning_boost_index > 0` (reinforce_node_access) یا یالی
با `reinforcement_count > 0` بین آن و seed/فایل seed (`reinforce_path`). این شرط بدون آستانه است چون این دو
شمارنده فقط با feedback حرکت می‌کنند. اثر روی ۷ مجموعه‌ی بنچمارک: صفر (بنچمارک feedback ندارد) — یعنی همین
اعداد هم نشان نمی‌دهند حلقه‌ی یادگیری روی ریپوی واقعی *کار می‌کند*؛ فقط دیگر ساختاراً مسدود نیست. سنجش
واقعی آن (packet دوم بعد از feedback روی dev-4) یک آیتم باز است.

## فاز D — holdout-ml2 (peft + keras-hub)، اولین اجرا (session 12، ۱۹ سپتامبر ۲۰۲۶، main بعد از #86)

چرا: holdout-ml (keras-io + setfit) در D-۳ پنج بار با نگاه به عددش تیون شد → طبق اصل holdout دیگر «ندیده» نیست
(در `measured.md` به dev-class برچسب خورد). دو ریپوی کتابخانه‌ای ML که overlay هرگز رویشان اجرا نشده بود انتخاب شد:
peft (HF adapter library، ۴۴۶ فایل py) و keras-hub (Keras 3 model library، ۱۳۴۹ فایل py). ۵+۵ سؤال از خواندن کد،
G1 با grep، PR #86 قفل قبل از هر اجرا.

| اجرا | recall | precision | forbidden | reachable / strict |
|---|---|---|---|---|
| اول، بدون هیچ تغییر موتور | 1.000 | 0.557 | 1 | 9/10 / 6 |
| همان، بعد از اصلاح یک خطای گلد (پایین) | **1.000** | **0.632** | **0** | **10/10** / 6 |

**خطای گلد (مثل ۲ مورد از ۴ در ۵a):** `peft_get_peft_model` فایل `auto.py` را forbidden داشت، در حالی که prompt
صریحاً `MODEL_TYPE_TO_PEFT_MODEL_MAPPING` را نام می‌برد و `auto.py` آن را import و استفاده می‌کند — G1 من روی
این symbol را جدا از فایل forbidden اجرا کرده بودم. forbidden به `merge_utils.py` (۰ hit) اصلاح شد؛ موتور درست بود.

**خوانش:** گیت holdout (recall ≥0.90 / precision ≥0.60 / forbidden 0) **بدون هیچ تیونی پاس شد** — اولین holdout
دامنه‌ای که این اتفاق برایش می‌افتد (holdout-ml اولش 0.203 بود). یعنی قواعد ساختاری D-۳ (examples-as-core،
ترجیح کد بر md/ipynb، file_by_stem) به ریپوی ML کتابخانه‌ای تعمیم پیدا کرد. keras-hub: ۳ تسک precision 1.00.
ضعف‌ها (ثبت، تیون نمی‌شود — F58):
- **peft بسته‌های خیلی بزرگ** می‌دهد (۲۵–۳۰k توکن برای `save_pretrained`/`load_adapter`): `peft_model.py` ~۲۰۰۰ خط
  است و `tuners_utils.py` + `utils/other.py` هم‌راه می‌آیند. precision 0.50 آنجا از دو فایل «زیرساخت» هم‌سایه است.
- **هم‌نام در tunerهای خواهر**: `Linear.merge` در `adamss/layer.py` و `glora/layer.py` هم هست؛ prompt «LoRA Linear»
  را می‌گوید ولی resolver «LoRA» را به پوشه‌ی `lora/` گره نمی‌زند (precision 0.33). هم‌جنس F21/F28 (twin هم‌نام)،
  این‌بار تمایز از *مسیر پوشه* می‌آید نه owner.
- **strict 6/10**: چهار fold روی متد اصلی سؤال — `get_peft_model` (تابع سطح ماژول که *اسم خود prompt* است، fold شده!)،
  `Linear.merge/unmerge`، `load_peft_weights/set_peft_model_state_dict`، `get_weight_norm`. مورد اول باگ‌نماست: اسم
  دقیق در prompt + تابع سطح ماژول، ولی fold. probe لازم روی dev (نه اینجا).
- keras-hub `kh_topk_sampler`: هر پنج sampler خواهر (beam/top_p/…) وارد packet شدند (precision 0.40) — همان الگوی
  «هم‌نام در فایل خواهر» با `get_next_token`.

## F58 → probe روی dev (session 12، PR #88): دو باگ واقعی، هر دو عمومی، هر دو روی dev قابل بازتولید

F58 روی holdout-ml2 گفت «`get_peft_model` با نام دقیق در prompt fold می‌شود» (strict 6/10). طبق اصل holdout، probe فقط
روی dev انجام شد: مجموعه‌ی `large` همان الگو را داشت (strict 14/20؛ ۴ تسک با need ی Folded که نامش دقیقاً در prompt
بود: `BaseTrainer.train`، `get_labels`، `predict`، `__call__`). سه چیز پیدا شد:

| # | یافته | نوع | فیکس |
|---|---|---|---|
| **F59** | **باگ harness**: need بدون owner (`trainer.py::train`) وقتی `BaseTrainer.train` باز و `MultiTrainer.train` fold بود، Folded گزارش می‌شد — `folded.iter().any(...)` روی نام برهنه. یعنی strict این‌همه مدت *کمتر از واقعی* گزارش می‌شد | harness | need بدون owner فقط وقتی Folded است که تعداد fold های هم‌نام ≥ تعداد تعریف‌های هم‌نام در کد ارسالی (`count_symbol_definitions`). large strict 14→17 بدون هیچ تغییر موتور |
| **F58** | نام dunder/snake_case که در prompt عیناً آمده (`__call__`, `get_labels`) برای نام خودش هیچ امتیازی نمی‌گرفت: `tokenize_name` آن را به `call`/`get`,`labels` می‌شکند ولی `prompt_tokens` زیرخط‌ها را نگه می‌دارد → هیچ‌وقت match | موتور، fold | `prompt_names_identifier`: کلمه‌ی identifier-شکل (زیرخط یا mixed-case، ≥۵ کاراکتر) که عیناً در prompt است +۶۰. کلمه‌ی ساده (`forward`, `train`) عمداً نه — F7. large strict 17→18 |
| **F60** | orchestrator کلاس seed: `v8DetectionLoss.__call__` = `return self.loss(self.parse_output(preds), batch)` — ۵۶ امتیاز، زیر helperهایی که کلمات prompt را دارند (۸۳/۷۲/۶۴/۶۰)، با بودجه‌ی ۴ fold می‌شد | موتور، fold | متدِ owner ساختاری که بدنه‌اش ≥۲ exon خواهرِ انتخاب‌شده در pass اول را صدا می‌زند (`calls_member`) +۳۰. large strict 18→**19/20** (تنها باقی‌مانده `django_template_render` = FileMissing، مسئله‌ی recall نه fold) |

dev بدون تغییر (0.906 / strict 20). holdout-ml2 **دست نخورد** — اگر عددش بعد از این PR تغییر کند، اثر جانبی یک فیکس
dev-driven است و همان‌طور ثبت می‌شود، نه هدف.

## F61 — precision روی large: seedهای ضعیف در فایل‌های asset و کلمات هم‌معنی (session 12، PR #89)

probe با dump دلیل هر فایل packet (`expansion_reason`) و seedهای required روی ۶ تسک کم‌precision ی `large`:
همه‌ی فایل‌های اضافه با reason `utility:8.50` و **sidecar=false** بودند — یعنی نه fill بلکه *seed* بودند. سه منبع:

| مورد | مثال | ریشه | فیکس |
|---|---|---|---|
| seed ضعیف در asset | `concept:next` → `<g id="next">` در `calendar-icons.svg`؛ `concept:error` → قاعده‌ی `.error` در `base.css` — هر دو برای سؤال پایتونی با seed قوی پایتونی | `prune_off_family_weak_seeds` (F34 خانواده‌ی زبان) فایل بدون خانواده (svg/css) را رد نمی‌کرد چون `family()` برایش None بود | seed ضعیف در `svg/css/scss/sass/less` وقتی seed قوی کدی هست حذف می‌شود (`is_style_asset`). سؤال style ای seed قوی کدی ندارد → دست‌نخورده |
| هم‌معنیِ بی‌owner در فایل غریبه | `concept:auth` → `auth()` در `contrib/auth/context_processors.py`؛ `concept:engine` → `Engine` در `template/engine.py` — هیچ‌کدام در prompt نیامده (از بسط مترادف آمده‌اند) | F27 (`weak_symbol_seed`) برای symbol بدون owner `None => true` داشت — یعنی هر تابع سطح ماژول با نام دقیق مفهوم قبول می‌شد | symbol بدون owner فقط اگر خود کلمه در prompt باشد؛ بقیه‌ی F27 دست‌نخورده |
| acronym کنار identifier بلند | `identifier:CSRF` → `csrf()` در `template/context_processors.py` در حالی که `CsrfViewMiddleware` seed قوی است | extractor «CSRF» را identifier مستقل می‌گیرد؛ هیچ قاعده‌ای آن را به identifier بلندتری که token اش است گره نمی‌زد (F28 فقط `owner.member`) | در `bare_owner`: acronym تمام‌بزرگ (≥۳) که token یک identifier seed *resolve‌شده‌ی* بلندتر است حذف می‌شود |

| set | قبل | بعد |
|---|---|---|
| large | 0.502 | **0.541** |
| holdout-2 | 0.496 | **0.538** |
| holdout-lang | 0.502 | **0.536** |
| holdout-ml | 0.360 | 0.365 |
| dev، holdout-c، holdout-ml2، holdout-cfg | — | بدون تغییر |

recall/forbidden/strict روی هر ۸ مجموعه بدون تغییر. ratchet large به 0.52 بالا رفت. **باقی‌مانده‌ی probe (ثبت، نه فیکس):**
`django_template_render` recall 0 — `Template.render` به کلاس `Template` در `backends/django.py` می‌رود نه `template/base.py`
(twin کلاس هم‌نام؛ گلد base.py را می‌خواهد؛ tie-break باید از «compile a parsed node list» بیاید: `nodelist` فقط در base.py).
`ultra_predict_stream` — `postprocess` هم‌نام در `detect/val.py`، `obb/val.py`، `classify/val.py` (twin در فایل خواهر، همان
خانواده‌ی F58 برای peft). `ultra_detection_loss` — `data/utils.py` با `file_seed:unexpanded` و `models/nas/model.py`.

## S1 — کش ایندکس در harness + F63 (session 12، PR #90)

**کش:** `tests/support/index_cache.rs` — گراف هر checkout یک بار ساخته و در `target/third_party_index_cache/<set>/<name>-<rev>-<hash>.bin`
ذخیره می‌شود؛ کلید = rev گیت checkout + هش سورس چهار crate گراف‌ساز (core/parser/index/graph). تغییر در
`neuromesh-context` (seed/fold/selection) کش را نگه می‌دارد — همان بخشی که بدون کش هم بدون تغییر اجرا می‌شد.
`NM_INDEX_CACHE=0` خاموشش می‌کند. large: ۴۱۶s → ۱۵۱s (بقیه‌ی ۱۵۱s = build + load ۷۰MB + ۲۰ activation).

**F63 (باگ واقعی که کش لو داد):** اولین مقایسه‌ی fresh/cached روی large *یکسان نبود* — packet fresh امضای
`async def asend()` را دو بار و `sync_send` را سه بار چاپ می‌کرد. علت: در `insert_indexed_node` هر تعریف
هم‌نام در یک فایل (django `Signal.send` دو `asend` در دو شاخه‌ی if دارد) همان id را دوباره به `file_to_nodes`/
`name_to_nodes`/`impl_index` push می‌کرد؛ mesh یک نود نگه می‌دارد ولی `nodes_in_file` همان span را ۲–۳ بار می‌داد.
گراف بارشده از snapshot (سرور MCP بعد از ری‌استارت، کش harness) index ها را از نو و بدون تکرار می‌ساخت → دو
مسیر با هم اختلاف داشتند. رفع: `push_unique`. همچنین `rebuild_indexes` توکن‌های stem فایل را (که `ingest_file`
ایندکس می‌کند) از دست می‌داد → اضافه شد. حالا fresh == cached روی large خط‌به‌خط، به‌جز `ultra_predict_stream`
که *بین دو اجرای fresh هم* مجموعه‌ی `val.py` های هم‌نام‌ش فرق می‌کند (nondeterminism قدیمی، precision/strict ثابت؛
ثبت برای D1).

**اثر F63 روی اعداد (اندازه‌گیری صادقانه‌تر، نه رگرسیون موتور):**

| set | قبل | بعد | چرا |
|---|---|---|---|
| holdout-c | 0.656 / strict 12 | **0.573** / strict **14** | fmt: ۴ تسک فایل `format.h`/`base.h` اضافه می‌گیرند. با explain: `format.h` با `utility:18.00` = callee با stem-focus. عدد قدیمی از تکرار idها در index (نسبت‌های per-file رقیق) سود می‌برد؛ سرور MCP بعد از ری‌استارت از قبل رفتار جدید را داشت |
| holdout-ml | 0.365 / strict 9 | 0.370 / strict **10** | — |
| بقیه‌ی ۶ مجموعه | — | بدون تغییر | — |

تلاش برای بازگرداندن holdout-c با حذف «قطعه‌های stem فایل نام‌برده» از focus terms (`format-inl.h` → `format`) در
سه جای activator: **بی‌اثر** (0.573 ثابت) → برگردانده شد؛ منشأ term `format` هنوز پیدا نشده. طبق اصل holdout
بیش از این روی holdout-c کار نشد. گیت target-only ی holdout-c (≥0.60) الان قرمز است — صادقانه ثبت شد.

**S2 — ابزار probe دسته‌ای:** `NM_EXPLAIN=1` (+ `NM_EXPLAIN_MAX_PRECISION`) در همه‌ی harnessهای مشترک →
`target/explain-<set>.txt`: هر تسک کم‌precision با فایل‌ها (reason/sidecar/tokens)، ✓/✗ نسبت به gold، و seedها
(query → node @ file). دیگر eprintln موقت لازم نیست.

## F64 — twin هم‌نام: بدنه‌ای که prompt توصیف می‌کند، و پوشه‌ای که prompt نام می‌برد (session 12، PR #91)

اولین خروجی ابزار explain روی large: `django_template_render` recall 0 — `Template`/`Template.render` به
`template/backends/django.py` (wrapper نازک) می‌رفت نه `template/base.py`. `twin_cohere` دو سیگنال داشت (چند seed
در یک فایل، stem فایل در prompt) و هر دو *مساوی* بودند → tie → ranking per-symbol (اندازه‌ی بدنه/degree) برنده.
دو سیگنال جدید، فقط برای شکستن tie بین فایل‌هایی که از قبل واجد شرایط‌اند (guard قبلی دست‌نخورده):
1. **کلمات پوشه**: جزء مسیرِ نام‌برده در prompt («On Unix» → `src/unix/…` نه `src/win/…`).
2. **کلمات بدنه**: تعداد کلمات متمایز prompt (≥۴ حرف، نه نام خود seed) در بدنه‌ی twinهای آن فایل — «compile a
   parsed node list into rendered output» فقط در بدنه‌ی `Template.render` واقعی هست.
ترتیب: together → stem → dir → body. نسخه‌ی اول فقط body داشت و روی holdout-c یک تسک libuv را از `unix/` به
`win/` برد (بدنه‌ی win طولانی‌تر = کلمات بیشتر) → dir قبل از body قرار گرفت؛ holdout-c به عدد قبل برگشت.

| set | قبل | بعد |
|---|---|---|
| large | 0.950 / 0.541 / 19/20 strict 19 | **1.000 / 0.566 / 20/20 strict 20** |
| dev-4 | 1.000 / 0.906 | 1.000 / **0.921** |
| ۶ holdout | — | بدون تغییر |

ratchet large: recall 0.99، precision 0.54، reachable 1.0.

## F62 — کلیدواژه‌های حدسیِ سرور با tier قوی؛ concept-seed ی که identifier می‌شد (session 12، PR #92)

از dogfooding (اولین سؤال واقعی Parsa روی همین ریپو): «How does prune_off_family_weak_seeds drop a weak seed that
landed in a style asset?» → علاوه بر فایل درست، `docs/assets/i18n.js` (`applyStatic`) و `docs/index.html` (`stats`)
به‌عنوان seed آمدند. با trace روی مسیر CLI/MCP (`neuromesh packet --json --query …`) دو منشأ:

1. **`auto_extract_keywords`** (MCP/CLI و همین‌طور `production_signature` ی harness): سرور از prompt کلیدواژه حدس
   می‌زند («style asset» → intent pack اکسپرس: `express.static`، `static`، `stat`) و آن‌ها را در `client_keywords`
   می‌گذارد؛ `push_client_keywords` همه را با reason `client_keyword` = **tier قوی** push می‌کرد، یعنی هیچ‌کدام از
   pruneهای seed ضعیف (noise path، off-family، style asset) به آن‌ها نمی‌رسید. `stat` → `<div id="stats">` در سایت
   مستندات. رفع: پرچم `client_keywords_inferred` روی `TaskSignature` (فقط وقتی سرور پر کرده، نه وقتی client داده)
   → reason `inferred_keyword` در لیست WEAK.
2. **مسیر tiered (`activate_tiered` → L1)** که MCP/CLI استفاده می‌کنند و harness نه: `resolve_concept_seeds` («static»
   → `applyStatic` از concept index) نتیجه را در `sig.identifiers` **ارتقا می‌داد** — قوی‌ترین tier، به‌طور
   ساختاری از هر prune مصون. رفع: به `related_concepts` (tier concept) می‌رود. بسته‌ی dogfood: ۲۶۹۳ → ۸۴۱ توکن،
   فقط `lang_cohere.rs`.

اثر روی harness (چون `production_signature` هم auto-extract می‌کند، بند ۱ همه‌جا اثر داشت):

| set | قبل | بعد |
|---|---|---|
| dev-4 | 0.921 | **0.938** |
| large | 0.566 | **0.574** |
| holdout-2 | 0.538 | **0.554** |
| holdout-lang | 0.536 | **0.541** |
| holdout-ml | 0.370 | **0.437** |
| holdout-cfg | 0.610 | **0.690** |
| holdout-c، holdout-ml2 | — | ثابت |

recall/forbidden/strict همه‌جا ثابت. ratchet: dev 0.92، large 0.55. **درس:** مسیر MCP/CLI (tiered) و مسیر harness
(`activate`) یکی نیستند — یک باگ فقط-MCP از هیچ بنچمارکی دیده نمی‌شد؛ dogfooding آن را در اولین سؤال داد.

## F55 — کلید config بی‌hint: «the X key/flag» و `_target_` (session 12، PR #93)

`hydra_compile_flag` (recall 0.5): «How does the compile key in the model config make MNISTLitModule.setup call
torch.compile?» — `configs/model/mnist.yaml` نمی‌آمد. سه لایه با trace روی همین تسک (اعتراف: probe روی holdout-cfg؛
از این‌جا به بعد این مجموعه برای سؤال‌های config dev-class است، مثل holdout-ml):

1. یال `Parameterizes` برای `self.hparams.compile` به `configs/experiment/example.yaml` (override) می‌رفت نه `model/mnist.yaml`:
   `resolve_config_key` بین دو فایل هم‌کلید dominant را می‌گرفت. رفع: `resolve_config_key_for_reader` — فایلی که
   مقادیرش نام owner/ماژول reader را می‌برد (`_target_: src.models.mnist_module.MNISTLitModule`) برنده است؛ فقط
   وقتی دقیقاً یک فایل چنین باشد. reader فقط نود کد (نه Config/File).
2. حتی با یال درست، فایل config وارد packet نمی‌شد: `spread_energies` یال Parameterizes را فقط کلید→خواننده می‌پیماید
   (تصمیم D-۴ برای جلوگیری از hub). پس *کلید* باید seed شود. «the compile key» → `config_key_mentions`: کلمه‌ی قبل از
   key/flag/option/setting/parameter/argument یا بعد از `--` → seed با reason `config_key` (STRONG) فقط روی نودهای
   Config/Hyperparameter، با همان resolver reader-محور.
3. extractor از «torch.compile» identifier برهنه‌ی `compile` می‌ساخت که *به فایل درست* resolve می‌شد ولی `bare_owner`
   (F28) آن را به‌عنوان member ی seed نقطه‌دار حذف می‌کرد. رفع: وقتی prompt همان کلمه را «X key» گفته، seed موجود به
   `config_key:` retag می‌شود تا F28 آن را نگیرد.

| set | قبل | بعد |
|---|---|---|
| holdout-cfg | recall 0.879 / 0.690 | recall **0.924** / 0.690 (گیت ≥0.90 پاس) |
| ۷ مجموعه‌ی دیگر | — | بدون تغییر |

باقی‌مانده‌ی cfg: `hydra_early_stopping` (کلید در دو فایل: `early_stopping.yaml` تعریف با `_target_`، `default.yaml`
مقداردهی؛ گلد دومی را می‌خواهد — نیازمند فهم ترکیب `defaults:` هایدرا) و `detr_masks_flag` (`datasets/coco.py` که
`args.masks` را می‌خواند؛ seed `masks` هست ولی خواننده‌ی دوم وارد نمی‌شود).

## F65 — کلمه‌ی برهنه به فایل markdown با پیشوند (session 12، PR #94)

`ultra_model_train_api` («… from the high-level Model API?»): identifier `API` → `search_symbols` → نود *File*
`docs/en/datasets/explorer/api.md` با قاعده‌ی پیشوند (`api.md`.starts_with(`api`)، score ≥86) → seed با
`file_seed:unexpanded` و ۳۷۰۰ توکن markdown. قاعده: کلمه‌ی برهنه یک query ی symbol است؛ نود File در مسیر noise
(md/rst/txt/docs) که فقط با آن شروع می‌شود anchor نیست — فایل را با نام (`api.md`) یا stem می‌خواهند.
large 0.574 → **0.578**؛ بقیه بدون تغییر.

## F66/F67 — skeleton پایتون: header کلاس گم‌شده، `}` سرگردان، docstring های باز (session 12، PR #95)

D4 (اندازه‌ی packet) با dump یک packet واقعی ultralytics شروع شد (`ultra_model_predict_api`، ۱۱.۸k توکن). سه چیز
دیده شد که هیچ متریک فایل‌سطحی نشان نمی‌دهد:

| # | مشکل | ریشه | رفع |
|---|---|---|---|
| **F66a** | `class YOLOE(Model):` در packet نبود؛ به‌جایش fold ماژول + یک خط docstring سرگردان («set_vocab: Set vocabulary and class names for the YOLOE model.») | `enclosing_header_line` خطی را header می‌گرفت که *شامل* `class ` و نام owner باشد — آن خط docstring هر دو را داشت | keyword نوع باید بلافاصله با نام owner به‌عنوان identifier بعدی بیاید (`impl Trait for Owner` هم) |
| **F66b** | `}` تنها زیر متدهای fold شده‌ی پایتون و یک `}` بعد از هر کلاس | `is_block_closer` و بستنِ گروه کلاس برای زبان indent-محور اعمال می‌شد (`}` در پایتون پایان dict literal است) | `is_indent_language` (py/pyi/ipynb/yaml/yml) → بدون closer |
| **F67** | ۳۸٪ خطوط فایل seed در packet docstring های Google-style بود | بدنه‌ی باز کامل چاپ می‌شد | docstring ی که بدنه‌ی باز را شروع می‌کند و >۳ خط است: خط خلاصه + marker fold (`<name>.__doc__`، قابل expand) + بستن رشته |

| | قبل | بعد |
|---|---|---|
| مجموع توکن packet های large (۲۰ تسک) | 123,913 | **115,535** (−۶.۸٪) |
| dev-4 | 25,435 | 25,274 |
| recall/precision/forbidden | — | ۷ مجموعه یکسان؛ holdout-ml 0.437→0.432 (−0.005: packet کوچک‌تر → fill یک فایل بیشتر جا داد؛ زیر آستانه‌ی ۰.۰۲)، holdout-2 strict 17→**18** |

اثر اصلی کیفی است: مدل حالا نام کلاس‌ها را می‌بیند و متن پایتون معتبر (بدون `}`) می‌گیرد — چیزی که task-success
با مدل واقعی (فاز C) حس می‌کند، نه بنچمارک فایل‌سطحی.

## F69 — حلقه‌ی یادگیری (بعد از F57): feedback مثبت مضر، feedback منفی بی‌اثر (session 12، PR #97)

harness جدید `tests/learning_loop.rs`: برای هر تسک گلد، packet اول → feedback مثل `neuromesh_record_feedback` → packet دوم.
دو جهت، هر کدام روی گراف تازه: **مثبت** (فایل‌های گلد «مفید») و **منفی** (فایل‌های اضافه‌ی packet «نامفید»).

| اندازه‌گیری | dev مثبت | dev منفی | large مثبت | large منفی |
|---|---|---|---|---|
| قبل از فیکس | precision 0.938 → **0.676** (۹/۲۰ packet عوض؛ دو بار فایل forbidden وارد شد) | 0.583 → 0.583 (۰/۳) | — | — |
| بعد | 0.938 → 0.938 (۰/۲۰) | 0.583 → **0.667** (۱/۳) | 0.578 → 0.578 | 0.398 → **0.494** (۴/۱۴) |

ablation (هر مکانیسم جدا): فقط `reinforce_callee_edges` مقصر بود — *همه‌ی* یال‌های Calls/Imports/References/UsedBy ی
فایل لمس‌شده را تقویت می‌کرد؛ «این فایل مفید بود» = «کل همسایگی‌اش دفعه‌ی بعد بیاید». رفع: فقط یال‌های *بین* نودهای
لمس‌شده (مسیری که واقعاً طی شد). feedback منفی: هر دو مکانیسم قبلی (`suppress_penalized_optional`، `negative_penalty`)
فقط فایل‌های optional را می‌دیدند؛ فایل‌های اضافه‌ی واقعی required اند. رفع: فایل required ی که هیچ seed قوی (symbol/
file/config key نام‌برده در prompt) به آن resolve نشده و `base_relevance` اش < 0.9 است (یک feedback منفی) demote می‌شود.
فایل نام‌برده در prompt هرگز با feedback حذف نمی‌شود (vit_sincos/vit_recorder به همین دلیل ثابت ماندند — twin هم‌نام
strong-seed). ۸ مجموعه بدون تغییر.

## F70 — seed اسم خوشه‌ای غیرقطعی: `predictions` → `plot_predictions` در هفت `val.py` دوقلو (session 13)

`ultra_predict_stream` (large) در سه activation پشت‌سرهم سه جفت متفاوت `val.py` می‌آورد ({depth, validator}،
{depth, detect}، {detect, semantic}). ریشه در `resolve_cluster_noun_seeds` (activator.rs): امتیاز هر فایل در یک
`HashMap<path, …>` جمع می‌شد و `into_values()` + sort فقط روی امتیاز → بین هفت فایل با امتیاز مساوی (۸۷.۵۱، همه
`plot_predictions`، هیچ‌کدام بدون bonus مسیر/stem/اسم خواهر) `take(3)` به ترتیب map انتخاب می‌کرد. `search_symbols`
خودش از قبل ترتیب کامل داشت؛ این فراخوان آن را دور می‌ریخت.

دو فیکس: (۱) ترتیب کامل — امتیاز، سپس مسیر. (۲) قاعده‌ی «اسم بدون تمایز»: اگر ردیف بالای رتبه‌بندی ≥۳ فایل با
*یک* نام symbol (که فقط شامل اسم است، نه برابر آن) و امتیاز مساوی و بدون هیچ bonus باشد، اسم یک مفهوم است نه یک
جا؛ seed نمی‌شود. (`predictions`، همان خانواده‌ی F61.)

| مجموعه | قبل | بعد |
|---|---|---|
| large | 0.578 | **0.583** (`ultra_predict_stream` 0.17→0.25، دیگر بین اجراها فرق نمی‌کند) |
| dev | 0.938 | 0.938 |
| ۶ holdout دیگر | — | بدون تغییر (recall/forbidden/strict همه ثابت) |

ratchet large 0.55 → 0.56.

## F71 — خواننده‌ی دوم کلید config وارد نمی‌شد؛ F72 — ترکیب `defaults:` هایدرا (session 13)

**F71 (`detr_masks_flag`، cfg):** یال‌های `Parameterizes` از `masks` (Hyperparameter در main.py) به هر پنج خواننده وجود
داشت (`build` در coco.py/coco_panoptic.py/detr.py، `build_backbone`، `main`) — probe با یک تست موقت روی گراف. اما
`select()` فقط برای Calls «صندلی callee» می‌دهد؛ خواننده‌های یک کلید همه با انرژی مساوی در fill می‌ماندند و هیچ‌کدام
بالا نمی‌آمد. قاعده‌ی جدید: خواننده‌های یک seed از نوع Config/Hyperparameter مثل callee اند، *فقط* وقتی prompt
stem فایلشان را نام برده باشد («the coco dataset builder» → `coco.py`). نام خود خواننده کافی نیست: `build` هم کلمه‌ی
prompt است و هم نام همه‌ی builderها — نسخه‌ی اول با آن coco_panoptic.py و detr.py را هم آورد (0.67→0.60)، با stem-only
0.75.

**F72 (`hydra_early_stopping`، cfg):** کلید `early_stopping` در دو فایل: `early_stopping.yaml` (schema، `_target_`) و
`default.yaml` که با `defaults: - early_stopping` روی آن سوار می‌شود و monitor/patience را *مقدار می‌دهد*. گراف هیچ
یالی بین دو فایل نداشت. سه تغییر: (۱) yaml parser آیتم‌های `defaults:` را Imports می‌کند (`- x` → `dir/x.yaml`،
`- group: x` → `dir/group/x.yaml`، `override`/`_self_`/`null`/`@`/`${}` رد می‌شوند). (۲) linker: import هرگز به
فایل import‌کننده نمی‌رسد — `resolve_ranked` کلید هم‌نام *خود* default.yaml را ترجیح می‌داد؛ حالا به file hint
می‌افتد → یال DependsOn به فایل درست. (۳) `select()`: اگر فایل B کلید هم‌نام seed را دارد و به فایل seed
Imports/DependsOn دارد، B composer است و صندلی می‌گیرد (score 18). نکته: ورودی composer باید `stem_focus=true`
باشد وگرنه قاعده‌ی «callee هم‌نام term ی که stem اش required است» (برای `build`/`build.py`) آن را می‌انداخت.

| مجموعه | قبل | بعد |
|---|---|---|
| holdout-cfg (dev-class) | recall 0.924 / precision 0.690 | recall **1.000** / precision **0.712** (`detr_masks_flag` 0.67→0.75، `hydra_early_stopping` 0.50→0.67) |
| ۷ مجموعه‌ی دیگر | — | بدون تغییر (dev 0.938، large 0.583، holdout-2 0.554، -c 0.573، -lang 0.541، -ml 0.432، -ml2 0.632) |

ratchet cfg: recall 0.90→0.95، precision 0.60→0.69 (margin −0.02).

## F68 — `@nm:seeds` در micro-header seedهای prune‌شده را نشان می‌داد (session 13)

header در `activate_inner` *قبل* از زنجیره‌ی prune ساخته می‌شد (bare_owner، off-family، data-seed، weak file، weak
substring، style noise). تست جدید `micro_header_lists_only_seeds_that_survived_pruning`: prompt «How does Trainer.run
compute the loss for a token?» روی دو فایل؛ `token` با prefix search به `Tokenizer` در `data/tokenizer.py` می‌رسد و کنار
seed قوی `Trainer` prune می‌شود — روی کد قبلی header آن را اعلام می‌کرد و packet نمی‌فرستاد (تست روی main قبلی قرمز،
با فیکس سبز). فیکس: تولید header بعد از آخرین prune و درست قبل از `seed_set`. تغییری در انتخاب فایل‌ها نیست؛ ۸ مجموعه
بدون تغییر.

## F73 — جدول alias با `contains` خام: «in**validate**» = خوشه‌ی validation (session 13، dogfood)

اولین dogfood فورک روی خود ریپو: «How does the index cache in the third-party harness decide when to invalidate?» →
packet فقط `tests/fixtures/mini-pinoox/Controller/MainController.php`. JSON نشان داد `client_keywords` =
validation/schemas/schemaController/Validator/validate/schema/Validierung — از هیچ‌جای prompt. ریشه: هر سه matcher در
`retrieval/alias.rs` (`prompt_has_alias_cluster_match`، `expand_aliases`، `alias_code_seeds_inner`) با
`lower.contains(term)` کار می‌کردند؛ «invalidate» ⊃ «validate». همان خانواده‌ی F61 (زیررشته)، روی مسیر alias.

فیکس: `term_in_prompt` — term باید در *ابتدای* یک کلمه باشد (کاراکتر قبلش alphanumeric نباشد)؛ صرف‌ها همچنان match
(`validates`، `sessions`، `authentication`←`auth`). تست واحد `alias_term_must_start_a_word`.

| مجموعه | قبل | بعد |
|---|---|---|
| large | 0.583 | **0.641** (`django_management_command` 0.50→1.00 — «Command**Error**» خوشه‌ی error را فعال می‌کرد؛ `ultra_base_model_forward` 0.33→1.00) |
| holdout-ml (dev-class) | 0.432 | 0.437 |
| ۶ مجموعه‌ی دیگر | — | بدون تغییر |

ratchet large 0.56 → 0.62. درس: **dogfood با سؤال‌های واقعی روی ریپوی خودمان یک finding در پنج سؤال داد** و آن
finding روی large ‎+0.058 آورد — بیش از هر تیون امتیازی در دو session اخیر.

## F74 — دو کلمه‌ی prompt که stem یک فایل را می‌سازند: «index cache» → `index_cache.rs` (session 13، dogfood)

همان سؤال F73 بعد از فیکس alias هنوز `tests/fixtures/mini-pinoox/Controller/MainController.php` را می‌آورد (identifier
`index` → متد `index()` با exact-name) و `tests/support/index_cache.rs` را نه. extractor فقط `index` را identifier می‌گیرد؛
هیچ مکانیزمی دو کلمه‌ی پشت‌سرهم را با stem فایل مقایسه نمی‌کرد.

قاعده (`push_compound_stem_seeds`، کنار path hints): هر جفت کلمه‌ی *مجزای* prompt که به‌هم‌چسبیده برابر stem یک فایل با
`_`/`-` باشد (یکتا در ریپو، ≥۶ حرف) → file seed با وزن path_hint؛ و نیمه‌های bare (`identifier:index`) حذف می‌شوند —
همان قاعده‌ی bare_owner برای `owner.member`. **نسخه‌ی اول holdout-2 را 0.554→0.552 انداخت** (`vision_generalized_rcnn`):
`roi_heads` ی prompt روی `_` شکسته و دوباره به `roi_heads.py` چسبیده شد و آن فایل از sidecar به required رفت. سیگنال
گم‌شده: identifier یک‌تکه (`roi_heads`) یک کلمه است نه جفت؛ جای خودش را نگه می‌دارد تا همسایه‌هایش جفت نشوند. بعد از آن
۸ مجموعه بدون تغییر؛ Q4 دقیقاً `index_cache.rs`. تست `compound_stem_names_the_file_and_drops_the_bare_halves` روی کد
قبلی قرمز.

نکته‌ی ثبت‌شده، فیکس‌نشده: `tests/fixtures/` به‌عنوان testdata شناخته نمی‌شود (`is_testdata_path` فقط `testdata`/
`test_data`)؛ افزودنش امتحان شد ولی برای این سؤال اثری نداشت و برای یک finding جدا نگه داشته شد.

## F75 — dogfood روی خود ریپو، ۱۰ سؤال: recall 0.600 / precision 0.325 (session 13، ثبت؛ فیکس‌نشده)

مجموعه‌ی `tests/third_party/self/` (۱۰ سؤال واقعی درباره‌ی همین ریپو، گلد = فایلی که پرسنده انتظار داشت). عدد روی main
بعد از F74: **recall 0.600، precision 0.325** — بدتر از هر holdout. ریپوی Rust چند-crate هیچ‌وقت در گلدها نبود. خوشه‌های
ریشه از dump:

| خوشه | نمونه | مکانیزم |
|---|---|---|
| **A. identifier ای که string literal است** | `neuromesh_record_feedback` → unresolved (فقط در `"neuromesh_record_feedback" =>` در `mcp/src/tools.rs` است) | هیچ ایندکس متنی از بدنه‌ی فایل‌ها نیست؛ نام ابزار/route/event/env-var هرگز resolve نمی‌شود. نیازمند ایندکس توکن‌های بلند snake_case در زمان index (feature، نه فیکس) |
| **B. seedهای fallback در مسیر نویز/تست** | `token:learning` → `docs/index.html`، `token:feedback`/`packet` → `tests/learning_loop.rs`، `token:apply` → `overlay.rs` | وقتی هیچ identifier ای resolve نشود، `lexical_fallback` هر کلمه‌ی prompt را به هر symbol ای می‌چسباند، حتی در `/docs/` و `tests/` |
| **C. acronym → prefix** | `identifier:CLI` → `cli_request_id`، `identifier:MCP` → `McpServer` | همان خانواده‌ی F61 (acronym fragment) |
| **D. کلمه‌ی prose که فقط پیشوند stem است** | `skeletonizer` → unresolved در حالی که `skeleton.rs`/`CodeSkeletonizer` هست | F65 فقط برای `.md` با پیشوند کار می‌کند |
| **E. «eval» → artifact overlay ML** | `artifact:EvalLoop→evaluate` در `tests/fixtures/ml-*` برای سؤال CLI eval | overlay ML روی fixtureهای تست فعال می‌شود |

آزمایش رد‌شده: تعمیم F74 به symbol (`packet cap` → `packet_cap()`): self recall 0.60→0.55 / precision 0.325→0.417 —
مخلوط؛ «python docstring» → `python_docstring()` در parser، seed دقیق ولی غلط، و چون یک seed resolve شد fallback ی که
تصادفاً `skeleton.rs` را می‌آورد خاموش شد. revert شد. درس: یک seed «دقیق» جای چند seed ضعیف را می‌گیرد — قاعده‌ی جدید
باید از قاعده‌ی قبلی که جایش را می‌گیرد *بهتر* باشد، نه فقط دقیق‌تر.

ترتیب پیشنهادی: B (کوچک، عمومی: fallback در نویز/تست ممنوع مگر prompt بگوید test/docs) → A (feature: ایندکس literal) → D → C/E.

## فاز C — اجرای دوم (2026-09-20، main بعد از #105) + F76 header بی‌شماره‌ی hunk

مسیر: Baseten، `deepseek-ai/DeepSeek-V4-Pro` (reasoning) پاسخ‌دهنده، `zai-org/GLM-5.3` داور. **تله‌ی شبکه:** Baseten
(Cloud Armor) به exit های VPN آذربایجان 403 می‌دهد حتی بدون کلید؛ فقط با exit اروپا/آمریکا 401/200. جدول کامل در
`docs/measured.md`. خلاصه: هر ۹ سلول هر سه گیت را پاس می‌کنند؛ holdout-vision/packet 0.80→**1.00** (strict 1.00)؛ packet
با ۱.۴–۲.۱× کارایی توکن بهتر از فایل‌های کامل.

**F76 (harness):** ۲ از ۵ شکست packet در fixtures (`orders_total_with_tax`، `orders_remove_item_underflow`) patch درست
داشتند ولی header را `@@ ... @@` نوشته بودند → `git apply --recount` هم «garbage» می‌گوید. فیکس: `number_hunk_headers`
اولین خط old-side هر hunk بی‌شماره را در فایل پیدا و header را می‌سازد؛ header شماره‌دار دست نمی‌خورد. تست واحد. اجرای
مجدد دو سلول به 402 (اعتبار تمام) خورد — عدد جدول pre-F76 است؛ انتظار: fixtures/packet 0.808→~0.885.

**عیب واقعی packet (باز):** `rs_router_handle` با whole-gold-files پاس، با packet fail: بدنه‌ی `extract_route` fold شده و
مدل مکانیزم را حدس زد. probe بعدی: چرا fold policy بدنه‌ی callee ای را که prompt نام برده تا می‌زند (seed_callee_exon_names
باید نگهش می‌داشت).

**هزینه:** ۹ سلول + ۲ سلول ناتمام، اعتبار Baseten تمام شد — مدل reasoning (Pro) چند برابر Flash توکن خروجی می‌سوزاند؛
برای اجراهای بعدی Flash کافی و ارزان‌تر است (نتایج ۱۵ سپتامبر با Flash هم گیت‌ها را پاس کرد).

## F75 (فیکس) + F77 — batch «همه با هم» بعد از G2 (session 13، PR #108)

Parsa: «همه‌ی اشکال‌ها را با هم و سریع». یک PR، شش تغییر، هر کدام با دلیل از dump:

| # | تغییر | کجا |
|---|---|---|
| F77 | داور QA سورس واقعی فایل‌های `needs` را می‌بیند و «علیه کد واقعی، نه کد معمول» می‌سنجد. `rs_router_handle` در G2 خطای داور بود: `extract_route` واقعاً یک خط `trim` است | `cli/commands/tasks.rs` |
| F75-B | seedهای حدسی (`token`، `fallback:*`، `concept`، `client_expansion`، `alias_gap_fill`) که به مسیر نویز/تست/fixture می‌رسند رد می‌شوند، مگر prompt درباره‌ی test/docs باشد. باگ جانبی: `is_noise_path` مسیر ریشه‌ای `docs/index.html` را نمی‌گرفت (`/docs/` بدون `/` اول) | `seed/sink.rs`، `selector.rs` |
| F75-C | acronym (`CLI`، `MCP`) فقط به نام دقیق، stem فایل، یا *type* ای که با آن شروع می‌شود (`McpServer`، `SmsStore`) می‌رسد؛ نه به تابع با prefix (`cli_request_id`). نسخه‌ی اول type را هم رد می‌کرد و fixture kotlin (`SMS` → `SmsReceiver`) را شکست | `seed/sink.rs` |
| F75-D | کلمه‌ی prose که با stem یک فایل غیرنویز شروع می‌شود (`skeletonizer` → `skeleton.rs`، یکتا، پسوند ≤۴ حرف) file seed می‌شود | `activator_seed.rs` |
| F75-A | **ایندکس literal:** هر رشته‌ی داخل کوتیشن با ≥۲ جداکننده (`"neuromesh_record_feedback"`) یا route (`"/api/v1/users"`) → فایل؛ در `GraphData.literal_index`، در snapshot persist می‌شود، خطوط comment و backtick شمرده نمی‌شوند. identifier بدون symbol → فایل‌های غیرنویز (≤۸)، رتبه‌بندی با echo کلمات prompt در path/symbolها، ۴ تای اول seed (`literal:` = STRONG). نکته: cap کل seed = ۵ (`max_resolved_seeds`) بود که `tools.rs` را (آخرین در ترتیب ingest) می‌انداخت → رتبه‌بندی لازم شد | `graph/intern.rs`، `graph.rs`، `activator_seed.rs` |
| — | compound symbol (`packet cap` → `packet_cap`) فقط در fallback و *کنار* حدس‌ها؛ `tests/fixtures/` = testdata | `seed/mod.rs`، `core/source_path.rs` |

| مجموعه | قبل | بعد |
|---|---|---|
| self (۱۰ سؤال، dev-class) | recall 0.600 / precision 0.325 | **0.750 / 0.449** |
| ۸ مجموعه‌ی گلد | — | بدون تغییر |

باقی‌مانده‌ی self: `self_eval_judge` (هیچ نامی: «CLI eval command» → `commands/tasks.rs`؛ نام دستور CLI به فایل map نمی‌شود)،
`self_negative_feedback` («learning loop» → `tests/learning_loop.rs` به‌جای activator — گلد قابل بحث)، `self_packet_cap`
(fallback شلوغ). درس تازه: **cap کل seed (۵) با seedهای فایل‌محور (literal) می‌جنگد** — ترتیب ingest نباید سرنوشت را تعیین کند.

## F78 — دور دوم dogfood (۱۰ سؤال جدید، self = ۲۰ سؤال): اسکریپت‌ها ایندکس نمی‌شدند (session 13، PR #109)

۱۰ سؤال جدید روی خود ریپو با engine بعد از #108: recall 4/10. خوشه‌ها و فیکس‌ها (یک PR):

| عیب | فیکس |
|---|---|
| **`.sh`/`.ps1` اصلاً ایندکس نمی‌شد** («How does the benchmark-holdout script…» → `benchmark_suite.rs`؛ «phase-c-run script … clean» → `site.css`) | زبان `Shell` (sh/bash/zsh/ps1) + `ShellParser` (توابع `name() {`، `function name`)؛ walker آن را code می‌شمارد |
| kebab token (`benchmark-holdout`) توسط extractor نصفه می‌شد (`benchmark`) | kebab token = stem فایل (یکتا) → file seed. **فقط kebab**: نسخه‌ی با `_` هم `roi_heads` را در holdout-2 به فایل تبدیل کرد (0.554→0.552) — snake token مال مسیر symbol است |
| `NM_EXPLAIN` (env var، یک `_`) در literal index نبود | قاعده‌ی env-var: تمام‌حروف‌بزرگ + یک `_` کافی است |
| literal فقط در فایل‌های تست/اسکریپت (explain.rs زیر `tests/support/`) به‌خاطر فیلتر نویز صفر می‌شد | نویز فقط وقتی فیلتر می‌شود که فایل غیرنویزی هم literal را داشته باشد |
| F75-E: artifact ML در `tests/fixtures/ml-*` برای «positive feedback» | artifact seeds مسیر نویز (با احترام به `examples_are_core`) را رد می‌کنند مگر prompt fixture/example بگوید |

| مجموعه | قبل | بعد |
|---|---|---|
| self (۲۰ سؤال) | recall 0.525 / precision 0.306 | **0.675 / 0.406** |
| ۸ مجموعه‌ی گلد | — | بدون تغییر |

باقی‌مانده‌ی self (recall 0): `self_large_ratchet` («large gold set» ≠ stem `third_party_large_gold`)، `self_argparse_flags`
(`add_argument` فقط داخل regex)، `self_exon_budget`، `self_pheromone_reinforce` («reinforced» ↔ `reinforce_path`؛ prefix
symbol چند‌معنی)، `self_eval_judge`. همه «مفهوم بدون نام» اند — نیازمند embedding/semantic یا گلد بحث‌برانگیز.

## F79 — آیا embedding (MiniLM، engine hybrid) مفهوم‌های بی‌نام را می‌گیرد؟ نه — اندازه‌گیری شد (session 13، PR #110)

پنج سؤال self با recall 0 همه «مفهوم بدون نام» بودند («ratchet»، «exon budget»، «reinforced» ↔ `reinforce_path`). فرض:
engine hybrid با MiniLM آن‌ها را می‌گیرد. اندازه‌گیری روی همان ۲۰ سؤال، همان گراف، با ایندکس embedding واقعی
(`NM_EMBED=1` در harness — هوک جدید و اختیاری در `tests/support/gold_set.rs`، `--features embeddings`؛ مدل
`minilm-multilingual-q` که `install embed` به‌خاطر شبکه شکست خورد و دستی در `%LOCALAPPDATA%\neuromesh\models\` گذاشته شد):

| engine | recall | precision | forbidden |
|---|---|---|---|
| fast (پیش‌فرض) | **0.675** | **0.406** | 0 |
| hybrid بدون ایندکس embedding (سکوت‌آمیز به مسیر لغوی می‌افتد) | 0.425 | 0.256 | 1 |
| hybrid با MiniLM واقعی | 0.500 | 0.122 | 1 |

هیچ‌کدام از ۵ سؤال مفهومی با embedding هم recall نگرفت؛ در عوض ۵ سؤالِ قبلاً درست، خراب شدند و یک forbidden آمد. **نتیجه:**
embedding به شکل فعلی اهرم بزرگ نیست — سرمایه‌گذاری روی آن (به‌عنوان جایگزین مسیر لغوی) توجیه ندارد. اگر روزی برگردیم،
فقط به‌عنوان *fallback برای سؤال بدون هیچ seed* و با گیت روی همین self set. تله: `NEUROMESH_ENGINE=hybrid` بدون ایندکس
embedding خطا نمی‌دهد و بی‌صدا بدتر می‌شود — در MCP هم همین است (کاربری که hybrid را روشن کند و index را با embed نساخته
باشد، packet بدتری می‌گیرد).

## F80 — holdout-web: شکل stack B2B (Fastify + Next.js app router)، ۲۰ سؤال (session 13، PR #111)

Parsa سؤال ندارد؛ به‌جای انتظار، دو ریپوی عمومی هم‌شکل با stack تیم (fastify/demo، shadcn-ui/taxonomy) با گلد کور (از خواندن
کد، قبل از هر اجرا) اضافه شد: `tests/third_party/holdout-web/`. **untuned: recall 0.542 / precision 0.402 / forbidden 1** —
بدترین recall بین همه‌ی مجموعه‌ها؛ یعنی روی همین نوع ریپو engine فایل گم می‌کند. چون روی آن فیکس شد، از همین لحظه
dev-class برای دامنه‌ی web است (گلد خصوصی تیم همچنان holdout واقعی).

خوشه‌ها و فیکس‌ها:
| عیب | فیکس |
|---|---|
| `route.ts` های هم‌نام (`PATCH` در `posts/[postId]` و `users/[userId]`): `concept:routing` به یکی از هفت `route.ts` تصادفی می‌رسید و به‌عنوان anchor، `PATCH` را به route غلط می‌کشید | anchor فقط از seed قوی یا مسیر نام‌برده؛ guess ای که به فایلی با stem مشترک (≥۲ فایل) برسد unresolved |
| «user route» ↔ `users/`، «webhook» ↔ `webhooks/` | twin_cohere: token های prompt/مسیر/بدنه با `strip_plural` |
| `stripe` → `STRIPE_API_KEY` در `.env.example` به‌جای `lib/stripe.ts` | فایل کد با stem == کلمه بر prefix Config-key مقدم (فقط وقتی همه‌ی hitها config-prefix اند؛ نسخه‌ی گسترده‌تر `physarum_usage` فیکسچر را شکست) |
| `onRequest`، `update-password`، `/login` در literal index نبودند | literal camelCase، kebab، route تک‌بخشی؛ kebab token ی که فایل نیست به‌عنوان route (`/update-password`) جست‌وجو می‌شود |
| «Stripe webhook» / «dashboard layout» بدون هیچ symbol | `push_path_word_seeds`: فایلی که ≥۲ کلمه‌ی prompt را در مسیرش دارد، با یک کلمه‌ی کم‌تکرار (≤۴ فایل). **فقط وقتی هیچ seed قوی نیست**: نسخه‌ی بدون این شرط web را به 0.742 می‌برد ولی holdout-c را 0.573→0.494 و cfg را 0.712→0.682 می‌انداخت (`loop-watcher.c` کنار `loop.c`)؛ guard «کلمه‌ی تازه» هم کافی نبود |
| نیمه‌ی bare یک identifier نقطه‌دار (`segmentsFromString`) به‌عنوان literal فقط فایل تست را می‌آورد | نیمه‌ی dotted از literal lookup مستثنا (holdout-lang 0.508→0.541 برگشت) |

| مجموعه | قبل | بعد |
|---|---|---|
| holdout-web | 0.542 / 0.402 | **0.642 / 0.502** (سقف با path-words بدون guard: 0.742 / 0.539) |
| self (۲۰) | 0.675 / 0.406 | 0.675 / 0.406 |
| ۸ مجموعه‌ی دیگر | — | بدون تغییر |

باقی‌مانده‌ی web: `fd_login_flow` (سه فایل گلد، packet یکی)، `tx_create_post_limit` (`lib/subscription.ts` از `getUserSubscriptionPlan`
callee نمی‌آید + forbidden `webhooks/stripe/route.ts` از twin `POST`)، `tx_stripe_checkout`/`webhook` (route های stripe بدون
نام symbol؛ path-words با anchor `lib/stripe.ts` خاموش است). درس: **قاعده‌ی «کلمات مسیر» با anchor نمی‌سازد** — راه درست
احتمالاً امتیازدهی کلمات مسیر *داخل* selection است نه seed جدید.

## F81 (ثبت، ship نشد) / F82 — دومین دور روی holdout-web (session 13، PR #112)

**F81 — یال Calls از تابع هم‌نام به فایل می‌چسبد:** در `finalize_links` منبع یک Calls با `resolve_unique(name)` پیدا می‌شد؛
وقتی `POST` در هفت `route.ts` هست، unique شکست می‌خورد و یال از *فایل* بیرون می‌رفت → callee های seed (`getUserSubscriptionPlan`)
هرگز صندلی نمی‌گرفتند. فیکس درست (`resolve_in_file` اول) recall web را 0.642→0.667 برد ولی **large 0.641→0.558 + forbidden،
dev 0.938→0.925** — گراف تماس درست، callee های بیشتری را زیر سقف ۳ صندلی می‌نشاند و قواعد focus برای آن تنظیم نشده‌اند.
revert شد؛ باز می‌ماند: قبل از F81 باید صندلی callee با «آیا prompt آن callee را نام برده» سخت‌گیرتر شود.

**F82 — stem قراردادی فریم‌ورک «نام‌برده» نیست:** فایل fill ی با stem مشترک بین ≥۳ فایل (`route.ts`) به‌خاطر focus «route»
sidecar می‌شد، از جمله forbidden. حالا فقط برای stemهای قراردادی (`route`، `index`، `page`، `layout`، `handler`، `controller`، …) و
فقط وقتی هیچ پوشه‌ی مسیرش focus term نباشد. نسخه‌ی عمومی (هر stem مشترک) `src/train.jl` ی flux را می‌انداخت (holdout-lang
recall 1.0→0.933) → whitelist قراردادی. یک قاعده‌ی دیگر («کلمه‌ای که نام پوشه است به symbol با prefix نمی‌رسد») web را به
0.767 می‌برد ولی تست kosha (پوشه‌ی `school/`) را می‌شکست — کنار گذاشته شد، ثبت برای بعد.

| مجموعه | قبل | بعد |
|---|---|---|
| holdout-web | 0.642 / 0.502 / forbidden 1 | 0.642 / **0.602** / **0** |
| large | 0.641 | **0.666** |
| ۷ مجموعه‌ی دیگر + self | — | بدون تغییر |

سقف token تسک `handle_tool_call_intent` (فیکسچر روی خود ریپو) 27.5k→28.5k: `activator.rs` خودش فایل گلد است و با هر PR بزرگ‌تر می‌شود.

## F83 — W1: صندلی callee فقط با شواهد prompt + F81 + یال member-call به شیء (session 14، PR W1)

**چرا F81 قبلاً large را می‌انداخت:** `select()` بدون هیچ شاهدی از prompt تا ۳ صندلی به callee می‌داد (فقط «≤۵ caller»).
تا وقتی یال Calls از تابع هم‌نام (`POST` در هر `route.ts`) به فایل می‌چسبید، این صندلی‌ها کم بودند؛ با گراف درست
(`resolve_in_file` قبل از `resolve_unique`) هر seed چند callee واقعی داشت و همه می‌نشستند (`setup_model` ی trainer برای
`Model.predict`؛ `update` ی `use-toast.ts` برای `db.user.update(`).

سه چیز با هم عوض شد (هر کدام تنها، مخلوط بود):
1. **شواهد prompt برای صندلی callee** (`selector.rs`): نام callee در prompt، یا stem فایلش، یا (`PromptWords`، کلمات
   *خود* prompt نه focus_terms که تکه‌های identifier و alias دارد): (الف) کل identifier با کلمات prompt هجی شده باشد
   (`create_access_token` ← «create the access token»)، (ب) کلمه‌ای که prompt وسط جمله با حرف بزرگ نوشته (`ItemsPublic` ←
   «how is Item linked»؛ F34 می‌گوید تنها seed نیست، ولی با یال Calls ی Proven شاهد است)، (ج) یک کلمه‌ی prose + فایل seed
   فایل callee را import می‌کند (`getUserSubscriptionPlan` ← «free plan» + `import … from "@/lib/subscription"`)، یا (د) کلمه‌ی
   stem/پوشه‌ی والد فایل callee (`tasks/`؛ پوشه‌های قراردادی `plugins/`, `lib/`, `src/` نه). کلمه‌ای که خود seed دارد
   (`Model.predict` → `model`) شاهد نیست. مسیر «بدون شاهد ولی ≤۵ caller» حذف شد.
   - نسخه‌ی «هر کلمه‌ی مشترک» (یک کلمه از prompt بلند): holdout-c 0.573→0.533، ml2 0.632→0.555 (`uv-common.c`، `signal.c`،
     `_buffer_dict.py` از تکه‌ی `state_dict`→«dict»). سیگنال گم‌شده: کلمه باید *نوشته‌ی کاربر* باشد و یا کل نام را بپوشاند
     یا با import ی صریح پشتیبانی شود.
2. **F81** در `finalize_links` (arm Calls): `resolve_in_file` قبل از `resolve_unique`.
3. **member-call روی شیء ساده** (`db.user.update(`، `userNameSchema.parse(`): parser (هر دو مسیر regex و tree-sitter)
   receiver_hint `obj:<root>` می‌دهد؛ linker اول خود شیء را در scope (همین فایل یا import شده) resolve می‌کند و یال به
   *تعریف شیء* می‌رود (`PATCH → userNameSchema @ lib/validations/user.ts`)؛ اگر شیء ناشناخته بود، تابع آزاد هم‌نام در جای
   دیگر فقط `Likely` است (صندلی نمی‌گیرد). dedupe یال‌ها per (caller, member, object) شد (`routeContextSchema.parse` قبلاً
   `userNameSchema.parse` را می‌خورد). `export const x = call/new/object` در TS حالا symbol است (قبلاً فقط SCREAMING_SNAKE؛ F54).

| مجموعه | قبل | بعد |
|---|---|---|
| holdout-web | 0.642 / 0.602 | **0.717** / 0.572 |
| large | 1.000 / 0.666 | 1.000 / **0.675** |
| holdout-2 | 0.554 | **0.700** |
| holdout-c | 0.573 | 0.578 |
| holdout-lang | 0.541 / forbidden 1 | 0.589 / 1 |
| holdout-ml | 0.437 | 0.478 |
| holdout-ml2 | 0.632 (strict 7) | 0.632 (strict 6) |
| holdout-cfg | 0.712 | 0.767 |
| dev | 1.000 / 0.938 | 1.000 / 0.938 |
| self (۲۰) | 0.675 / 0.406 | 0.675 / 0.428 |

web باقی‌مانده (recall): `fd_login_flow` 1/3، `fd_rate_limit` 1/2 (env.ts)، `fd_session_plugin` 1/2 (env.ts)، `fd_task_delete_image`
1/3 و `fd_task_upload` 2/3 (route ی `tasks/index.ts` ی caller)، `fd_update_password` 1/2 (`passwordManager.hash` روی
decorator ی fastify — شیء در scope نیست)، `tx_dashboard_guard` 0/2، `tx_stripe_*` (W2). precision web 0.602→0.572: صندلی
`lib/validations/post.ts` با stem «post» (قاعده‌ی قدیمی stem، حالا با یال جدید) در دو تسک.

**فیکسچر `handle_tool_call_intent`:** با حذف مسیر «بدون شاهد» activator.rs از packet افتاد (recall 0.67 < 0.8 gate).
بازگرداندنش با قاعده‌ی «import شده + ≤۵ caller» (بدون کلمه‌ی prompt) اندازه‌گیری شد: large 0.675→0.542 + forbidden، dev
0.938→0.925، holdout-2 0.700→0.679، cfg 0.767→0.712 — همان صندلی بی‌شاهد قدیمی. activator.rs «intent» را فقط در تست‌های
خودش که همین prompt را نقل می‌کنند دارد (همان حالت self-referential که F25 از `physarum_usage` حذف کرد)؛ استخراج intent در
`signature.rs` است و caller اش `tools.rs`. گلد اصلاح شد (۳→۲ فایل، دلیل در `tests/gold_tasks.toml`). ratchet ها: large 0.64→0.655،
cfg 0.69→0.745.
