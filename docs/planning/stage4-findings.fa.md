# مرحله ۴ — یافته‌ها و اشکالات ثبت‌شده (زنده)

هر اشکالی که حین کار دیده می‌شود همین‌جا ثبت می‌شود؛ وقتی PR رفعش مرج شد، تیک می‌خورد.

## اشکالات

| # | اشکال | کجا | وضعیت |
|---|---|---|---|
| F1 | کلمه‌ی دایرکتوری در prompt (`shakespeare_char`) هیچ‌جا به‌عنوان مسیر خوانده نمی‌شد؛ fallback اولین `prepare.py` را برمی‌داشت | `activator::resolve_seed_query` | ✅ PR #20 (`seed::path_steer`) |
| F2 | شمارنده‌ی forbidden در `third_party_gold.rs` ورودی مسیردار را با basename هم match می‌کرد → twin طلایی «ممنوع» حساب می‌شد (۱۳ در واقع ۱۲ بود) | `tests/third_party_gold.rs` | ✅ PR #20 (از `gold::gold_file_hit` استفاده می‌شود) |
| F3 | `skeletonize_from_spans` span غیر-exon با بدنه‌ی کوچک‌تر از `min_lines` را حذف می‌کرد نه fold → `decode=SymbolMissing` | `skeleton.rs` | ✅ PR #21 |
| F4 | کد سطح ماژول بین spanها کاملاً حذف می‌شد؛ اسکریپت ۶۸ خطی به ۳۰ توکن می‌رسید | `skeleton.rs` | ✅ PR #21 (gap ≤۶ خط عیناً، بزرگ‌تر fold برگشت‌پذیر `module`) |
| F5 | خطوط سطح کلاس بعد از آخرین متد (attribute پس از متدها) هنوز خارج از گروه کلاس می‌افتند و به‌عنوان gap ماژول emit می‌شوند (نادر؛ بی‌ضرر) | `skeleton.rs` | ⏳ باز |
| F6 | `file_node_paths()` در هر فراخوانی همه‌ی مسیرها را clone می‌کند؛ path_steer دو بار به‌ازای هر seed query صدا می‌زند. روی ریپوی ۲۰۰ فایلی ناچیز، روی ۵۰k فایل قابل‌اندازه‌گیری | `path_steer.rs`, `graph.rs` | ⏳ باز (اندازه بگیر قبل از بهینه‌سازی) |
| F7 | fold policy برای بلوک‌های ماژول از prompt استفاده نمی‌کند: بلوکی که جواب سؤال در آن است (ساخت vocab) هم fold می‌شود؛ فقط قابل expand است. تلاش برای امتیازدهی مستقیم به gap ماژول با `FoldPolicy::score` روی فیکسچر `neuromesh_repo` شکست (فایل‌های بزرگ decoy با تطبیق واژه‌ی پرتکرار امتیاز کاذب می‌گرفتند، packet از ۲۷۵۰۰ به ۵۵۸۷۶ توکن پرید) — برگردانده شد. بخشی از ریشه (name_hits فقط `ident_tokens` را می‌دید نه کلمات خام prompt) در PR #26 با `focus_hits` رفع شد اما `GPT.forward` هنوز در نمونه‌ی nanoGPT fold می‌شود چون در رقابت بودجه‌ی exon (`SEED_EXON_BUDGET=4`) با متدهای دیگر همان فایل می‌بازد — تنظیم بودجه/رتبه‌بندی هنوز باز است | `skeleton.rs`, `fold.rs` | ⏳ باز (بخشی #26) — رقابت بودجه‌ی exon هنوز حل نشده |
| F8 | گیت ریلیز (`eval --release-gates`) روی main با precision 0.383 قرمز است (آستانه‌ی precision_min) — از قبل، نه regression | `neuromesh-cli eval` | ⏳ هدف مرحله ۴: ≥0.73 |
| F9 | با fold-not-delete packetها کمی بزرگ‌تر شدند (express +۱۲٪) چون کد ماژول دیگر حذف نمی‌شود؛ هیچ گیتی قرمز نشد ولی باید در آیتم ۷ (marker کوتاه‌تر) دیده شود | `skeleton.rs` | ✅ PR #30 (marker متد امضا را تکرار نمی‌کند — خط header همان بالا هست؛ `lines folded` → `lines`؛ gap ماژول `module L12-L40`. مجموع توکن گلد ۴۳۹۴۷ → ۳۹۵۷۲ (−۱۰٫۰٪)، task ۴۷۳۴۵ → ۴۲۵۷۲ (−۱۰٫۱٪)؛ recall/precision/oracle بی‌تغییر) |
| F10 | `pick_dominant_candidate` بین تعریف‌های هم‌نام (۱۴× `SimpleViT`، ۱۸× `posemb_sincos_2d`) با اندازه‌ی بدنه/درجه انتخاب می‌کرد؛ دو seed یک سؤال به دو فایل مختلف می‌رفتند | `graph.rs`, `activator.rs` | ✅ PR #22 (`seed::twin_cohere`: هم‌رخدادی + پوشش کامل stem توسط prompt) |
| F11 | `tokenize_ident("SimpleViT")` → `["simple","vi"]`؛ حرف تکی `T` حذف می‌شود و `vit` گم می‌شود. برای twin_cohere با «کلمه‌ی چسبیده» دور زده شد؛ ولی هر جای دیگری که به توکن‌های camelCase با حرف بزرگ پایانی تکیه می‌کند همین سوراخ را دارد | `neuromesh-parser/identifiers.rs` | ✅ PR #29 (`tokenize_camel_chunk`: دنباله‌ی حروف بزرگ به کلمه‌ی mixed-case می‌چسبد → `simple`,`vit`؛ `getID` هنوز `get`,`id`؛ `fold::tokenize_name` هم از همان استفاده می‌کند؛ workaround «کلمه‌ی چسبیده» از `twin_cohere::prompt_token_set` حذف شد) |
| F12 | `vit_sincos` هنوز دو sidecar هم‌ردیف می‌آورد (`simple_vit_with_fft.py`, `vaat.py`) → precision 0.33 به‌جای 1.0 | Physarum sidecar | ⏳ آیتم ۴ |
| F13 | هنگام mutation-test با `git checkout -- <file>` دو بار wiring کامیت‌نشده هم برگشت؛ قاعده: قبل از mutation کامیت یا `git stash push <file>` | فرایند | ✅ قاعده ثبت شد |
| F14 | یال `Calls` بین دو زبان: فراخوانی `select(...)` (SQLModel) در Python به component `Select` در `frontend/.../select.tsx` resolve شده → فایل TSX به‌عنوان callee *required* (utility:16) وارد packet سؤال backend می‌شود (`fastapi_register`, `fastapi_items_owner`) | `graph.rs::finalize_links` | ✅ PR #24 (`same_language_family` روی هدف `resolve_call_ranked`/`resolve_ranked`؛ `language_family` به `neuromesh-core` منتقل شد) |
| F15 | seedهای ضعیف (`concept:auth`, `concept:login`) در سؤال backend به کلاینت TypeScript می‌رفتند و Physarum سه فایل frontend دیگر روی‌شان می‌آویخت | `seed/lang_cohere.rs` | ✅ PR #23 (خانواده‌ی زبانی seedهای قوی، seed ضعیف خارج از آن حذف) |
| F16 | `twin_cohere` نودهای JSON/YAML (`components.json:config`) را anchor/twin حساب می‌کرد | `seed/twin_cohere.rs` | ✅ PR #23 (نودهای بدون family کنار گذاشته می‌شوند) |
| F17 | مصرف‌کننده‌های یک symbol seed (همه‌ی routeهایی که `settings` را import می‌کنند) با امتیاز ثابت 10 و بدون هیچ سیگنال query وارد fill می‌شدند؛ همان «هم‌ردیف بدون امتیاز query» آیتم ۴ | `selector.rs` (`inbound_use`) | ✅ PR #23 (فقط با match نام/stem در focus terms) |
| F18 | بلوک synaptic fill بی‌جهت بود (۹×وزن روی هر همسایه‌ی seed و فایلش) | `selector.rs` | ✅ PR #23 (فقط یال‌های خروجی از seed) |
| F20 | gating مصرف‌کننده‌ها دو فیکسچر را شکست (`physarum_usage`: «Where is Physarum used?»، `sms_stored`: «received» ↔ `SmsReceiver`): سؤال‌های usage باید مصرف‌کننده بگیرند و match باید inflection را تحمل کند | `selector.rs` | ✅ PR #23 (`focus_terms_ask_for_consumers`, `same_word_stem`) |
| F19 | فایل‌های داده (`package.json`, `components.json`) خارج از قاعده‌ی family هستند و هنوز در `fastapi_settings` می‌آیند (`package.json` با utility:36 چون focus term `secrets`/`database`؟) | fill | ✅ PR #27 (`seed::config_cohere`: نود Config/JSON/YAML/TOML بدون کلمه‌ی config/json/… در prompt، seed سؤال کد نمی‌شود) |
| F21 | فیکسچرهای کوچک (مثلاً `mini-aspnet::sms_store`) روی دو تعریف هم‌نام (`Store` در `Program.cs` و در `Sms.cshtml`) بدون سیگنال تمایز دیگری تکیه دارند: seed resolution یکی را (اشتباه) انتخاب می‌کند و تنها fill عمومی (بدون قید) فایل درست دیگر را می‌آورد. gate کردن fill بدون قید scale (آیتم ۱/PR سیدکار) این fixture را می‌شکند. فعلاً با آستانه‌ی اندازه‌ی پروژه (`large_project`، >۲۰ فایل) دور زده شد — gate فقط روی ریپوهای واقعی اثر می‌کند. ریشه‌ی واقعی: twin_cohere باید «Store» را به `Program.cs` cohere کند نه `Sms.cshtml` (هم‌رخدادی برابر است، تساوی باید با «handler نه view» شکسته شود) | `seed/twin_cohere.rs` | ⏳ باز |
| F22 | seed ضعیف (`concept:model` — alias expansion، کلمه‌ای که در prompt نبود) به **فایل** `models.py` با تطبیق stem resolve می‌شد و `expand_file_seeds_to_symbols` آن را به ۸ seed کلاس required تبدیل می‌کرد (precision `fastapi_settings` 0.33). seed ضعیفی که به نود File می‌رسد فقط وقتی می‌ماند که stem فایل در prompt آمده باشد (با تحمل جمع) یا هیچ seed قوی‌ای نباشد | `seed/weak_file_seed.rs` | ✅ PR #27 |
| F23 | `style_routing.rs` هنوز نام‌های فیکسچر `mini-shop` را هاردکد دارد: `inject_style_seeds` (`ProductCard`، `src/styles/_priceCard.scss`، `price-card-tile`)، `style_token_queries` (`hover-lift`, `focus-within`, `price-card`)، `inject_view_component_seeds` (`checkout`, `cartview`, `setqty`, `stepper`) و `STYLE_KEYWORDS` (`hover-lift`, `price-card`). فقط `style_noise_penalty` در #28 عمومی شد؛ بقیه باید با «stem/token نام‌برده در prompt» جایگزین شوند (اثر روی ریپوهای واقعی صفر است — هیچ style task آنجا نیست) | `style_routing.rs` | ⏳ باز |
| F24 | هیچ سقفی روی طول prompt در مرز MCP نیست: prompt ۲ مگابایتی (کنترل‌کاراکتر، path traversal، marker جعلی) در تست امنیت مرحله ۴ بی‌خطا و در سقف packet (۷۱ توکن از ۱۲۰۰۰) تمام می‌شود ولی ۱۱٫۳ ثانیه (debug) طول می‌کشد — استخراج signature خطی روی طول prompt است. پیشنهاد: سقف (مثلاً ۳۲ KB) در `neuromesh-mcp` با پیام صریح، نه بریدن بی‌صدا | `neuromesh-mcp`, `neuromesh-task` | ⏳ باز (فقط اندازه‌گیری شد) |
| F25 | گیت ریلیز `eval --release-gates` (Benchmark A، split holdout) روی این ریپو فقط **۳ سلول** دارد: `handle_tool_call_intent` (prec 0.75)، `physarum_usage` (0.40) و `missing_seed` — که **به‌طور عمدی** seed ندارد و packet خالی می‌دهد، ولی precision آن **۰** حساب می‌شود نه «تعریف‌نشده». میانگین 0.383 = (0.75+0.40+0)/3؛ با این تعریف رسیدن به ≥0.73 ساختاراً ناممکن است. پیشنهاد: سلول‌های no_seed/packet خالی از میانگین precision خارج شوند (→ 0.575) و `physarum_usage` جدا دیده شود (۵ فایل برای ۲ گلد؛ سؤال usage است، پس gating مصرف‌کننده به‌درستی خاموش است) | `benchmark_suite.rs`, `evaluate.rs` | ⏳ باز (اندازه‌گیری شد؛ تغییر تعریف گیت با تأیید) |
| F26 | seeder آرتیفکت ML (`retrieval::artifact_seeds`) با دیدن «training loop»/«forward» **هر** نود با آن role را seed می‌کند: `artifact:TrainLoop→bench` (`bench.py` forbidden ×۲) و `artifact:Layer→forward → LayerNorm.forward` که با priority ۲۰۰ جای `GPT.forward` را در بودجه‌ی exon می‌گیرد (strict `nano_train_forward`) | `retrieval/artifact_seeds.rs` | ⏳ باز |
| F27 | seed ضعیف با **زیررشته** به symbol فایل دیگر می‌رسد (`concept:config → GPT.configure_optimizers`) با وجود seed قوی (`configurator.py`) → `model.py` forbidden در `nano_configurator`. خواهر F22 در سطح symbol | `seed/weak_file_seed.rs` (گسترش) | ⏳ باز |
| F28 | جفت `owner.member` بدون توجه به owner resolve می‌شود: `identifier:req.get → response.js:res.get`، `identifier:res.json → express.js:json`؛ و owner سه‌حرفی خودش seed می‌شود: `identifier:res → View.resolve` (stem-search). سه forbidden express (`view.js`، `response.js` ×۲) | `activator::resolve_seed_query`, `graph::resolve_ranked` | ⏳ باز |

