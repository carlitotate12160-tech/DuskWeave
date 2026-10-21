# DW-FIX-M1-RUN-DIAGNOSTICS — prompt IDE

PROJECT: DuskWeave
MODE: FIX
WORKSPACE: D:/DuskWeave-ci-complexity
BRANCH: fix/m1-run-diagnostics
BASE: b439661f82c4cebf1d615629f07eb176dd3a53e8

Kerjakan langsung satu FIX PR. Outcome: kegagalan keputusan provider melalui m1-run dapat dijelaskan lewat receipt dan inspect dari proses baru, tanpa data sensitif, efek replay, penambahan pengawas generik, atau pelemahan pengaman. Ini mendukung loop MVP objective → hipotesis → tindakan berizin → bukti → adaptasi, dengan APT41/Lazarus/Volt Typhoon sebagai referensi perilaku, bukan playbook atau izin operasi.

## 1. Preflight dan kewenangan

Baca AGENTS, ENGINEERING_STATE, BUILD_ORDER/00/07, QUALITY_BAR §7, skill build-duskweave, ADR-011/016 dan kontrak M1 yang relevan. Penerimaan §7/§4a pada 0615fb72 tetap berlaku. Periksa git worktree list, identity, branch, HEAD dan status. Base di atas sudah memuat #107/#108. Bila source/authority berubah, rekonsiliasi diff sebelum melanjutkan; jangan memakai lampiran lama sebagai current state.

Gunakan branch di atas pada worktree terverifikasi yang bersih; jangan bekerja di master atau membersihkan worktree lain. Verifikasi origin sebelum publish. Tidak ada perubahan produksi dari packet ini yang telah dikerjakan.

Resource yang diperiksa: dwpg-r3 berjalan, PostgreSQL 17, image postgres@sha256:d74eeac9a635390a49bc21bd49fccd973de707e2a53a76ac49b552b8712ec46f. Periksa kembali image, versi dan binding loopback; reuse service, jangan recreate. Buat database baru dw_native_run_diagnostics_20261021a dan login restricted unik melalui bootstrap yang ada. Collision harus menolak; gunakan nama dw_native_ baru yang dihasilkan helper bila perlu. Jangan reset password role lama, menyalin/mengedit DB demo, mengubah .env, environment global, volume atau container. Password role bersifat cluster-wide.

Rust 1.94.1 MSVC; ordinary target D:/DuskWeave-build/rust-1.94.1/normal. Satu owner memegang target selama build/test/subprocess; tidak ada cargo clean. Report D:/DuskWeave-build/rust-1.94.1/reports/m1-run-diagnostics-20261021a. Coverage lokal, hanya bila diperlukan, memakai target terpisah D:/DuskWeave-build/rust-1.94.1/coverage-m1-run-diagnostics-20261021a. Tidak boleh membaca/copy/edit .cargo/config.toml. Jangan mencetak key, DSN atau environment dump.

Seluruh pengujian packet ini offline: hapus DEEPSEEK_API_KEY dari environment proses bootstrap/test/probe beserta turunannya. Jangan bergantung pada env shell yang diwariskan. Tidak ada provider contact, DNS target, paid retry, atau pengaktifan ulang run lama.

## 2. Bukti dan batas kesimpulan

- Episode asli c9c2d22d-945a-409c-95bc-9f3e6d683616: started→claimed 1280 ms; claimed→UNKNOWN 4751 ms; total 6031 ms. Ini waktu pencatatan parent, bukan benchmark query claim.
- Environment peluncuran yang tersimpan memuat semua DSN; missing-env belum terbukti sebagai penyebab episode asli.
- Probe tanpa key pada child binary yang sama: exit 124, 5148 ms, tanpa READY. Ada beberapa sumber exit 124; jangan menetapkan sumber hanya dari angka exit.
- Copy diagnosis dw_diag_20261021a memiliki satu panggilan completed dan satu keputusan; terminal owner_view_requested, nol DNS fixture dan nol keputusan kedua. Ini bukan bukti loop lengkap.
- Kelemahan terbukti: read_ready membuang terminal output; finished meratakan semua exit gagal. Jangan mengubah catatan sejarah supaya tampak berhasil.

## 3. Exact editable map

