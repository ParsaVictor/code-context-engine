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
| F7 | fold policy برای بلوک‌های ماژول از prompt استفاده نمی‌کند: بلوکی که جواب سؤال در آن است (ساخت vocab) هم fold می‌شود؛ فقط قابل expand است | `skeleton.rs`, `fold.rs` | ⏳ باز — مربوط به آیتم ۵ (fold policy) |
| F8 | گیت ریلیز (`eval --release-gates`) روی main با precision 0.383 قرمز است (آستانه‌ی precision_min) — از قبل، نه regression | `neuromesh-cli eval` | ⏳ هدف مرحله ۴: ≥0.73 |
| F9 | با fold-not-delete packetها کمی بزرگ‌تر شدند (express +۱۲٪) چون کد ماژول دیگر حذف نمی‌شود؛ هیچ گیتی قرمز نشد ولی باید در آیتم ۷ (marker کوتاه‌تر) دیده شود | `skeleton.rs` | ⏳ باز |
| F10 | `pick_dominant_candidate` بین تعریف‌های هم‌نام (۱۴× `SimpleViT`، ۱۸× `posemb_sincos_2d`) با اندازه‌ی بدنه/درجه انتخاب می‌کرد؛ دو seed یک سؤال به دو فایل مختلف می‌رفتند | `graph.rs`, `activator.rs` | ✅ PR #22 (`seed::twin_cohere`: هم‌رخدادی + پوشش کامل stem توسط prompt) |
| F11 | `tokenize_ident("SimpleViT")` → `["simple","vi"]`؛ حرف تکی `T` حذف می‌شود و `vit` گم می‌شود. برای twin_cohere با «کلمه‌ی چسبیده» دور زده شد؛ ولی هر جای دیگری که به توکن‌های camelCase با حرف بزرگ پایانی تکیه می‌کند همین سوراخ را دارد | `neuromesh-parser/identifiers.rs` | ⏳ باز |
| F12 | `vit_sincos` هنوز دو sidecar هم‌ردیف می‌آورد (`simple_vit_with_fft.py`, `vaat.py`) → precision 0.33 به‌جای 1.0 | Physarum sidecar | ⏳ آیتم ۴ |
| F13 | هنگام mutation-test با `git checkout -- <file>` دو بار wiring کامیت‌نشده هم برگشت؛ قاعده: قبل از mutation کامیت یا `git stash push <file>` | فرایند | ✅ قاعده ثبت شد |
| F14 | یال `Calls` بین دو زبان: فراخوانی `select(...)` (SQLModel) در Python به component `Select` در `frontend/.../select.tsx` resolve شده → فایل TSX به‌عنوان callee *required* (utility:16) وارد packet سؤال backend می‌شود (`fastapi_register`, `fastapi_items_owner`) | `graph.rs::finalize_links` | ✅ PR #24 (`same_language_family` روی هدف `resolve_call_ranked`/`resolve_ranked`؛ `language_family` به `neuromesh-core` منتقل شد) |
| F15 | seedهای ضعیف (`concept:auth`, `concept:login`) در سؤال backend به کلاینت TypeScript می‌رفتند و Physarum سه فایل frontend دیگر روی‌شان می‌آویخت | `seed/lang_cohere.rs` | ✅ PR #23 (خانواده‌ی زبانی seedهای قوی، seed ضعیف خارج از آن حذف) |
| F16 | `twin_cohere` نودهای JSON/YAML (`components.json:config`) را anchor/twin حساب می‌کرد | `seed/twin_cohere.rs` | ✅ PR #23 (نودهای بدون family کنار گذاشته می‌شوند) |
| F17 | مصرف‌کننده‌های یک symbol seed (همه‌ی routeهایی که `settings` را import می‌کنند) با امتیاز ثابت 10 و بدون هیچ سیگنال query وارد fill می‌شدند؛ همان «هم‌ردیف بدون امتیاز query» آیتم ۴ | `selector.rs` (`inbound_use`) | ✅ PR #23 (فقط با match نام/stem در focus terms) |
| F18 | بلوک synaptic fill بی‌جهت بود (۹×وزن روی هر همسایه‌ی seed و فایلش) | `selector.rs` | ✅ PR #23 (فقط یال‌های خروجی از seed) |
| F20 | gating مصرف‌کننده‌ها دو فیکسچر را شکست (`physarum_usage`: «Where is Physarum used?»، `sms_stored`: «received» ↔ `SmsReceiver`): سؤال‌های usage باید مصرف‌کننده بگیرند و match باید inflection را تحمل کند | `selector.rs` | ✅ PR #23 (`focus_terms_ask_for_consumers`, `same_word_stem`) |
| F19 | فایل‌های داده (`package.json`, `components.json`) خارج از قاعده‌ی family هستند و هنوز در `fastapi_settings` می‌آیند (`package.json` با utility:36 چون focus term `secrets`/`database`؟) | fill | ⏳ باز |

## اعداد ratchet (ریپوهای واقعی)

| PR | recall | precision | forbidden | oracle | strict |
|---|---|---|---|---|---|
| main (4b74c25) | 0.900 | 0.146 | 12 (شمارنده‌ی درست) | 14/21 | 12 |
| #20 path steer | 0.950 | 0.196 | 11 | 14/21 | 12 |
| #21 fold-not-delete | 0.950 | 0.196 | 11 | 15/21 | 13 |
| #22 twin coherence | **1.000** | 0.214 | 11 | 16/21 | 14 |
| #23 language family + consumer gating | 1.000 | **0.266** | **9** | 16/21 | 14 |
| #24 no cross-language Calls edge (F14) | 1.000 | **0.273** | 9 | 16/21 | 14 |

## ترتیب باقی‌مانده‌ی مرحله ۴

2. ~~`vit_sincos` recall 0~~ ✅ بدون tantivy حل شد (#22)؛ tantivy/reranker همچنان برای precision لازم است
3. ~~هرس frontend برای سؤال backend~~ ✅ #23 (F15/F16؛ باقی‌مانده: F14 یال cross-language، F19 فایل داده)
4. sidecar Physarum هم‌ردیف‌ها — F17/F18 در #23 نیمی از آن را بست؛ خودِ sidecar Physarum (`vit_sincos` هنوز `simple_vit_with_fft.py`, `vaat.py`) مانده
5. fold policy (strict) — F7 هم اینجا
6. حذف `style_noise_penalty` هاردکد
7. marker fold کوتاه‌تر — F9 هم اینجا
