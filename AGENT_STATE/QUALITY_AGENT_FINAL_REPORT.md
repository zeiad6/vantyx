# تقرير التدقيق النهائي — Vantyx

**الوكيل:** وكيل الجودة والموثوقية والأمان المستقل
**التاريخ:** 2026-11-28
**نقطة البداية:** `6e58b37` (T-122)
**الحالة النهائية المدققة:** `150997d` (فوق `f1dbe36`: T-123–T-127)،
مدفوعة إلى `origin/main`، وCI run 46113804180 نجح 789/789.
**الحالة:** التدقيق مكتمل. لم يعدّل QA أي ملف مصدر/اختبار/توثيق/CI/
Cargo مشترك، ولم ينفّذ أي commit أو عملية Git مدمّرة.

---

## 1. Executive Summary

### ماذا فُحص؟

فُحص المشروع من طرف إلى طرف على Linux: lexer، parser، checker، IR، المفسّر
(المرجع الدلالي)، الـ backend الأصلي (GNU C11)، runtime، الذاكرة، CLI،
الوحدات، الملفات، الأدوات، AI host، الاختبارات الحالية، الفوزرات، التوثيق،
وسجلات الحالة. شُغّلت الاختبارات على لقطات معزولة (snapshot) ببصمات مصدر
مسجّلة، وليس على `target/` المشترك، لتجنّب التسابق مع وكيل التطوير.

### ماذا اكتُشف؟

خمسة اكتشافات حقيقية، جميعها قابلة لإعادة الإنتاج:

| ID | الخطورة | الوصف | الحالة |
| --- | --- | --- | --- |
| QA-F001 | **P0** | `vantyx build <src> -o <src>` يستبدل المصدر بملف ELF ويعيد exit 0؛ يشمل الوحدات المستوردة وأهداف الروابط الرمزية والصلبة | **أُصلح كـ T-124 وتحقّق مستقلًا** |
| QA-F003 | P2 | قائمة/خريطة ذاتية المرجع تُقبل كنوع تكراري غير مطابق، فيصبح تعبير `Int` قائمة في وقت التشغيل | **أُصلح كـ T-126 وتحقّق مستقلًا** |
| QA-F002 | P3 | UTF-8 BOM يُرفض بـ `E0001` في المدخل والوحدات وstdin، بينما CRLF مقبول وغير موثّق | **أُصلح كـ T-125 وتحقّق مستقلًا** |
| QA-F004 | P3 | اختبارات native/e2e غير معزولة عن تنظيف `/tmp`؛ فشل clang العابر يمكن أن يُحوّل بوابة خضراء إلى حمراء | **أُصلح كـ T-127، لكنه قدّم QA-F005** |
| QA-F005 | P2 (هشاشة كامنة) | T-127 قد يجعل مسار Unix socket 109 أحرف مع PID من 7 خانات وعدّاد من 3 خانات، فوق حد `SUN_LEN` (107)؛ قد يفشل اختبار native واحد | **مفتوح في `f1dbe36`؛ CI 46113804180 نجح 789/789؛ patch من سطرين مُتحقق على لقطة QA** |

### ماذا أُصلح؟

لم يطبّق QA أي إصلاح مباشر لأن الملفات المشتركة كانت ضمن عمل الوكيل الرئيسي
غير المكتمل. بدلًا من ذلك قدّم QA: إعادة إنتاج دقيقة، اختبارات مستقلة، تحليل
سبب جذري، وpatch مقترح. الوكيل الرئيسي طبّق الإصلاحات الثلاثة (T-124، T-125،
T-126) ثم T-127 لعزل الاختبارات، وحدّث التوثيق وعدد الاختبارات، وتحقّق QA
منها على لقطات جديدة. T-127 عالج QA-F004 لكنه قدّم QA-F005 عند مسار CI؛
قدّم QA patch من سطرين مُتحققًا منه، ولم يُطبّق بعد.

### ماذا لم يُصلح / لم يُتحقق منه؟

- **QA-F005** (مسار Unix socket في T-127 عند PID من 7 خانات) لم يُصلح في
  الالتزام `f1dbe36`؛ patch من سطرين جاهز ومُتحقق على لقطة QA، لكنه لم
  يُطبّق. هذا هشاشة اختبار كامنة وليس سلوك لغة. QA-F004 نفسه أُصلح بنيويًا
  في T-127. الالتزامان `f1dbe36` و`150997d` دُفعا، وCI run 46113804180 نجح
  789/789 لأن PID الـrunner كان أقصر.
- **Windows وmacOS** لم يُختبرا فعليًا؛ CI يعمل على Ubuntu فقط، والكود
  المولّد يستخدم رؤوس POSIX.
- **الذكاء الاصطناعي السحابي الحي** لم يُعد التحقق منه؛ الفحوص كانت offline.
- **REPL pty** لم يُعد تشغيله في هذا التدقيق.
- **10M `range`** استُبعد من harness الافتراضي بسبب ذاكرة المضيف (3.8 GiB)،
  وليس لأنه حد لغوي.
- **long soak fuzzing** لم يُنفّذ؛ التشغيلات محدودة ومُبذّرة (seeded).

### مستوى الموثوقية الآن

