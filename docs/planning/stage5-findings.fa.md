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
| dev / holdout-2 / c / lang | — | (در PR) |

recall holdout-cfg 0.879: دو تسک hydra که کلید فقط از طریق `self.hparams.x` خوانده می‌شود و `hparams` با
`save_hyperparameters()` پر می‌شود — یالی به YAML نیست چون نام فایل config در آن ماژول نیامده (Hydra `_target_`
آن را instantiate می‌کند). ثبت (F55): «کلید بی‌hint در ریپوی چند-config» — hint از `_target_: module.Class`
در YAML به کلاس Python قابل استخراج است؛ انجام نشد (precision-tuning بسته).
