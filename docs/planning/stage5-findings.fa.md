# فاز ۵ — یافته‌ها (زنده)

طبق اصل holdout (`docs/planning/05-phases-holdout-to-release.fa.md`): این یافته‌ها فقط **ثبت** می‌شوند، فیکس نمی‌شوند —
مگر با تصمیم صریح که آن ریپو از holdout به dev منتقل شود. شماره‌ی بعدی بعد از این فایل: **F37**.

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
| F36 ✅ ریشه‌یابی‌شده (۱۴ سپتامبر، فیکس‌نشده) | ریشه‌ی دقیق پیدا شد: `activator.rs:453-466` هر کلمه‌ی ≥۵ حرفی **متن خام prompt** را (نه فقط identifierهای واقعی) به‌عنوان `focus_term` ثبت می‌کند — کلماتی مثل «incoming»، «reject»، «invalid» هم‌ردیف «CsrfViewMiddleware» می‌شوند. بعد `selector.rs:490-512` برای **هر** focus_term یک `graph.resolve_ranked(term, None, None)` مستقل و بدون هیچ محدودیت زمینه‌ای می‌زند و هر match را با gain ثابت **۳۶.۰** به optional_files اضافه می‌کند — بدون سنجش این‌که آیا آن کلمه واقعاً هدف سؤال بوده یا فقط یک فعل/صفت انگلیسی معمولی در جمله بوده. این دقیقاً مسیری است که svg/فایل تست/فایل بی‌ربط را با امتیاز بالا وارد packet می‌کند | `activator.rs:453` (تولید focus_terms از هر کلمه)، `selector.rs:490-512` (تبدیل بدون فیلتر هر focus_term به یک seed مستقل) | تأیید شده با خواندن کد مستقیم؛ **فیکس نشد** — طبق تجربه‌ی F35 (یک فیکس ساده سه لایه‌ی دیگر را باز کرد)، فیکس این باید با ~۱۵-۲۰ دقیقه تست کامل روی هر ۳ مجموعه انجام شود، نه عجولانه. برای session بعد: کاندید fix — همان معیار F34 (کلمه باید identifier-shaped باشد، نه هر کلمه‌ی انگلیسی ≥۵ حرفی) روی generation focus_terms در activator.rs:453-466، یا حداقل روی مصرف آن در selector.rs:490 |

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

## جمع‌بندی صادقانه

- recall خوب است (0.95) — موتور تقریباً هیچ‌وقت فایل گلد را کاملاً گم نمی‌کند، حتی روی ریپوی ندیده.
- precision (0.146) و forbidden (واقعی: ۲/۲۰، بعد از کسر ۲ اشتباه گلد) نشان می‌دهند دقتِ 0.856 مرحله ۴ **کاملاً به واژگان و ساختار ۴ ریپوی dev بسته بود** — یک نوع overfit سیستماتیک، نه یک باگ نقطه‌ای.
- ریشه‌ی مشترک هر سه یافته (F33–F35): قواعد seed/synonym مرحله ۴ روی متن/زبان مشخص (JS dotted-access، انگلیسی generic، واژگان auth-style) نوشته شدند و روی زبان/دامنه‌ی جدید یا miss می‌کنند (بی‌ضرر) یا با کلمه‌ی عمومی هم‌نام برخورد می‌کنند (مضر).
- طبق تصمیم فاز ۵a: این ریپوها اکنون **holdout باقی می‌مانند** (به dev منتقل نشدند)؛ فیکس این یافته‌ها کار فاز بعدی (بازبینی مرحله ۴ با تصمیم صریح Parsa) است، نه این session.
- ۵c اندازه‌گیری شد (بالا) — عدد نگران‌کننده‌ای نبود، یک گیت با حاشیه‌ی کم (`l1_p95_slo`) و یک بخش اندازه‌گیری‌نشده (baseline ریپوی بزرگ) ماند.
- اندازه‌گیری‌نشده در این session: ۵b (B2B) — هنوز شروع نشده، منتظر گلد Parsa.