مرتفع للنواة اللغوية والـ backend الأصلي على Linux ضمن النطاق المدعوم
والموثّق: 789 اختبارًا ناجحًا في لقطة T-123–T-126، و789 في T-127 بجذر قصير،
وdifferential fuzzing متفق، وASan/UBSan نظيفان، وأمثلة متطابقة. القيود
المعلنة تبقى قائمة: لا إعادة استرجاع ذاكرة عامة في native (ما عدا مؤقتات
العرض الموثقة)، لا sandbox، حاويات ديناميكية `Any`/`List[Any]`/`Map[Any,*]`
مرفوضة في native، وWindows غير متحقق. لا يوجد عيب وظيفي أو أمني مفتوح من
فئة P0/P1 بعد إصلاح T-124/T-125/T-126. **QA-F005** هشاشة اختبار كامنة (P2)
مفتوحة في `f1dbe36`: قد يفشل اختبار Unix socket واحد عند PID من 7 خانات
وعدّاد من 3 خانات. CI run 46113804180 نجح 789/789، والإصلاح متابعة موصى
بها لضمان حتمية CI.

---

## 2. Verification

### الاختبارات التي شُغّلت وعددها

| اللقطة | الاختبارات | التوزيع | النتيجة |
| --- | --- | --- | --- |
| HEAD `6e58b37` | 776 | 419 lib + 15 CLI + 47 AI + 132 e2e + 163 native | ناجحة، exit 0 |
| T-123 snapshot | 783 | 422 + 15 + 47 + 135 + 164 | ناجحة، exit 0 |
| T-123+T-124 snapshot | 784 | 422 + 15 + 47 + 135 + 165 | ناجحة، exit 0 |
| **T-123–T-126 snapshot** | **789** | **425 + 15 + 47 + 137 + 165** | **ناجحة، exit 0** |
| T-127 snapshot (جذر قصير) | 789 | 425 + 15 + 47 + 137 + 165 | ناجحة، exit 0 |
| **`f1dbe36` عند جذر صناعي 44 حرفًا وPID من 7 خانات** | **164/165 native** | 137 e2e ناجحة | **فشل اختبار socket واحد (QA-F005)** |
| **CI run 46113804180 على `150997d`** | **789** | **425 + 15 + 47 + 137 + 165** | **ناجحة، exit 0** |

عند جذر خاص بطول CI (108/109 حرفًا لمسار socket) يفشل اختبار
`recursive_copy_special_files_match_in_both_engines` بـ `SUN_LEN`؛ هذا
QA-F005 وليس فشلًا في بقية المجموعة. مسار CI القياسي
`/home/runner/work/vantyx/vantyx/target/tmp` ينتج 109 مع PID من 7 خانات
وعدّاد من 3 خانات، بينما PID الـrunner في التشغيل المرصود كان أقصر فنجح
CI. المسار المحلي ينتج 104.

لقطة T-123–T-126: `/home/kali/qa-work/final-verify`؛ لقطة T-127:
`/home/kali/qa-work/t127-verify`؛ ولقطة الالتزام النظيف:
`/home/kali/qa-work/f1dbe36-verify`. بصمات T-123–T-126:

```text
src/lib.rs    df82c8de...73dd0
src/value.rs  1f00b97a...41ae0
src/main.rs   234d80b9...4eebe
src/module.rs 0a15c962...c0a4e
tests/e2e.rs  43cb23d8...450d2
```

بصمات T-127: `tests/e2e.rs` `61f53bca...06a0d` و`tests/native.rs`
`9cd3728f...d44844` (نفس بصمة الملف الملتزم في `f1dbe36`).

### البوابات النهائية

```sh
cd /home/kali/qa-work/final-verify
export TMPDIR=/home/kali/qa-work/tmp
export CARGO_TARGET_DIR=/home/kali/qa-work/target-final-verify
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
VANTYX_CC=gcc cargo test --test native
ASAN_OPTIONS=detect_leaks=0:abort_on_error=1 \
UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1 \
VANTYX_CC="$PWD/tools/clang-sanitize.sh" cargo test --test native
```

| البوابة | النتيجة |
| --- | --- |
| `cargo test` | **789 passed, 0 failed**، exit 0 |
| `cargo clippy --all-targets -- -D warnings` | exit 0 |
| `cargo build --release` | exit 0 |
| `cargo fmt --check --all` على الشجرة الحية | exit 0 |
| native مع GCC | 165/165، exit 0 (95.35s) |
| native تحت ASan/UBSan | 165/165، exit 0 (140.95s) |
| `tools/test_differential_fuzz.py` | 9/9 |
| `tools/differential_fuzz.py --iterations 30 --seed 11` | 30/30 متفق |
| `tools/fuzz_smoke.py --iterations 200` | 9 حالات حدود + 200 إدخال عشوائي/معدّل، تشخيصات مضبوطة |
| الأمثلة الخمسة | 5/5 متطابقة بين المفسّر وnative |
| T-127: e2e + native مع `TMPDIR=/proc` وجذر قصير | 137/137 و165/165، exit 0 |

T-127 لا يجعل اختبار socket مستقلًا عن طول المسار: مع PID من 7 خانات
وعدّاد من 3 خانات يفشل (QA-F005). CI run 46113804180 نجح لأن PID الـrunner
كان أقصر؛ نجاح `TMPDIR=/proc` محليًا صحيح لكنه لا يغطي طول المسار.

### ملاحظة fmt العابرة