Production:
- src/database_config.rs
- src/m1_run.rs
- src/m1_run_cli.rs
- src/m1_run_supervision.rs
- src/postgres_m1_run.rs
- migrations/0020_m1_run_diagnostics.sql (baru)

Tests/support:
- tests/m1_run_cli.rs
- tests/m1_run_diagnostics.rs (baru; skenario diagnosis yang kohesif)
- tests/m1_provider_fixture.rs (hanya assertion yang terdampak dan bukti regresi)
- tests/support/m1_run.rs
- tests/support/m1_native_bootstrap.py
- tests/test_m1_native_helpers.py

Packet/tracking/operator:
- docs/workflows/DW-FIX-M1-RUN-DIAGNOSTICS.md (baru; salin packet dan tambahkan bukti ringkas)
- docs/ENGINEERING_STATE.md
- docs/MVP_AND_DEFERRED_SCOPE.md
- docs/RUN_M1_AUTONOMOUS_FIXTURE.md

Read-only: main.rs; database_wait.rs; m1_run_control.rs; m1_run_inference.rs; m1_inference_control.rs; m1_reasoning_cli.rs; transport/transport_control; ContextPack/proposal/Terrain producers dan consumers; migrations 0001–0019; CI; accepted PRD/ADR/contracts; tests/support/database_wait_proxy.rs; tests/m1_run_postgres.rs dan tests/m1_run_failures.rs. Kebutuhan edit tambahan harus disebutkan dengan sebab konkret sebelum memperluas map.

## 4. Keputusan implementasi

### A. Preflight konfigurasi, bukan kewenangan baru

Setelah request valid dan sebelum store.start/hash/spawn, parse konfigurasi owner yang diperlukan alur admitted: core, Broker, Cognition, Trajectory dan Terrain. Gunakan parser dan pilihan env-local/file yang sudah ada; jangan mewajibkan URL env pada deployment file, jangan membaca kedua sumber atau menambah fallback. Invalid/missing/conflict harus menolak dengan kategori tetap dan label owner tertutup, tanpa nilai konfigurasi.

Periksa SystemRoot pada Windows dan keberadaan konfigurasi control-root untuk provider profile. Kualifikasi root aktual tetap pada boundary yang ada. Preflight bukan pengganti koneksi/role qualification atau pemeriksaan kewenangan saat digunakan. Tidak perlu membuka koneksi tambahan untuk preflight.

Jangan membuat API key wajib untuk start: regresi offline live_key_missing yang durable tetap didukung. Inspect/recover/stop tetap tidak memerlukan key maupun seluruh konfigurasi start. Jangan mengubah input validation precedence yang sudah diuji.

### B. Gunakan pipe yang sudah ada untuk terminal error aman

Pertahankan READY persis sebagai frame pertama untuk jalur yang berhasil. Jangan menerima atau melewati prelude, log, stage marker, JSON receipt atau frame salah sebelum READY.

Pembaca provider stdout membedakan:
1. exact READY yang cocok run/phase;
2. satu baris terminal error=<code> sebelum READY: kegagalan child, BUKAN READY;
3. setelah READY, terminal error yang diizinkan atau EOF;
4. invalid/oversize/late/absent frame: penolakan protokol.

Gunakan batas 128 byte per frame termasuk LF dan batas drain total yang sudah ada. Code harus berasal dari allowlist eksplisit: missing_env, invalid_database_config_mode, database_config_conflict, missing_database_url_file, database_file_platform_unqualified, invalid_dsn, non_loopback_host, missing_credentials, connect_failed, unqualified_database_waits, run_control_unqualified, run_control_stopped, run_child_refused, inference_authority_unavailable, run_inference_pending, child_failed. Unknown code dipetakan ke child_failed; nilai unknown aslinya tidak disimpan atau dipantulkan. Jangan memakai kecocokan regex sebagai bukti bahwa suatu string aman.

Reuse reader dan watchdog yang sudah ada; jangan menambah service/thread diagnostik. stderr tetap dibuang. Jangan menyimpan stdout utuh, panic text, SQL text, provider response atau private reasoning.

Pisahkan observed exit status dari reported child code dan parent-observed cause. Untuk hard exit tanpa frame, alasan child boleh unavailable. Exit 124 saja tidak boleh diterjemahkan menjadi preparation_timeout. Bila parent sendiri mengamati readiness deadline/phase deadline/local stop, laporkan fakta itu dengan sumber parent; jangan mengaku mengetahui watchdog internal mana yang menang.

