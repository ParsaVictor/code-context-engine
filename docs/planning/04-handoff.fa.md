<div style="direction:rtl;text-align:right;font-family:Tahoma">

# تحویل و شروع جلسه‌ی بعد

آخرین به‌روزرسانی: ۲۰۲۶/۰۹/۰۹ — پایان فاز ۰

---

## ۱. مختصات پروژه

| مورد | مقدار |
|---|---|
| ریپو | `github.com/ParsaVictor/code-context-engine` (پابلیک، **فورک گیت‌هابی نیست** — ریپوی مستقل با تاریخچه‌ی مشترک) |
| کلون محلی | `C:\1\1_پروژه\5_neuromesh\repo` |
| ریموت‌ها | `origin` = ریپوی ما · `upstream` = `pinoox/neuromesh` |
| نقطه‌ی baseline | تگ `baseline-v0.9.0` (کامیت upstream `fe65e18`) |
| برنچ کاری | `p0-isolation` → PR پیش‌نویس **#10** به `main` |
| هویت گیت | `ParsaVictor <1.parsa.karkooti@gmail.com>` — همه‌ی کامیت‌ها به نام پارسا |
| Toolchain محلی | cargo/rustc **1.98.1**، rustfmt، clippy، MSVC **14.44** — نصب و کارکرده |

**قبل از هر push حتماً محلی اجرا کن:**

```bash
export PATH="$USERPROFILE/.cargo/bin:$PATH"
cargo fmt --all && cargo clippy --all-targets && cargo test --all
```

سه دور CI اول این پروژه فقط سر `cargo fmt` هدر رفت. حالا toolchain محلی هست؛ استفاده‌اش کن.

---

## ۲. فاز ۰ — چه چیزی تمام شد

هدف فاز: **باگ آلودگی گراف بین پروژه‌ها را قطعی ببند و با تست CI اثباتش کن.**

| ایشو | کار | وضعیت |
|---|---|---|
| #1 | `stable_project_id` — شناسه از مسیر canonical ریشه‌ی پروژه | ✅ |
| #3 | گارد ناوردای تک‌پروژه‌ای + eviction | ✅ |
| #4 | سوییچ تمیز workspace + `reconcile_loaded_project_id` | ✅ |
| #5 | حذف نوسان `workspace_root` در ingest | ✅ |
| #6 | رد workspaceی که حدس زده شده و marker ندارد | ✅ |
| #7 | حافظه پروژه را دنبال کند (store دیگر پین نیست) | ✅ |
| #8 | **گیت نشتی end-to-end در CI** | ✅ |
| #9 | مستندات `docs/isolation.md` | ✅ |
| #11 | ignore rules فقط نسبت به ریشه‌ی پروژه | ✅ |
| #2 | رجیستری مرکزی پروژه‌ها | ⏸ عقب افتاد — برای بستن باگ لازم نبود، زیرساخت P1 است |
| #12 | flake تست زمان‌محور (ارثی از upstream) | ⏸ باز |

**معیار «انجام‌شده» محقق شد:** دو فیکسچر که `scripts/util.py` را بایت‌به‌بایت مشترک دارند، سوییچ بین‌شان، و ادعای صفر نشتی — در CI به‌عنوان گام مستقل اجرا می‌شود.

### باگ‌هایی که واقعاً بسته شد

1. **`ProjectId` از نام پوشه ساخته می‌شد.** دو چک‌اوت با نام `app` یک شناسه می‌گرفتند. ۱۸ سایت اصلاح شد.
2. **سوییچ رد‌شده.** گارد `same_workspace_path` فقط مسیر را مقایسه می‌کرد → گراف A روی B سرو می‌شد.
3. **فایل هم‌مسیر و هم‌محتوا.** `ingest_file_keep` زودخروج می‌زند؛ گره‌های پروژه‌ی قبلی زنده می‌ماندند.
4. **حافظه در پروژه‌ی اشتباه.** اپیزودها در `neuromesh.json` پروژه‌ی قبلی نوشته می‌شد.
5. **ایندکس صفر فایل.** پروژه‌ی زیر پوشه‌ای به نام `build`/`dist`/`vendor`/`AppData` هیچ فایلی ایندکس نمی‌کرد.
6. **ایندکس‌کردن پوشه‌ی بی‌ربط.** سرور بدون workspace حالا رد می‌کند و دلیل می‌گوید.