`cargo fmt --check --all` فشل على لقطة 00:12 فقط بسبب التفاف أسطر اختبار BOM
الجديد. الوكيل الرئيسي شغّل rustfmt بعد ذلك (بصمة `tests/e2e.rs` أصبحت
`22b342a7...`)، والفرق تنسيقي بحت، وجميع بصمات المصدر الأخرى متطابقة،
و`cargo fmt --check --all` على الشجرة الحية يعيد exit 0. لذلك لا يُسجّل
كاكتشاف، بل كملاحظة تسلسل زمني.

### اختبارات فشلت أو لم تُشغّل

- **فشلت سابقًا 4 اختبارات native** بسبب تنظيف `/tmp` المتزامن (خطأ clang
  `unable to rename temporary ... No such file or directory`)، وليس بسبب
  الكود. أعيد تشغيلها بـ `TMPDIR` معزول فنجحت جميعها. هذا أصل QA-F004.
- **فشل اختبار native واحد** في T-127 عند جذر خاص بطول 43+ حرفًا: مسار
  Unix socket 108/109 أحرف مقابل حد `SUN_LEN` 107. مسار CI القياسي ينتج
  109 مع PID من 7 خانات، لذا قد يفشل CI. CI run 46113804180 نجح لأن PID
  الـrunner كان أقصر. هذا QA-F005، وليس عيبًا في اللغة أو الـ backend.
- **لم تُشغّل:** Windows/macOS (بيئة غير متاحة)، AI سحابي حي (الوصول غير
  مضمون)، REPL pty، soak fuzzing طويل، حالة 10M `range` (ضغط ذاكرة المضيف).

---

## 3. Findings

### QA-F001 — `vantyx build -o` يدمّر المصدر ويعيد 0

- **الخطورة:** P0 (فقد بيانات)
- **المكوّن:** CLI (`build`)، native driver
- **الوصف:** تمرير مسار `-o` مباشرة إلى المترجم C دون فحص إن كان يطابق ملف
  المصدر أو وحدة مستوردة أو هدف رابط. النتيجة: استبدال `.vtx` بملف ELF،
  exit 0، بلا تشخيص.
- **إعادة الإنتاج:**

```sh
printf 'fn main():\n    print("keep")\n' > victim.vtx
vantyx build victim.vtx -o victim.vtx
echo $?          # 0
file victim.vtx  # ELF
```

- **المتوقع:** رفض الكتابة فوق أي مصدر محمّل، exit غير صفري، رسالة واضحة،
  وبقاء المصدر بايتًا ببايت.
- **السبب الجذري:** `src/main.rs::build_file` لم يكن يملك مسارات المصادر
  على القرص؛ `SourceRegistry` يحمل الأسماء لا المسارات.
- **الإصلاح:** T-124: `Compiled.source_paths`، مقارنة canonical عبر
  `fs_safety::paths_refer_to_same_file_paths`، exit 64، ورسالة
  `is also a source file` + `help: choose a different -o path`، قبل توليد
  الكود.
- **التحقق:** entry، module، relative، symlink، hard link، directory
  symlink، و`VANTYX_CC=/bin/false` كلها exit 64 والمصدر سليم. البناء العادي
  يعمل. الحد الموثّق: `build - -o source.vtx < source.vtx` ما زال يستبدل
  الملف لأن CLI لا يعرف مسار إعادة التوجيه.
- **الحالة:** FIXED (T-124)، ملتزم في `f1dbe36`، وCI run 46113804180 نجح
  789/789.

### QA-F002 — BOM يُرفض في المدخل والوحدات

- **الخطورة:** P3 (توافق/توثيق)
- **المكوّن:** lexer + module loading
- **الوصف:** U+FEFF في بداية الملف يُرفض بـ `E0001` عند 1:1، مع أن CRLF
  مقبول، والعديد من محررات Windows تكتب BOM افتراضيًا. لم يكن موثّقًا.
- **الإصلاح:** T-125: `strip_utf8_bom` في `src/lib.rs`، تطبيقه في
  `src/module.rs` قبل فحص الحجم والlexing، وفي `src/main.rs::read_source`
  (stdin والملفات)، مع اختبار e2e.
- **التحقق:** BOM entry، BOM+CRLF، BOM module، BOM stdin → exit 0؛ BOM داخلي
  أو مكرر → `E0001`؛ ملف BOM فقط → `E0308` بلا panic. المواصفة §1.1 وثّقت
  القاعدة.
- **الحالة:** FIXED (T-125)، ملتزم في `f1dbe36`، وCI run 46113804180 نجح
  789/789.

### QA-F003 — دورة ذاتية تُطابق نوعًا تكراريًا خاطئًا

- **الخطورة:** P2 (سلامة نظام الأنواع التدريجي)
- **المكوّن:** `src/value.rs::value_conforms` و`tighten_value`
- **الوصف:** مجموعة الزيارة كانت `HashSet<usize>` بمفتاح هوية الحاوية فقط،
  فتعود `true` عند أي زيارة متكررة دون النظر إلى النوع المتوقع. لذلك
  `xs = [xs]` تُقبل كـ `List[List[Int]]`، ويُطبع `ys[0][0]` كـ `[[...]]`
  بينما النوع الثابت `Int`، والمقارنة false، والجمع `R0005`.
- **الإصلاح:** T-126: `HashSet<(usize, Type)>` في `value_conforms` و
  `tighten_value`، مع إزالة المفتاح بعد كل فحص، وثلاثة اختبارات library
  واختبار e2e.