Tuntaskan race exit-vs-reader dengan drain yang dibatasi sisa deadline asli, bukan join tanpa batas atau grace clock baru. Bila informasi tidak sempat tersedia, laporkan unavailable secara jujur. Kegagalan pipe/diagnostik tidak pernah mengizinkan READY atau release.

### C. Diagnosis durable kecil dan terpisah dari status efek

Tambahkan satu record final per (run_id, phase), append-only/idempotent, melalui adapter PgRunStore dan forward migration 0020. Kontrak internal m1_run_phase_diagnostic_v1 berisi:
- run_id, phase (enum yang ada);
- cause: child_error | child_exit | readiness_refused | readiness_deadline | phase_deadline | local_stop | supervisor_unavailable | unavailable;
- cause_source: child_report | parent_observed | unavailable;
- child_error: allowlisted code di atas atau null;
- exit_code: signed 32-bit integer sesuai status.code() atau null bila tidak tersedia; jangan membuang Windows exception code yang berada di luar 0..255;
- readiness_observed: boolean;
- elapsed_ms: waktu parent sejak phase launch, integer 0..150000 atau null jika tidak tersedia;
- publication disposition pada receipt: committed | pending_or_unknown | unavailable.

Maksimum payload diagnosis 1024 UTF-8 byte, tanpa field/text bebas. Informasi child dilabel sebagai laporan child, bukan fakta authority. Cause final berarti observasi parent, bukan rekonstruksi akar penyebab yang belum diukur.

SQL memverifikasi run/phase dan writer Broker pemilik run, search_path tetap, enum/bounds/unknown-key rejection. Tidak ada direct write grant, cross-campaign write, pengubahan record lama atau perluasan role. Retry identik idempotent, conflict menolak. Diagnosis setelah expiry tidak membutuhkan authority baru untuk bertindak: hanya accountable metadata tentang run yang sudah ada; tidak boleh mengubah admission, charges, phase state atau history.

Parent mencatat setelah child berhenti melalui jalur terminal yang dibatasi wait/run budget yang ada. Jangan melakukan SQL/logging dari reaper, callback atau watchdog. Kegagalan publikasi diagnosis tidak menghambat STOP/reap dan tidak mengubah kegagalan menjadi sukses.

Tambahkan diagnostic opsional pada summary/inspect tanpa mengubah arti field existing. Run lama tanpa record menampilkan unavailable; jangan backfill penyebab. Pertahankan UNKNOWN bila efek/commit belum pasti. Recovery tetap DB-only tanpa relaunch/inference/effect replay; tidak boleh merekayasa diagnosis yang hilang.

Bootstrap initial/refresh harus menyertakan 0020: 18 initial, 9 refresh. Perbarui helper assertion dan run-support refresh agar definisi run_summary lama tidak menimpa 0020. Gunakan fungsi baru yang sempit; jangan menyalin seluruh migration lama.

### D. Supervisor: kurangi pekerjaan yang tidak perlu, jangan menyatukan failure domain

Tidak ada framework supervisor, scheduler, event bus, pool global, tracing subsystem, async conversion atau thread per sample. Pertahankan pemisahan:
- parent independent reaper vs SQL loop/pipe;
- bounded readiness supervision;
- local STOP observer;
- child preparation cover;
- authority coordinator vs transport-local permit guard;
- parent EOF detection dan whole-decision envelope.

Mekanisme berbeda tidak otomatis redundant: dokumentasikan pemilik, periode hidup, operasi yang dapat block dan failure yang dicakup dalam packet ini, bukan dokumen tracking baru.

FIX ini tidak mengganti PowerShell ACL, menghapus hash child, membagi koneksi antar-role/thread, menghapus prepared_basis kedua, atau menaikkan deadline. Dua TerrainBasis saat ini dipakai consumer berbeda; deduplikasi naif dapat merusak independensi coordinator.

