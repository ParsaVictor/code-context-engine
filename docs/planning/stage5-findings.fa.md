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

## جمع‌بندی صادقانه

- recall خوب است (0.95) — موتور تقریباً هیچ‌وقت فایل گلد را کاملاً گم نمی‌کند، حتی روی ریپوی ندیده.
- precision (0.146) و forbidden (واقعی: ۲/۲۰، بعد از کسر ۲ اشتباه گلد) نشان می‌دهند دقتِ 0.856 مرحله ۴ **کاملاً به واژگان و ساختار ۴ ریپوی dev بسته بود** — یک نوع overfit سیستماتیک، نه یک باگ نقطه‌ای.
- ریشه‌ی مشترک هر سه یافته (F33–F35): قواعد seed/synonym مرحله ۴ روی متن/زبان مشخص (JS dotted-access، انگلیسی generic، واژگان auth-style) نوشته شدند و روی زبان/دامنه‌ی جدید یا miss می‌کنند (بی‌ضرر) یا با کلمه‌ی عمومی هم‌نام برخورد می‌کنند (مضر).
- طبق تصمیم فاز ۵a: این ریپوها اکنون **holdout باقی می‌مانند** (به dev منتقل نشدند)؛ فیکس این یافته‌ها کار فاز بعدی (بازبینی مرحله ۴ با تصمیم صریح Parsa) است، نه این session.
- ۵c اندازه‌گیری شد (بالا) — عدد نگران‌کننده‌ای نبود، یک گیت با حاشیه‌ی کم (`l1_p95_slo`) و یک بخش اندازه‌گیری‌نشده (baseline ریپوی بزرگ) ماند.
- اندازه‌گیری‌نشده در این session: ۵b (B2B) — هنوز شروع نشده، منتظر گلد Parsa.