- **التحقق:** list/map/mutual mismatch → `R0005` exit 2 stdout فارغ؛
  `List[List[Any]]` و`Map[Str, Map[Str, Any]]` ما زالا مقبولين coinductively؛
  `[[1]]` و`{"k": {"a": 1}}` سليمان؛ الحاويات الديناميكية قابلة للتحوير؛
  العرض/المساواة دون تغيير.
- **الحالة:** FIXED (T-126)، ملتزم في `f1dbe36`، وCI run 46113804180 نجح
  789/789.

### QA-F004 — اختبارات native/e2e غير معزولة عن `/tmp`

- **الخطورة:** P3 (موثوقية بنية الاختبار، ليس عيب لغة)
- **المكوّن:** `tests/native.rs`، `tests/e2e.rs`، مساعدات `temp_dir()`
- **الوصف:** إذا نظّف أي process آخر `/tmp` أثناء التشغيل، يفشل clang بـ
  `unable to rename temporary ... No such file or directory`، فتصبح البوابة
  حمراء رغم سلامة الكود.
- **الدليل:** 4 من 164 اختبارًا فشلت بهذه الطريقة أثناء التدقيق؛ أعيد
  تشغيلها بـ `TMPDIR=/home/kali/qa-work/tmp` فنجحت 164/164، ثم 789/789.
- **الإصلاح:** T-127: `test_root()` خاص (يفضّل `VANTYX_TEST_TMPDIR` ثم
  `CARGO_TARGET_TMPDIR`) وضبط `TMPDIR`/`TMP`/`TEMP` على كل عملية `vantyx`
  تابعة. تحقّق QA بـ 789/789 بجذر قصير، وe2e 137/137 + native 165/165 مع
  `TMPDIR=/proc`.
- **الحالة:** FIXED (T-127)، ملتزم في `f1dbe36`، وCI run 46113804180 نجح
  789/789؛ T-127 قدّم QA-F005 كهشاشة مسار كامنة.

### QA-F005 — T-127 قد يتجاوز حد `SUN_LEN` عند مسار CI

- **الخطورة:** P2 (هشاشة اختبار كامنة؛ لا تأثير على اللغة أو runtime)
- **المكوّن:** `tests/native.rs::recursive_copy_special_files_match_in_both_engines`
  و`workspace()` / `test_root()` في T-127
- **الوصف:** T-127 نقل مساحات العمل إلى `CARGO_TARGET_TMPDIR`. اختبار Unix
  socket يربط `<workspace>/tree/sock`، وحد `sun_path` على Linux هو 108 بايت
  مع NUL (107 للمسار). المسار المحلي 104 يمر، لكن مسار GitHub Actions
  القياسي `/home/runner/work/vantyx/vantyx/target/tmp` (42 حرفًا) مع PID من
  7 خانات وعدّاد المجموعة الكاملة 120 ينتج 109، فيفشل الاختبار برسالة
  `path must be shorter than SUN_LEN`. PID الـrunner المرصود كان أقصر،
  فنجح CI run 46113804180.
- **إعادة الإنتاج:** لقطة نظيفة من `f1dbe36` (`tests/native.rs` بصمة
  `9cd3728f...d44844`): جذر 42 → مسار 107 → نجاح؛ جذر 43 → 108 → panic
  exit 101؛ جذر 44 → 109 → panic exit 101. المجموعة الكاملة على الجذر
  الافتراضي 41 مع PID من 7 خانات أنتجت 164/165.
- **السبب الجذري:** اسم مساحة العمل الوصفي
  `native-<counter>-copy-special-interpreter` داخل جذر مشتق من مسار checkout
  غير محدود الطول؛ الاختبار يفترض ضمنيًا أن الجذر قصير.
- **الإصلاح المقترح:** تغيير اسمَي مساحة العمل إلى `cs-i` و`cs-n` (patch من
  سطرين، يوفر 18/17 حرفًا) في
  `/tmp/opencode/vantyx-qa/proposed-t127-socket-path.patch`، أو استخدام
  helper قصير مخصص، أو ربط socket في مسار قصير ثم نقله إلى `tree/`.
- **التحقق:** على لقطة QA فقط، بعد patch وإعادة البناء: الاختبار يمر عند
  جذر 43/44؛ native 165/165 وe2e 137/137 عند جذر 44 مع
  `TMPDIR=/proc` (أطول من CI). السجلات:
  `t127-native-ci-length.log` و`t127-e2e-ci-length.log`.
- **الحالة:** OPEN في `f1dbe36`؛ patch جاهز وغير مطبّق. CI run 46113804180
  نجح 789/789 لأن PID الـrunner أقصر، لذا هذا عيب هشاشة وليس حاجبًا حاليًا.
  يُنصح بإصلاحه في التزام متابعة لضمان حتمية CI.

---

## 4. Regression Tests

### اختبارات أضافها الوكيل الرئيسي استجابةً لاكتشافات QA

| الاختبار | الملف | يغطي |
| --- | --- | --- |
| `build_refuses_to_overwrite_a_source_file` | `tests/native.rs` | T-124: entry/module/symlink، exit 64، بقاء المصدر |
| `utf8_bom_is_accepted_at_the_start_of_sources` | `tests/e2e.rs` | T-125: BOM+CRLF entry، module، stdin، رفض BOM داخلي |
| `self_referential_containers_reject_mismatched_recursive_types` | `tests/e2e.rs` | T-126: list/map self-cycle → `R0005` |
| 3 اختبارات library | `src/value.rs` | T-126: self/mutual/map conformance |