Simpan pengukuran launch→claim→READY/exit dan durasi parent total dari focused offline scenarios. Pisahkan waktu child dari waktu persiapan parent dan accounting; ketidaktersediaan READY adalah censored observation, bukan durasi tahap internal. Jangan menyebut warm run sebagai cold-start benchmark, menghitung overhead pasti dari durasi provider, atau mengklaim startup <500 ms tanpa bukti. Jika diperlukan profiling internal lebih jauh, catat gap konkret; jangan menambah debug mode produksi di luar map.

## 5. Acceptance dan verifikasi

Mulai dengan failing assertion asli untuk error yang hilang pada baseline; jangan membuat fault baru di produksi untuk mengarang RED. Gunakan synthetic child/parser tests untuk framing dan actual m1-run entrypoint untuk konfigurasi/DB.

Wajib:
1. Core/history/Terrain config hilang, malformed dan conflict: kategori+owner aman; nol run/reservation/call/effect baru. SystemRoot Windows yang hilang gagal sebelum DB; argumen invalid tetap error=invalid_args.
2. Kasus fresh child setelah start yang memang gagal: kode allowlisted terlihat di terminal receipt dan fresh inspect ketika DB tersedia. Race reader/exit tidak mengubah kode menjadi hasil acak. Protocol parser tested before dan after READY.
3. Semua exact-readiness negative controls tetap menolak: prelude, wrong UUID/phase, padded, CRLF, invalid UTF-8, oversized, truncated, EOF dan late frame. Terminal error tidak pernah dianggap READY. Synthetic secret sentinel pada unknown code tidak keluar ke receipt/DB/report.
4. Exit 124 tanpa cause marker tidak diklaim sebagai preparation_timeout. Parent-observed deadline dibedakan. Actual blocked-pipe/SQL/child-stop regressions tetap membuktikan independent reap dan callback purity.
5. SQL direct-call negative controls: foreign writer, wrong run/phase, unknown code/key, oversize dan conflicting duplicate menolak. Identical publication idempotent; existing history/status/charges utuh.
6. DB unavailable saat diagnostic publication: child sudah berhenti, hasil pending_or_unknown/unavailable jujur; tidak ada replay saat recovery.
7. Deterministic skeleton tetap dua keputusan/fixture/Terrain/history lewat real entrypoint. Provider profile offline tetap durable live_key_missing, tanpa fallback. Jangan menulis provider fixture sukses sebagai real LLM cognition.
8. owner_view_requested tetap terminal yang berbeda dari completed. Jangan memaksa pilihan DNS atau menyisipkan operator continuation.

Urutan hemat waktu:
- cargo fmt -- --check
- python -m unittest discover -s tests -p test_m1_native_helpers.py
- cargo test --locked --test m1_run_cli
- Dalam satu fresh restricted fixture/bootstrap session, jalankan cargo test --locked --bin duskweave, kemudian target m1_run_diagnostics, m1_provider_fixture, m1_run_postgres, m1_run_failures secara serial (--test-threads=1); reuse database fixture yang sama hanya sesuai mekanisme refresh test, bukan DB demo. Semua command Rust memakai --locked; pastikan tidak ada key dalam environment setiap subprocess.
- cargo clippy --locked --all-targets -- -D warnings
- python scripts/check_links.py dan python scripts/check_structure.py
- complexity ratchet dengan analyzer 0.0.25 yang sudah terverifikasi: --base b439661f82c4cebf1d615629f07eb176dd3a53e8 dan report directory packet. Jangan install/update tool global atau mengganti analyzer; tool unavailable bukan PASS.

CI menjalankan full suite dan coverage yang dipersyaratkan, termasuk focused Linux reservation-stall. Pertahankan 90% total/80% per-file, McCabe ratchet dan semua failure assertions. Lokal bukan full-suite Windows PASS bila legacy CREATEROLE gate tidak tersedia. Jangan mengulang full test lokal tanpa risiko baru; jangan menjalankan role-mutating fixtures paralel.

## 6. Tracking, review dan STOP

Tracking UPDATE:
- ENGINEERING_STATE: outcome FIX/next action dan remaining provider-loop blocker; tidak mencatat merge/seal diri sendiri.
- MVP roadmap §1.4: disposition diagnostik/debt yang disentuh; ACL latency, owner-connection optimization, internal timing gap dan owner-view continuation tetap OPEN kecuali terbukti dikerjakan.
- Guide: config requirements, arti diagnostic/UNKNOWN, no-replay recovery dan perbedaan satu keputusan vs dua keputusan.
- Packet: RED/GREEN, SHA/platform, exact commands/results, byte bound, supervisor fault map dan remaining uncertainty. Referensikan bukti historis; jangan duplikasi seluruh laporan.

