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

## اعداد ratchet (ریپوهای واقعی)

| PR | recall | precision | forbidden | oracle | strict |
|---|---|---|---|---|---|
| main (4b74c25) | 0.900 | 0.146 | 12 (شمارنده‌ی درست) | 14/21 | 12 |
| #20 path steer | 0.950 | 0.196 | 11 | 14/21 | 12 |
| #21 fold-not-delete | 0.950 | 0.196 | 11 | 15/21 | 13 |

## ترتیب باقی‌مانده‌ی مرحله ۴

2. `vit_sincos` recall 0 → reranker + tantivy
3. هرس frontend برای سؤال backend
4. sidecar Physarum هم‌ردیف‌ها
5. fold policy (strict) — F7 هم اینجا
6. حذف `style_noise_penalty` هاردکد
7. marker fold کوتاه‌تر — F9 هم اینجا