### إصلاح QA-F005 المقترح (لم يُطبّق)

```diff
-    let interpreter_directory = workspace("copy-special-interpreter", source);
+    let interpreter_directory = workspace("cs-i", source);
...
-    let native_directory = workspace("copy-special-native", source);
+    let native_directory = workspace("cs-n", source);
```

على لقطة QA فقط: الاختبار يمر عند جذر 43/44، والمجموعتان native 165/165
وe2e 137/137 عند جذر 44 مع `TMPDIR=/proc`. الـpatch محفوظ في
`/tmp/opencode/vantyx-qa/proposed-t127-socket-path.patch`. لم يُطبّق على
الشجرة الحية لأن `tests/native.rs` مملوك للوكيل الرئيسي.

### اختبارات QA المستقلة (خارج المستودع، مملوكة لـ QA)

| الأداة | التغطية | النتيجة |
| --- | --- | --- |
| `/tmp/opencode/vantyx-qa/qa_diff.py` | ~90 حالة differential | لا DIFF؛ 6 BUILD_FAIL مدخلات غير صالحة متوقعة |
| `/tmp/opencode/vantyx-qa/regression_build_output.sh` | P0 guard (3 حالات) | 3/3 مع البناء المُرقّع |
| `/home/kali/qa-work/qa_lint_module.py` | lint/module/CLI | ناجح بعد تصحيح توقعات خاطئة |
| `/home/kali/qa-work/qa_deep.py` | Unicode/عربي/emoji/recursion | 8/8 |
| `/home/kali/qa-work/qa_ai.py` | AI offline | ناجح |
| `/home/kali/qa-work/qa_fs.py` | filesystem safety | 8/8 |
| `/home/kali/qa-work/qa_boundary.py` | `E0401` boundary | ناجح |
| `/home/kali/qa-work/qa_strings.py` | strings/floats | 6/6 |
| `/home/kali/qa-work/t123-probe/qa_t123*.py` | T-123 adversarial | 33 فحصًا، 5 تصحيحات توقعات، لا bypass |

**التصنيف:** QA-F001 = REGRESSION-CRITICAL (فقد بيانات)؛ QA-F003 =
REGRESSION-HIGH (سلامة أنواع)؛ QA-F005 = REGRESSION-MEDIUM (هشاشة CI)؛
QA-F002 = REGRESSION-LOW (توافق)؛ QA-F004 = REGRESSION-LOW (بنية اختبار).

**أوامر التشغيل النموذجية:**

```sh
cargo test
VANTYX_CC=gcc cargo test --test native
ASAN_OPTIONS=detect_leaks=0:abort_on_error=1 \
UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1 \
VANTYX_CC="$PWD/tools/clang-sanitize.sh" cargo test --test native
python3 tools/test_differential_fuzz.py
python3 tools/differential_fuzz.py --iterations 100 --seed 1
python3 tools/fuzz_smoke.py --binary target/debug/vantyx
```

---

## 5. Security

### Critical

- **QA-F001 (P0):** فقد بيانات مصدر عند `build -o` نفس المسار. أُصلح في
  T-124 وتحقّق منه عبر entry/module/symlink/hard-link/dir-symlink. لا يوجد
  مسار معروف متبقٍ لتدمير مصدر محمّل عبر `-o`، باستثناء حالة `build -`
  الموثقة حيث لا يستطيع CLI معرفة مسار إعادة التوجيه.

### High

- لا توجد ثغرة أمنية P1 مفتوحة بعد إصلاح T-124/T-125/T-126 ضمن النطاق
  المفحوص. QA-F005 مصنّف P2 وهو هشاشة اختبار وليس ثغرة أمنية.

### Medium

- **QA-F003 (P2):** عدم اتساق أنواع عند الدورات. أُصلح؛ لا تأثير على الذاكرة
  أو تنفيذ كود.
- **TOCTOU في حارس `-o`:** الفحص مقارنة metadata ثم يكتب المترجم؛ يمكن
  نظريًا التلاعب بينهما. هذا موثّق كـ best-effort، وليس transaction ذرية.
- **حذف/نسخ/نقل الملفات:** القيود موثقة (رفض الجذور المدمّرة، عدم وجود
  rollback/trash، حدود النسخ المتداخل). لم تُكتشف ثغرة جديدة.
- **AI prompt injection / tool permissions:** لم تُختبر اختبارًا عدائيًا
  شاملًا؛ الفحوص offline غطّت JSON/schema/redaction/bounds، وليس حقن
  prompts أو استدعاء أدوات خبيثة. تبقى مخاطرة متبقية غير مقاسة.
- **QA-F005 (P2 هشاشة CI):** مسار Unix socket في T-127 قد يتجاوز `SUN_LEN`
  مع PID من 7 خانات؛ ليس ثغرة أمنية. CI الحالي نجح؛ يُصلح كمتابعة لضمان
  الحتمية.

### Low

- **QA-F002 (P3):** BOM؛ أُصلح.
- **QA-F004 (P3):** موثوقية الاختبارات؛ أُصلح بـ T-127.

### Residual risks

- لا sandbox؛ البرامج يمكنها قراءة/كتابة الملفات ضمن صلاحيات المستخدم
  واستدعاء المترجم C.