## اعداد ratchet (ریپوهای واقعی)

| PR | recall | precision | forbidden | oracle | strict |
|---|---|---|---|---|---|
| main (4b74c25) | 0.900 | 0.146 | 12 (شمارنده‌ی درست) | 14/21 | 12 |
| #20 path steer | 0.950 | 0.196 | 11 | 14/21 | 12 |
| #21 fold-not-delete | 0.950 | 0.196 | 11 | 15/21 | 13 |
| #22 twin coherence | **1.000** | 0.214 | 11 | 16/21 | 14 |
| #23 language family + consumer gating | 1.000 | **0.266** | **9** | 16/21 | 14 |
| #24 no cross-language Calls edge (F14) | 1.000 | **0.273** | 9 | 16/21 | 14 |
| #25 sidecar/utility fill gate (F12, آیتم ۴) | 1.000 | **0.572** | **4** | **19/21** | **17** |
| #26 fold: prompt-named method scores (بخشی از F7) | 1.000 | 0.572 | 4 | 19/21 | 17 |
| #27 data-node seeds + weak file-stem seeds (F19, F22) | 1.000 | **0.606** | 4 | 19/21 | 17 |
| #28 style_noise_penalty عمومی شد (آیتم ۶) | 1.000 | 0.606 | 4 | 19/21 | 17 |
| #29 tokenize_ident uppercase tail (F11) | 1.000 | 0.606 | 4 | 19/21 | 17 |
| #30 marker fold کوتاه‌تر (F9، −۱۰٪ توکن) | 1.000 | 0.606 | 4 | 19/21 | 17 |
| #31 تست امنیت مرحله ۴ | 1.000 | 0.606 | 4 | 19/21 | 17 |

## ترتیب باقی‌مانده‌ی مرحله ۴

2. ~~`vit_sincos` recall 0~~ ✅ بدون tantivy حل شد (#22)؛ tantivy/reranker همچنان برای precision لازم است
3. ~~هرس frontend برای سؤال backend~~ ✅ #23 (F15/F16؛ باقی‌مانده: F14 یال cross-language، F19 فایل داده)
4. sidecar Physarum هم‌ردیف‌ها — F17/F18 در #23 نیمی از آن را بست؛ خودِ sidecar Physarum (`vit_sincos` هنوز `simple_vit_with_fft.py`, `vaat.py`) مانده
5. fold policy (strict) — F7 هم اینجا
6. ~~حذف `style_noise_penalty` هاردکد~~ ✅ #28 (قاعده‌ی عمومی: در سؤال استایل، فایل کامپوننت/اسکریپتی که prompt نامش را نبرده noise است)
7. ~~marker fold کوتاه‌تر~~ ✅ #30 (F9)