Review arsitektur sebelum style memakai QUALITY_BAR §7. LOC/diff/function span advisory; McCabe enforced. Tidak ada helper fragmentation untuk menurunkan angka, perubahan threshold, atau penghapusan consumer lama yang belum punya replacement.

Satu author self-check lalu satu distinct adversarial pass oleh reviewer berbeda dari implementor; laporkan provenance jujur, bukan self-review berlabel independen. Perbaiki valid findings dalam PR yang sama, final-head checks, lalu kirim PR untuk owner; jangan auto-merge. Bukan kewajiban meminta user menilai setiap detail code.

STOP untuk authority conflict, base drift yang belum direkonsiliasi, kebutuhan file/dependency di luar map, invariant violation atau required gate yang tetap gagal setelah dua FIX cycles. Routine compile/lint/test failure diperbaiki dalam scope. Bila preparation positif tetap gagal, bawa measured failure ke architecture review; jangan menaikkan timer atau mengganti fixture agar hijau.

Definisi selesai: entrypoint→child terminal observation→restricted append-only diagnostic→fresh inspect terbukti, pengaman dan skeleton tidak regresi, required CI green. Klaimnya FIX diagnostik terkualifikasi, bukan root cause episode lama terpecahkan, real loop sukses, izin target atau seal M1.

## 7. Implementation evidence (2026-10-21)

Verified repository: `carlitotate12160-tech/DuskWeave`, actual worktree
`D:/DuskWeave-ci-complexity`, branch `fix/m1-run-diagnostics`, issued base
`b439661f82c4cebf1d615629f07eb176dd3a53e8`. GitHub master matched that base at
preflight. The current committed head, PR state and final CI come from Git/GitHub;
this candidate does not contain its own head SHA or claim its own merge.

Windows x86_64 MSVC / Rust 1.94.1; owned `dwpg-r3` retained, pinned image and
127.0.0.1:5432 binding requalified, PostgreSQL 17.11. No competing cargo/rustc/
duskweave verification owner was running at preflight. Ordinary target is the
issued `D:/DuskWeave-build/rust-1.94.1/normal`, held by this execution across
builds and native subprocess tests. No local coverage was run. Reports are under
`D:/DuskWeave-build/rust-1.94.1/reports/m1-run-diagnostics-20261021a`.

The first bootstrap retained `dw_native_run_diagnostics_20261021a` after a new
0020 function syntax error. This was setup failure, not product RED. Credentials
were not recovered. After correction, the helper generated the fresh fixture
`dw_native_fixture_ebc5531ddb0afa40`; its restricted credentials remain only in
the live bootstrap and child environments. Initial migration count is 18 and
refresh count 9. `DEEPSEEK_API_KEY` was removed explicitly before bootstrap,
tests and probes, and the bootstrap removes it again. No provider/target contact
or historical run restart was performed. All retained databases/roles survive.

**RED:** before production edits, the added assertion
`m1_run_supervision::tests::child_terminal_error_before_ready_preserves_the_safe_code`
ran through `cargo test --locked --bin duskweave` and failed because the baseline
returned `run_readiness_refused` for `error=missing_env` instead of preserving
the admitted child code. Compilation/setup succeeded; one test failed at the
behavior assertion. The same assertion then passed after the reader fix.

**Wiring:** `main` existing dispatch → `m1_run_cli::start` → parse-only owner
preflight → existing run admission → `PhaseChild` reaper/readiness observation →
stop/reap and original-deadline-bounded terminal drain → `PgRunStore` → restricted
`execution.run_phase_diagnostic` → existing summary plus narrow diagnostic
projection → fresh `m1-run-inspect`. No production command/export exists only
for tests. Inspect/recover/stop retain the existing Broker-only composition;
recovery does not publish guessed diagnostics or restart children.