- الحاويات الديناميكية مرفوضة في native بـ `E0401`؛ المفسّر فقط يدعمها.
- حدود الموارد: `read_file` 64 MiB، عمق الاستدعاء 256 (`R0012`)، حدود
  الوحدات/الglob/النسخ المتداخل موثقة؛ لا حد عام للذاكرة أو الوقت.
- المفاتيح/الأسرار: redaction في metrics اختُبر offline؛ لم يُختبر تسريب
  عبر أخطاء مزوّد حقيقية أو logs إنتاجية.
- فشل الكتابة على `/dev/full` يعطي `R0013` exit 2، وstdout مغلق exit 0،
  وأنبوب مكسور `R0013` مع `(os error 32)`؛ تطابق بين المحركين.

---

## 6. Memory Safety

### Verified

- **ASan/UBSan:** مجموعة native كاملة 165/165 نظيفة (leak detection معطّل
  عمدًا)، وعرض struct/enum متداخل ودوري نظيف.
- **LSan مركّز:** لا تسريب في مؤقتات العرض بعد T-121.
- **T-121:** أُعيد إنتاج تحسن RSS: 8,144–8,428 KiB (median ~8,308) مقابل
  8,296 المعلن، وزمن 623–642 ms، مع إخراج متطابق.
- **T-123:** عقود العناصر/المفاتيح/القيم تمنع الكتابة الخاطئة عبر alias قبل
  الطفرة (`R0005`)؛ 33 فحصًا عدائيًا بلا bypass.
- **T-126:** لا تغيير في تخصيص الذاكرة؛ فقط مفاتيح زيارة أكبر قليلًا.

### Potential

- **تسريبات مقصودة:** معظم تخصيصات native تُسرّب عمدًا في v0.1؛ ADR-0056
  يخطط للمراحل، Stage 1 منفّذ (T-122)، Stage 2 جزئي (T-121). لا arena/
  refcount/GC.
- **دورات `Rc` في المفسّر:** تُسرّب (لا cycle collector)، موثّق.
- **بيانات العقود (T-123):** لا إعادة استرجاع لها؛ تعيش داخل تخصيص `Rc`
  القائم.
- **خرائط المفاتيح البنيوية:** O(n) في native؛ فهرس المفاتيح العددية ثابت
  البذرة؛ إعادة بناء الفهرس عند الحذف O(n).
- **حمل الذاكرة:** 10M `range` يستهلك ~709 MiB في المفسّر على مضيف 3.8 GiB؛
  ليس حدًا لغويًا لكنه مخاطرة تشغيلية.

### Unverified

- Allocation failure، ضغط ذاكرة طويل الأمد، تسريبات عبر تشغيلات متكررة
  لبرامج ضخمة، وسلوك Windows/macOS.
- لا `unsafe` في `src/` (فحص نصي)، لكن الكود المولّد C يعتمد على libc
  ويدير الذاكرة يدويًا؛ ASan/UBSan يغطي الحالات المختبرة فقط.

### Recommendations

1. إبقاء ASan/UBSan في CI (موجود) وتوسيع LSan المركّز لحالات العرض الكبيرة.
2. توثيق أي مرحلة جديدة من ADR-0056 فقط بعد قياسات RSS/زمن.
3. إضافة اختبار ضغط لدورات `Rc` في المفسّر لتأكيد الحد الموثّق.
4. عدم ادعاء “memory safe” بشكل عام؛ الأدق: “no observed unsafety in tested
   paths, with documented intentional leaks.”

---

## 7. Cross-Platform

| المنصة | الحالة | ملاحظات |
| --- | --- | --- |
| **Linux** | **VERIFIED** | كل الاختبارات والبوابات وASan/UBSan والأمثلة على Kali x86_64 |
| **Windows** | **NOT VERIFIED / ENVIRONMENT BLOCKED** | CI Ubuntu فقط؛ الكود المولّد يستخدم رؤوس POSIX؛ لا toolchain Windows متاح؛ BOM أصبح مقبولًا لكنه لا يثبت الدعم |
| **macOS** | **NOT VERIFIED / ENVIRONMENT BLOCKED** | لا بيئة متاحة؛ نفس مخاطر POSIX/الروابط والمسارات |

### POTENTIAL ISSUES

- **CRLF:** مقبول ومُختبر.
- **BOM:** كان مرفوضًا، أصبح مقبولًا (T-125)؛ يقلل مشكلة ملفات Windows.
- **الروابط الرمزية/الصلبة:** اختبارات `cfg(unix)`؛ سلوك Windows غير مثبت.
- **`/dev/full` وSIGPIPE:** اختبارات Linux فقط.
- **فواصل المسارات:** الكود يستخدم `Path`/`PathBuf` في Rust، لكن C المولّد
  يستخدم `/` وواجهات POSIX.
- **shell assumptions:** أوامر التوثيق والاختبار تفترض shell Unix.
- **الترميز:** UTF-8 فقط؛ UTF-16/32 غير مدعوم (موثّق).
- **الروابط الصلبة:** اختبار سابق فشل على ext4 بسبب حد الروابط لكل inode
  وعولج بتوزيع الروابط؛ لا يثبت سلوك أنظمة ملفات أخرى.

**لا يجوز الادعاء بدعم Windows/macOS native قبل تشغيل CI فعلي هناك.**

---

## 8. Performance

### المنهجية