### تصحیح مهمی که در مسیر انجام شد

گزارش اولیه ادعا می‌کرد «اگر پروژه‌ی دوم `graph.bin` نداشته باشد، A و B قاطی می‌شوند». **این غلط بود.** `prune_absent_files` عمده‌ی گره‌های A را پاک می‌کند. جمله‌ی درست: **ایزوله‌سازی upstream تصادفی است، نه تضمین‌شده** — و در سه مسیر مشخص (بالا) واقعاً می‌شکند. سند `03-P0-isolation-design.fa.md` با علامت «تصحیح» به‌روز شده.

---

## ۳. قدم بعدی — فاز ۱

ترتیب پیشنهادی (از `ROADMAP.md`):

1. **PR به upstream** — #1/#3/#4/#5/#8/#11 را به‌شکل PR تمیز به `pinoox/neuromesh` بفرست. `git diff upstream/main` را مبنا بگیر. بهترین حالت: merge شود و اسم تیم روی feature اصلی یک پروژه‌ی در حال رشد بنشیند.
2. **`main` را به‌روز کن** — PR #10 را از draft دربیاور و merge کن.
3. **Universal Artifact Graph** — `NodeType`/`EdgeType`های ML به `crates/neuromesh-core/src/types.rs`.
4. **پارسر `.ipynb`** + لایه Config→Code.
5. **Overlay فریم‌ورک PyTorch** (تشخیص `nn.Module`, `forward`, `DataLoader`, train loop, checkpoint, metric).
6. **demo قاتل:** «چرا mAP افت کرد؟» → عقب‌گرد `Metric → EvalLoop → Model → Checkpoint → Transform → Dataset`.

**فیکسچر آماده است:** `tests/fixtures/iso-ml-python/` همین الان زنجیره‌ی `dataset.py → model.py → train.py` را با `DetectionDataset`, `build_transforms`, `Detector.forward`, `detection_loss`, `save_checkpoint`, `evaluate_map` دارد. عمداً برای همین demo ساخته شد.

---

## ۴. نکاتی که جلسه‌ی بعد باید بداند

- **`__all__` در مانیتور عمداً چندپروژه‌ای است** (`collective_mesh`). ناوردا را نمی‌شکند چون گره‌ها شناسه‌ی گراف فعلی را می‌گیرند. اگر سراغ federated retrieval رفتیم، این نقطه‌ی شروع است.
- **monorepo فعلاً یک پروژه است.** `stable_project_id` تا نزدیک‌ترین `.git` بالا می‌رود. تفکیک زیرپروژه‌ها feature جداست و پیاده نشده.
- **`cargo test --all` روی این ماشین ممکن است یک تست زمان‌محور را رد کند** (ایشو #12). به‌تنهایی پاس می‌شود و در CI سبز است.
- **دو تست فرمت‌حساس‌اند:** ماکروهای `assert!`/`assert_eq!` را از اول چندخطی بنویس تا rustfmt دستشان نزند.
- **فیکسچرهای ایزوله باید خارج از ریپو stage شوند** — هر مسیری داخل ریپو همان شناسه‌ی ریپو را می‌گیرد.

---

## ۵. پرامپت شروع چت بعدی

> ادامه‌ی پروژه‌ی `code-context-engine` (فورک مستقل NeuroMesh) در
> `C:\1\1_پروژه\5_neuromesh\repo`.
> فاز ۰ (ایزوله‌سازی پروژه) تمام و در CI سبز است — `docs/planning/04-handoff.fa.md` را بخوان.
> می‌خواهم فاز ۱ را شروع کنیم: Universal Artifact Graph و پشتیبانی ML/PyTorch.
> اول PR فاز ۰ را به upstream بفرست، بعد سراغ فاز ۱ برویم.

</div>