**Byte and trust boundaries:** frames are at most 128 bytes including LF and
the entire reader consumes at most the existing 16384-byte drain ceiling.
Successful readiness still requires the exact first frame. One terminal error
is admitted before or after READY; extra/malformed frames refuse. Unknown codes
become `child_failed` at the pipe boundary. Only the closed 9-field record is
retained; the SQL boundary rejects unknown keys/codes and payloads over 1024
UTF-8 bytes. SQL checks signed 32-bit status and elapsed 0..150000/null, writer
identity and an existing started phase. The only runtime write route is the
security-definer publication function with fixed search_path. Identical retry
is idempotent; conflicting publication and direct mutation refuse. Separate
trust-boundary validation is deliberate, rather than duplicate authority.

| Supervisor owner | Lifetime / potentially blocking operation | Failure covered |
| --- | --- | --- |
| Parent independent reaper | Phase launch to stop/drop; owned child kill/wait, no SQL/pipe read | Parent SQL loop or output reader stall |
| Readiness reader/watchdog | Provider decision launch to exact READY/refusal/original preparation bound; reader may block on stdout | Prelude, invalid/absent/late frame; terminal output projection |
| Parent local STOP observer | Provider child lifetime; qualified local control check, no SQL | Local stop before/while parent SQL waits |
| Child preparation cover | Child entry to armed provider/envelope; monotonic deadline only | Blocked hash/claim/owner setup before READY |
| Authority coordinator | One bounded decision; one restricted SQL check at a time | Missing/late current authority or premises |
| Transport-local permit | Actual transfer; latch and monotonic expiry only in callback | Coordinator unavailable/stalled, local revocation; no SQL/history in callback |
| Parent EOF / decision envelope | Child inference lifetime / original decision deadline | Parent death and blocked cooperative abort |

The parent publishes only after owned child termination. Publication failure
cannot postpone STOP/reap or turn a failed child into success. Local pending
records retain each phase independently; a later successful publication cannot
erase an earlier pending phase. Fresh durable summary marks missing records
unavailable, including old runs. Existing effect/charge/history/status fields
retain their producer; 0020 wraps the old summary without copying its body.

**Focused local checks:** format, 28 Python helper tests, 2 CLI tests, 4 diagnostic
tests, 12 provider-fixture tests and 2 real-entrypoint PostgreSQL skeleton tests
passed on the working candidate. Binary tests passed 51/51 before the final
lifecycle refactor and are repeated below through the retained bootstrap.
Failure-target, final committed-head checks and CI results must be read from
their actual subsequent runs; earlier local results are not transferred to a
later commit automatically. Clippy all-targets passed; link/structure checks
passed with advisory size warnings. The pinned analyzer 0.0.25 ratchet passed
against the issued base: ten retained debt items, none worsened, no cap change.
The final working-source serial run also passed binary 51/51 and failure target
14/14. Windows legacy full-suite CREATEROLE remains unqualified; Linux CI supplies the
required full-suite/coverage and focused reservation evidence for its candidate.

Commands executed (each Rust process has the issued ordinary target; Python was
`C:/Users/ekoku/AppData/Local/Programs/Python/Python312/python.exe`):

```text
cargo fmt -- --check
python -m unittest discover -s tests -p test_m1_native_helpers.py
cargo test --locked --test m1_run_cli
cargo test --locked --bin duskweave -- --test-threads=1
cargo test --locked --test m1_run_diagnostics -- --test-threads=1 --nocapture
cargo test --locked --test m1_provider_fixture -- --test-threads=1 --nocapture
cargo test --locked --test m1_run_postgres -- --test-threads=1 --nocapture
cargo test --locked --test m1_run_failures -- --test-threads=1
cargo clippy --locked --all-targets -- -D warnings
python scripts/check_links.py
python scripts/check_structure.py
python scripts/check_complexity.py --analyzer D:/DuskWeave-build/rust-1.94.1/tools/rca-0.0.25-pr22/bin/target/release/rust-code-analysis-cli.exe --base b439661f82c4cebf1d615629f07eb176dd3a53e8 --report-dir D:/DuskWeave-build/rust-1.94.1/reports/m1-run-diagnostics-20261021a/complexity
```

The five DB-dependent targets above ran serially inside one retained bootstrap
session. Reports `diagnostic-final.log`, `provider-final.log`,
`run-postgres-final.log`, `run-failures-final.log`, `bin-confirmed.log` and the
complexity report contain the actual local results, including prior setup/test
corrections rather than presenting those attempts as PASS. Clippy initially
reported collapsible conditionals and the ratchet caught three new regressions;
those were corrected within the map and the subsequent gates passed.