قياسات على Kali Linux، x86_64، 4 CPUs، 3.8 GiB RAM، مع تكرارات وتوثيق
الأوامر؛ التفاصيل الخام في `QUALITY_AGENT_TEST_LOG.md` وسجلات
`/home/kali/qa-work/`. لا تُعلن تحسينات دون أرقام.

### Baseline

| الحالة | المفسّر | Native |
| --- | --- | --- |
| Startup | ~4.3–7.0 ms / 3.8 MB | — |
| Loop 1M | ~1,020 ms / 73.8 MB | ~5.5–8.7 ms / 2 MB |
| `fib(24)` | ~208–228 ms | ~3.5–4.2 ms |
| Build | ~347–385 ms / ~105 MB | — |
| `range(10M)` | ~2.44 s / ~709 MB | ~0.425 s / ~105 MB |
| T-121 print workload | — | median peak RSS ~8,308 KiB، زمن ~623–642 ms |

### ملاحظات

- T-124 يضيف فحص metadata واحدًا لكل بناء؛ تأثيره غير قابل للقياس عمليًا.
- T-125 يضيف فحص بادئة واحدًا لكل مصدر؛ مهمل.
- T-126 يضيف نسخ `Type` في مفاتيح الزيارة؛ قد يزيد تكلفة التحقق على
  الحاويات المتداخلة العميقة، لكن لا قياس انحدار. لم تُجرَ مقارنة قبل/بعد
  لأن التغيير وظيفي ومحدود.
- لا توجد ادعاءات أداء جديدة في هذا التقرير؛ الأرقام أعلاه خط أساس
  للمقارنة المستقبلية.
- 10M `range` خارج harness الافتراضي بسبب ذاكرة المضيف فقط.

---

## 9. Agent Coordination

### الملفات التي عدّلها QA

- `AGENT_STATE/QUALITY_AGENT_SCOPE.md`
- `AGENT_STATE/QUALITY_AGENT_TASKS.md`
- `AGENT_STATE/QUALITY_AGENT_FINDINGS.md`
- `AGENT_STATE/QUALITY_AGENT_TEST_LOG.md`
- `AGENT_STATE/QUALITY_AGENT_HANDOFF.md`
- `AGENT_STATE/QUALITY_AGENT_FINAL_REPORT.md` (هذا الملف)

كلها ملفات QA غير متعقّبة، ولم تُلتزم. كما أُنشئت أدوات/سجلات خارج المستودع
في `/tmp/opencode/vantyx-qa/` و`/home/kali/qa-work/`.

### الملفات التي قُرئت فقط

`src/*` (خصوصًا `main.rs`، `module.rs`، `value.rs`، `interp.rs`، `fs_safety.rs`،
`lib.rs`)، `tests/e2e.rs`، `tests/native.rs`، `docs/*`، `AGENTS.md`،
`PROJECT_STATUS.md`، `TASKS.md`، `AGENT_STATE/*` الخاص بالوكيل الرئيسي،
و`.github/`.

### الملفات التي تجُنّبت بسبب الوكيل الرئيسي

جميع ملفات المصدر والاختبارات والتوثيق وCI وCargo المشتركة، خصوصًا
`src/value.rs`، `src/interp.rs`، `src/main.rs`، `src/module.rs`،
`src/fs_safety.rs`، `tests/e2e.rs`، `tests/native.rs`، و`TASKS.md` /
`PROJECT_STATUS.md` / `AGENTS.md`. لم يُطبّق patch QA المقترح.

### التعارضات التي اكتُشفت

- الوكيل الرئيسي كان يعدّل الملفات نفسها أثناء التدقيق (T-123 ثم T-124/125/
  126 ثم T-127)، ثم التزمها في `f1dbe36` ومزامنة الحالة `150997d`. عولج
  ذلك بأخذ لقطات (snapshots) ببصمات، وعدم الكتابة في الشجرة المشتركة.
- أُعيد بناء `target/debug/vantyx` أثناء التدقيق (00:10) فتغيّر سلوك دورة
  من القبول إلى الرفض. لم يكن عدم حتمية؛ كان إصلاح T-126. أُعيد التحقق على
  لقطة ثابتة وثنائي خاص.
- فشل fmt العابر على اللقطة أصلحه الوكيل الرئيسي في الشجرة الحية؛ الفرق
  تنسيقي فقط.
- بعد الالتزام، أخذ QA لقطة نظيفة من `f1dbe36` وأثبت أن T-127 قد يفشل عند
  PID من 7 خانات؛ CI المرصود نجح لأن PID الـrunner أقصر. الـpatch المقترح
  لم يُطبّق. هذه هي المهمة المعلّقة الوحيدة، لكنها ليست حاجبة حاليًا.

### المهام المؤجلة

- تطبيق patch QA-F005 في `tests/native.rs` (مملوك للوكيل الرئيسي).
- تطبيق أي إصلاح آخر في المستودع (محمي).
- إضافة اختبارات QA إلى المستودع (تجنبًا للتعارض).
- Windows/macOS CI.
- إعادة التحقق السحابي الحي.
- REPL pty وsoak fuzzing.

---

## 10. Remaining Tasks

### P0/P1 — متابعة موصى بها

1. **أصلح QA-F005 في التزام متابعة** في `tests/native.rs`: تغيير اسمَي
   مساحة العمل إلى `cs-i`/`cs-n` (patch من سطرين في
   `/tmp/opencode/vantyx-qa/proposed-t127-socket-path.patch`)، أو ما يعادله.
   CI الحالي نجح، لكن الإصلاح يمنع هشاشة مستقبلية عند PID من 7 خانات.
