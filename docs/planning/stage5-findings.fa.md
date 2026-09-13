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
| F33 | پرسش‌های شکل `Owner.__call__` (متد dunder به‌جای عضو معمولی): مسیر owner-qualified که F28 برای `req.get`/`res.json` ساخت (`activator::resolve_dotted_member`، seed از `query.split_once('.')` وقتی کوئری یک‌تکه‌ی `"owner.member"` باشد) اینجا هرگز صدا زده نمی‌شود — چیزی قبل از آن، توکنایزر query را به دو seed جدا می‌شکند: `identifier:WSGIHandler` (درست) و `identifier:__call__` (بدون owner، سراسری). `identifier:__call__` با اولین `__call__` دیگری که در گراف پیدا می‌شود match می‌کند (`ASGIHandler.__call__`، @1.00 L1_exact) نه با `WSGIHandler.__call__` که واقعاً پرسیده شده. نتیجه در `django_wsgi_entry`: `asgi.py` forbidden وارد packet شد. این خواهر مستقیم F28 است ولی روی زبانی که متد با `__` شروع می‌شود (Python dunder)، نه روی `owner.member` جاوااسکریپتی | seed query tokenization (قبل از `activator.rs::resolve_dotted_member`؛ منبع دقیق توکن‌سازی pinpoint نشده — کار بعدی) | `NM_PROBE="How does WSGIHandler.__call__ turn a raw WSGI environ into a Django response?"` روی django holdout: `seed identifier:__call__ -> sym:django/core/handlers/asgi.py:ASGIHandler.__call__ @1.00 L1_exact` |
| F34 | کلمه‌ی عمومی/نام محصول در prompt با هر symbol هم‌نام در کل ریپو (بدون در نظر گرفتن ربط دامنه) match می‌شود: کلمه‌ی «Django» در متن پرسش (توصیف چارچوب، نه اسم کلاس هدف) با `OGRGeomType.django` (attribute کوچک‌حرفی در ماژول GIS کاملاً بی‌ربط) `@1.00 L1_exact` match شد. همچنین در پرسش CSRF، `concept:next` با نود یک آیکون SVG (`calendar-icons.svg:next`) و `concept:middleware` با یک فایل تست فیکسچر (`tests/middleware_exceptions/middleware.py`) match شدند — هر دو فقط چون هم‌نامِ کلمه‌ی عمومی prompt بودند، نه چون مرتبط بودند | seed resolution عمومی (concept/identifier match بدون وزن‌دهی به ربط فایل) | probe CSRF: `seed concept:next -> sym:.../calendar-icons.svg:next @1.00`, `seed concept:middleware -> sym:tests/middleware_exceptions/middleware.py:middleware @1.00`; probe wsgi: `seed identifier:Django -> sym:.../geomtype.py:OGRGeomType.django @1.00` |
| F35 | یک لیست seed چندزبانه/synonym گسترده برای مفاهیم auth/middleware/token همیشه امتحان می‌شود، حتی وقتی هیچ‌کدام در prompt یا در ریپو معنی ندارند: در probe CSRF این seedها امتحان شدند و همه miss کردند (`bearer`, `app.use`, `validateToken`, `verifyJwt`, `araKatman`, `ara_katman`, `ara katman`, `zwischen`, `middlewares`, `pipeline`, `next()`) — روی dev repos (که یکیشان express است) بعضی از این‌ها واقعاً match دارند (`app.use`)، ولی این یعنی لیست برای واژگان dev-repoها ساخته شده، نه یک مکانیزم عمومی. بی‌ضرر وقتی miss می‌کنند؛ خطرناک وقتی (نادر) به یک symbol هم‌نام تصادفی برخورد کنند — دقیقاً منشأ F34 | seed concept-expansion (فایل دقیق pinpoint نشده) | همان probe CSRF بالا؛ خط `seed ara katman -> - @0.00` و مشابه |
| F36 | الگوی کلی precision پایین: هر seed hit (حتی وقتی درست است) یک «utility fill» جدا با وزن ۸.۵–۱۶ اضافه می‌کند که خودش یک فایل کامل به packet اضافه می‌کند؛ روی ریپوی بزرگ با تعداد seed بیشتر (چون واژگان prompt عمومی‌تر با چیزهای بیشتری match می‌کند)، تعداد فایل نهایی از تعداد فایل گلد خیلی جلو می‌زند (نمونه: `django_csrf` — ۱ فایل گلد، ۸ فایل packet). این با مرحله ۴ (که دقیقاً همین‌جور قاعده‌ها را برای ۴ ریپوی مشخص محدود کرد) سازگار است: قواعد fill-gating مرحله ۴ (F17–F22) برای واژگان و ساختار همان ۴ ریپو تنظیم شدند و روی واژگان/ساختار جدید (django/ultralytics) همان محدودیت را اعمال نمی‌کنند | `selector.rs`، قواعد fill mرحله ۴ (F17–F22) | استنتاج از الگوی مشترک در هر دو probe؛ ردیابی دقیق‌تر (کدام قاعده‌ی fill کدام فایل را آورد) کار بعدی است، نه این session |

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

## جمع‌بندی صادقانه

- recall خوب است (0.95) — موتور تقریباً هیچ‌وقت فایل گلد را کاملاً گم نمی‌کند، حتی روی ریپوی ندیده.
- precision (0.146) و forbidden (واقعی: ۲/۲۰، بعد از کسر ۲ اشتباه گلد) نشان می‌دهند دقتِ 0.856 مرحله ۴ **کاملاً به واژگان و ساختار ۴ ریپوی dev بسته بود** — یک نوع overfit سیستماتیک، نه یک باگ نقطه‌ای.
- ریشه‌ی مشترک هر سه یافته (F33–F35): قواعد seed/synonym مرحله ۴ روی متن/زبان مشخص (JS dotted-access، انگلیسی generic، واژگان auth-style) نوشته شدند و روی زبان/دامنه‌ی جدید یا miss می‌کنند (بی‌ضرر) یا با کلمه‌ی عمومی هم‌نام برخورد می‌کنند (مضر).
- طبق تصمیم فاز ۵a: این ریپوها اکنون **holdout باقی می‌مانند** (به dev منتقل نشدند)؛ فیکس این یافته‌ها کار فاز بعدی (بازبینی مرحله ۴ با تصمیم صریح Parsa) است، نه این session.
- ۵c اندازه‌گیری شد (بالا) — عدد نگران‌کننده‌ای نبود، یک گیت با حاشیه‌ی کم (`l1_p95_slo`) و یک بخش اندازه‌گیری‌نشده (baseline ریپوی بزرگ) ماند.
- اندازه‌گیری‌نشده در این session: ۵b (B2B) — هنوز شروع نشده، منتظر گلد Parsa.