**Observed timings (one warm sample each, milliseconds):** offline missing-key
parent total 3009, launch-to-terminal observation 1933, READY observed; publication
lock failure parent total 3276, child observation 1323, pending receipt and fresh
no-replay recovery passed. Parked claim reply: parent launch-to-barrier 678,
parent total 5572, phase observation 5003, no READY/claim observed, with parent
`readiness_deadline`. Parked reservation: decision anchor 507 after child launch
(3 ms query), parent total 46096, child observation 45623, READY observed,
exit 124 retained as `child_exit`, internal cause unavailable. These samples
are observations, not cold-start benchmarks, query latency or exact overhead.
READY timestamps and internal hash/ACL/owner/context/coordinator decomposition
remain unavailable; an absent READY is censored. Safe phase timestamps are in
the retained receipts. No claim of startup <500 ms or historical root cause.

**Author review, QUALITY_BAR section 7:** Q1/Q2: lifecycle observation stays in
supervision, configuration in its parser, immutable metadata in the Broker
adapter; no sibling model mutation or new dependency. Q3/Q4: the existing reader,
watchdogs, admission and summary producer are retained, without a framework or
new timer; process settlement and typed projection are separate responsibilities.
Q5/Q6: the explicit map and real-entrypoint/fresh-inspect assertions cover the
requested path. Q7/Q8: direct SQL negatives, code sentinel, signed Windows exit,
framing/race/deadline/reap and no-replay controls exercise the changed boundaries.
Distinct reviewer evidence and exact-head CI remain separate from this author
assessment. The enlarged supervision file retains one lifecycle owner plus its
cohesive parser/process scenario tests; existing provider/support test files
retain their shared fixture responsibility. LOC/span warnings are retained for
review, not bypassed through unrelated helpers or threshold changes.

Tracking UPDATE: ENGINEERING_STATE points to this active FIX/next action; roadmap
section 1.4 records its bounded disposition and remaining OPEN debt; the operator
guide explains configuration, diagnostics/UNKNOWN and database-only recovery.
No BUILD_ORDER edit is needed: stage/navigation and accepted scope are unchanged.
PowerShell ACL latency, owner-connection optimization, internal timing gaps,
owner-view continuation, real provider loop and live qualification remain OPEN.

## 8. Distinct review and correction R1

One distinct agent `/root/diagnostics_review` reviewed committed `ca95f56`
against the issued base, using the repository adversarial-review skill. It
performed source/report review only (no build, DB or edits); the author ran the
reported local tests. The pass found one VALID blocking issue: reader completion
could precede the readiness watchdog verdict. A fast child could exit zero after
exact READY while the parent still had no final deadline-qualified verdict;
`protocol_valid` could accept that pending state. The original source therefore
remained NOT READY despite its passing focused tests. No other source defect
survived the full pass; final CI remains separately required.

**R1 RED:** `cargo test --locked --bin duskweave
reader_completion_without_a_watchdog_completion_cannot_finalize_success --
--test-threads=1` failed its actual assertion: `ChildExit` was returned instead of
`Unavailable` when only reader completion was available. This is a synthetic
completion-order fixture, without a fault added to production.

**One cohesive correction:** both existing reader and watchdog send completion;
settlement consumes both within the remaining original phase deadline before
snapshot/protocol acceptance. A required readiness verdict must be positively
observed; missing completion cannot qualify success. No new thread, grace clock,
deadline or SQL in a watchdog was added. The gated late-watchdog test uses the
actual reader, actual successful child, queued exact frame and production
`readiness_watch`; it proves pending acceptance refuses and the final diagnostic
is `readiness_deadline` when the watchdog processes the frame after its bound.
The test first needed a fixture correction to wait for actual process exit after
pipe EOF; that setup failure is not claimed as product RED.

The R1 focused supervisor run passes 10/10; current-head broader regressions,
review of this delta and Linux CI/coverage are the remaining delivery evidence.
The previous committed head's local checks passed 85 Rust/28 helper assertions;
they are historical candidate evidence, not qualification of this correction.