2. **تحقق بمسار/PID أطول:** شغّل native وe2e مع
   `VANTYX_TEST_TMPDIR` بطول 43+ حرفًا و`TMPDIR=/proc`؛ توقّع 165/165
   و137/137، ثم `cargo test` كاملًا (789) وبقية البوابات.
3. **حالة CI:** run 46113804180 على `150997d` نجح 789/789. لا تُعد كتابة
   تاريخ منشور؛ التزام متابعة كافٍ.
4. **أبقِ عزل الاختبارات:** T-127 أصلح QA-F004 بنيويًا؛ إضافة
   `TMPDIR="$RUNNER_TEMP"` على مستوى خطوة CI تبقى دفاعًا في العمق وتوثيقًا
   للمتطلب.

### P2

5. تقرير سياسة `vantyx build - -o source.vtx < source.vtx`: تحذير أو رفض
   يتطلب معرفة مسار إعادة التوجيه، أو توثيق صريح إضافي.
6. تقييم إضافة فحص قراءة/تشخيص أوضح عندما تتعارض عقدة حاوية مع قراءة
   لاحقة بنوع مختلف (حاليًا الكتابة فقط هي المحمية).
7. قياس أثر T-126 على تحقق الحاويات المتداخلة العميقة إذا ظهرت حساسية
   أداء.
8. جعل اختبار socket في T-127 مستقلًا عن طول المسار تمامًا (ربط في مسار
   قصير ثم نقل الـsocket إلى `tree/`، أو helper مخصص) بدل الاعتماد على
   هامش الأسماء القصيرة.

### P3

9. توثيق سلوك BOM على مستوى مكتبة `compile(&str)` (لا يزيل BOM) مقابل CLI.
10. إضافة اختبار يثبت رفض BOM المكرر إذا كان السلوك مقصودًا.
11. تشغيل soak fuzzing طويل (بذور متعددة) وREPL pty.
12. إعادة التحقق من AI السحابي الحي عند توفر الوصول، واختبار حقن prompts
    وصلاحيات الأدوات عدائيًا.
13. إضافة CI لـ Windows/macOS إن أمكن، أو توثيق “native POSIX-only” بوضوح
    أكبر.
14. نقل أهم سيناريوهات QA المستقلة إلى اختبارات المستودع إذا رغب الفريق،
    مع نسبتها إلى QA-F001–QA-F005.

---

## ملحق: فهرس الأدلة

- النتائج: `AGENT_STATE/QUALITY_AGENT_FINDINGS.md`
- السجل الخام وخريطة التغطية: `AGENT_STATE/QUALITY_AGENT_TEST_LOG.md`
- المهام: `AGENT_STATE/QUALITY_AGENT_TASKS.md`
- النطاق: `AGENT_STATE/QUALITY_AGENT_SCOPE.md`
- التسليم: `AGENT_STATE/QUALITY_AGENT_HANDOFF.md`
- اللقطة النهائية: `/home/kali/qa-work/final-verify`
- لقطة T-127: `/home/kali/qa-work/t127-verify`
- لقطة الالتزام النظيف: `/home/kali/qa-work/f1dbe36-verify`
- سجل T-127 الكامل: `/home/kali/qa-work/cargo-test-t127-full.log`
  (164/165) و`cargo-test-t127-short-full.log` (789)
- T-127 مع `TMPDIR=/proc`: `/home/kali/qa-work/t127-native-proc.log`،
  `t127-e2e-proc.log`
- إصلاح QA-F005 التجريبي: `/home/kali/qa-work/t127-native-ci-length.log`،
  `t127-e2e-ci-length.log`
- patch QA-F005 (غير مطبّق):
  `/tmp/opencode/vantyx-qa/proposed-t127-socket-path.patch`
- سجل 789: `/home/kali/qa-work/cargo-test-final-full.log`
- GCC: `/home/kali/qa-work/final-gcc-native.log`
- ASan/UBSan: `/home/kali/qa-work/final-san-native.log`
- Differential fuzz: `/home/kali/qa-work/final-diff-fuzz.log`
- Fuzz smoke: `/home/kali/qa-work/final-fuzz-smoke.log`
- لقطة T-123: `/home/kali/qa-work/t123-verify`
- سجل 783: `/home/kali/qa-work/cargo-test-t123-full.log`
- سجل 776: `/home/kali/qa-work/cargo-test-head.log`
- patch P0 المقترح (غير مطبّق): `/tmp/opencode/vantyx-qa/proposed-build-output-guard.patch`

---

**الخلاصة:** Vantyx في حالة جيدة على Linux ضمن النطاق المدعوم. الاكتشاف
الخطير الوحيد (P0) أُصلح وتحقّق منه، وثغرة سلامة الأنواع (P2) أُصلحت، وتوافق
BOM (P3) أُصلح، وعزل الاختبارات أُصلح بـ T-127. يبقى **QA-F005** هشاشة
اختبار كامنة في `f1dbe36`؛ CI الحالي نجح 789/789، ويُصلح كمتابعة لضمان
الحتمية. الـpatch جاهز ومُتحقق. تبقى قيود المنصات والذاكرة والأمان موثّقة
بصدق ولا يجوز تجاوزها في الادعاءات.
