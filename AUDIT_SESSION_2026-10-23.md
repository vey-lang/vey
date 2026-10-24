# Vey Derin Denetim — Oturum Raporu

**Tarih:** 23 Ekim 2026  
**Depo:** `vey-lang/vey`  
**İncelenen HEAD:** `7ba80adf8bd0c7a1308b77e0eea24202445511f0` (`main`, `origin/main`)  
**Başlangıç çalışma ağacı:** Temiz (`main...origin/main`).  
**İlk oturum sonu çalışma ağacı:** README düzeltmesi ve bu rapor; takip değişiklikleri aşağıdadır. Commit veya push yapılmadı.

## Kısa sonuç

İlk denetimde README/runtime test beklentileri düzeltildi; tekrarlı exit listener giderildi ve cache'e kararlı compiler fingerprint eklendi. Type-soundness ve tooling bulgularına ek olarak cross-file LSP definition/references/rename; expression üyeleri, qualified markup component etiketleri, qualified type/enum adları, enum variant'ları ve literal-string computed member'lar (escaped keys dahil) için tamamlandı. Ayrı type-symbol namespace'i yerel type alias ve type parameter kullanımlarını da izliyor. Scope'a bağlı imported-alias gölgelemesi düzeltildi; checked module namespace'leri, fixed object/tuple şekilleri, `unknown` ve indekslenemeyen primitive/union dallarında computed erişimlerin örtük `any`'e düşmesi engellendi. List/string/dictionary okumaları olası `undefined` değerini taşır; let-else başarı dalı bunu daraltır ve yazma hedeflerinde eleman/değer tipi korunur. Optional chaining JavaScript'in nullish `undefined` sonucuyla hizalandı; ordinary nullable member/call erişimleri narrowing ister. Recursive generic alias placeholder'ları tür argümanlarını koruyarak alan tiplerini doğrular. Son `cargo test --all-targets` **402 geçti, 0 başarısız**. Açık `any` ve unchecked npm import'larının dinamik davranışı korunuyor; performans ölçülmedi.

## A. Doğrulanmış bulgular

| Öncelik | Alt sistem | Kanıt / minimal gözlem | Neden | Durum |
|---|---|---|---|---|
| P2 | README / geliştirici dokümantasyonu | `cargo test --test readme` başlangıçta `every_file_of_the_project_is_in_the_layout` testinde `src/cli/program.rs` eksikliği nedeniyle başarısız oldu. Aynı envanter taraması `src/cli/project.rs` adının da README'de bulunmadığını gösterdi. | Proje yerleşimi tablosu iki var olan modülü güncel tutmamış. | Düzeltildi: ikisi de README'ye eklendi. `cargo test --test readme` 4/4 geçti. |
| P1 araştırma gerekli | Runtime / hata yanıtları ve dosya sunumu | İlk all-targets çalışmasında altı runtime testi beklenenden farklı hata yanıtı/route davranışı nedeniyle başarısız oldu; seri tekrar da aynı sonucu verdi. | Takipte README HTTP sözleşmesi ve route önceliğiyle karşılaştırma, testlerin eski beklentilerini ortaya çıkardı; düz metin hata testi için `Accept` başlığı ve doğru route/statik davranış beklentileri güncellendi. | **Kapatıldı:** düzeltme sonrası tüm runtime ve tüm hedef testleri geçti; statik dosya güvenlik kontrolleri zayıflatılmadı. |
| P3 | Compiler cache / bakım | Test derlemesi `src/compiler/cache.rs` içinde kullanılmayan `compiler_stamp()` için `dead_code` uyarısı verdi; depoda başka çağrısı yoktu. | Cache anahtarından compiler stamp'i çıkarılmıştı, fakat mtime tabanlı fonksiyon kalmıştı. | **Kapatıldı:** ölü fonksiyon kaldırıldı ve yerine compiler/build girdilerine dayalı kararlı fingerprint cache key'e eklendi; hit/miss doğrulandı. |

## B. Yapılan değişiklikler

- `README.md`: proje yerleşimi tablosuna `src/cli/program.rs` ve `src/cli/project.rs` eklendi. Dil davranışı veya kullanıcıya açık API değişmedi.
- `AUDIT_SESSION_2026-10-23.md`: bu denetim oturumunun bulguları ve doğrulama durumu kaydedildi.

## C. İlk test kanıtları (tarihsel başlangıç durumu)

Komutlar Rust araç zincirini PATH'e ekleyerek çalıştırıldı (`/usr/local/rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin`).

- Başlangıç `cargo test --all-targets`: README envanter testi başarısız oldu (`src/cli/program.rs` eksik); o noktaya kadar main unit 8, CLI 48, dev 9, DOM 15 ve lang 64 test geçti.
- Değişiklik sonrası `cargo test --test readme`: **4 geçti, 0 başarısız**.
- Değişiklik sonrası `cargo test --all-targets`: **tamamlanmadı / başarısız**. `tests/runtime.rs`: **94 geçti, 6 başarısız**. Hatalar arasında 413/400 yanıt gövdesi beklentileri, `sendFile`/404, statik dosya/symlink ve dosya-önceliği vakaları var. Daha önceki hedeflerde main unit 8, CLI 48, dev 9, DOM 15, lang 64; README 4 ve regressions 43 geçti.
- `cargo test --test runtime -- --test-threads=1`: aynı altı başarısızlık seri koşuda da görüldü.
- `cargo test --test server`: **10 geçti, 0 başarısız**.
- `git diff --check`: geçti.

Bu maddeler ilk denetim aşamasındaki başarısız baseline'ı kaydeder; son takip doğrulaması 390/390 başarılıdır ve raporun son bölümünde yer alır.

## D. Veyboard ve tasarım gözlemleri

Veyboard README ve `src/site.vey` gözden geçirildi. README üç açık sınırlamayı doğrudan belgeliyor: markup'ta `html`/`head`/`body` etiketlerinin bulunmaması, component gövdesindeki değerlerin prop'lar bağlanmadan önce üretilmesi ve `for` sözcüğünün keyword olması nedeniyle form alanlarının label içine sarılması. `site.vey` bu tercihlerin somut kullanımını gösteriyor. Mevcut kanıt bunları **belgelenmiş dil tasarımı/ergonomi sınırlamaları** olarak sınıflandırmaya yeterli; bu oturumda derleyici bug'ı oldukları kanıtlanmadı ve Veyboard yeniden yazılmadı. Sunucu tarafı formlar ve tek süreçli kullanım uygulama tercihleri olarak kalıyor.

## E. İlk oturum sonunda kalan işler (takipte güncellendi)

İlk oturum sonunda runtime test sözleşmesi, cache fingerprint ve daha geniş doğruluk/tooling taraması açık bırakılmıştı. Sonraki bölümlerde runtime test beklentileri belgelenmiş HTTP sözleşmesine göre düzeltildi, cache riski kaynak/build fingerprint'iyle kapatıldı; type-soundness, tooling ve LSP çapraz-dosya taraması genişletildi. LSP artık doğrudan static expression üyelerini (`m.area`, `m["area"]`), qualified markup components (`m.Card`), qualified type/enum adlarını (`m.Box<T>`, `m.Color`) ve enum variant segmentlerini (`m.Color.Red`) kapsar. Yerel type alias/type parameter adları da ayrı type namespace'inde izlenir. Dynamic computed keys (`m[key]`) ve açık olmayan belgeler için genel workspace index'i kapsam dışındadır. Önceki `AUDIT_START.md` gözlemleri tek başına bu oturumda doğrulanmış sayılmaz.

## F. Performans

Bu oturumda dosya değişikliği algılama, derleme, süreç yeniden başlatma veya sunucu hazır olma fazları ölçülmedi. Dolayısıyla 600 ms iddiası için yeni bir ölçüm veya önce/sonra karşılaştırması raporlanmıyor. HEAD'deki `7ba80ad` cache değişikliğinin restart etkisine ilişkin nedensel performans sonucu burada ölçülmedi.

## G. Git özeti

- Denetim başlangıç HEAD'i: `7ba80adf8bd0c7a1308b77e0eea24202445511f0`.
- İlk audit commit/push: `0e46312` (`main` -> `origin/main`, başarılı).
- Post-push LSP devamı: `565b3cf` ile çapraz-dosya navigasyon genişletildi; bu devamda enum variant ve local type-symbol kapsamı eklendi.
- Son doğrulama: `cargo test --all-targets` **390 geçti, 0 başarısız**; `git diff --check` temiz.


## Takip denetimi — 23 Ekim 2026, 19:40 (+03:00)

### Önceki altı runtime başarısızlığının kök nedeni ve çözümü

Altı test ayrı ayrı önceki beklentileriyle yeniden çalıştırıldı; her biri yeniden üretildi. `src/runtime/server.js` ile README'deki 1394–1416 satırları karşılaştırıldığında, runtime'ın statik dosya/path güvenliği veya durum kodları değil, beklenti sözleşmelerinin güncelliğini yitirdiği doğrulandı:

| Test | Kanıtlanan neden | Düzeltme |
|---|---|---|
| `a_chunked_body_that_is_too_large_gets_a_413_response` | `http.request` varsayılan `Accept: */*` gönderiyordu. Runtime `wantsHtml()` ile bunu HTML istemi sayıyor; 413 ve handler'ın çalışmadığı bilgisi doğruydu. README hata temsilini Accept başlığına bağlar. | İsteğe açık `Accept: application/json` eklendi; belgelenen düz metin 413 başlığı/mesajı ve `handled:false` doğrulanıyor. Body limiti veya bağlantı güvenliği değiştirilmedi. |
| `malformed_json_body_is_rejected_before_the_route_runs` | Aynı varsayılan `Accept: */*`, 400 durumunu Vey hata sayfası biçiminde döndürüyordu; route çalışmıyordu. | Açık Accept ile düz metin 400 ve `handled:false` doğrulanıyor. |
| `response_can_send_files_and_downloads_asynchronously` | Eksik `sendFile` 404'ü için test `Not found` varsayıyordu; runtime, `Accept` başlığına göre hata başlığı ve açıklaması döndürüyordu. | İstemci düz metin yanıtı açıkça seçiyor ve belgelenmiş 404 metnini denetliyor. Dosya indirme ve 200 gövdeleri de aynı testte korunuyor. |
| `static_file_open_rechecks_the_resolved_target` | Symlink değişiminde runtime 403 döndürüp dosyayı reddediyordu; test sadece eski `Forbidden` gövdesini bekliyordu. | Düz metin temsilini seçip 403 metnini ve `swapped:true` bilgisini kontrol ediyor. Güvenlik denetimi aynen kaldı. |
| `static_range_open_rejects_a_swapped_outside_symlink` | Aynı eski `Forbidden` gövde varsayımı; fiili status 403 ve erişim reddi doğruydu. | Düz metin 403 mesajını ve `swapped:true` kontrol ediyor; range/symlink güvenlik kodu değişmedi. |
| Önceki `the_files_of_a_folder_are_served_before_the_routes` | Test adı ve beklentisi README'nin açık sözleşmesinin tersiydi. `app.static()` eşleşmeyen route'lar için fallback'tir; mevcut route dosyayla aynı yolu alır. `server.js` içindeki `notFound()` static fallback'inin route eşleşmesinden sonra çağrılmasıyla da uyumlu. | Test `a_route_wins_against_a_file_of_the_same_name` olarak adlandırıldı ve route gövdesini doğruluyor. |

Dolayısıyla bu altı vaka **runtime hatası değil, test beklentisi/fixture kusuru** olarak sınıflandırıldı. Güvenlik davranışını gevşeten veya testleri yalnızca yeşil yapmak için eşikleri değiştiren bir düzeltme yapılmadı.

### Yeni doğrulanmış runtime hatası ve düzeltmesi

Veyboard'ın ilk `vey test` çalıştırması, `tests/store.vey`, `tests/tasks.vey` ve `tests/pages.vey` sırasında Node `MaxListenersExceededWarning` üretti. Kök neden `src/runtime/core.js` içindeki `test()` fonksiyonunun her çağrıda aynı `printSummary` callback'ini `process.on("exit", ...)` ile yeniden kaydetmesiydi. On iki test içeren minimal runtime repro'su düzeltme öncesi exit listener sayısının **12** olduğunu gösterdi.

`test_summary_installs_only_one_exit_listener` regresyon testi eklendi; `test()` artık `summaryListenerInstalled` bayrağıyla listener'ı tek kez kuruyor. Hedefli test geçti; aynı Veyboard suite'i ikinci kez koştuğunda `MaxListenersExceededWarning` görülmedi. Bu, işlev değişikliği ve regresyon testiyle kapatılmış **P2 runtime/bakım hatasıdır**.

### Compiler cache ve `compiler_stamp()` kararı

Kaynak ve Git geçmişi doğrulandı: `compiler_stamp()` için aktif çağrı yok; HEAD'deki `key()` yalnızca paket sürümünü, kaynak yolları/içeriğini ve stil yolları/içeriğini hash'liyor. Önceki cache kodu ise executable mtime'ını anahtara ekliyordu. HEAD commit açıklaması bu mtime'ın geliştirme sırasında cache'i her derlemede geçersiz kıldığını ve kaldırmanın yaklaşık 600 ms'den 130 ms'ye düşürdüğünü iddia ediyor; bu oturumda bu performans rakamları bağımsız ölçülmedi.

Fonksiyonun çağrılmıyor olması doğrulandı, fakat bu oturumda silinmedi: aynı semver `CARGO_PKG_VERSION` ile oluşturulmuş farklı compiler binary'leri proje kaynakları değişmemişse aynı cache anahtarını üretebilir. Böylece compiler/codegen değişikliğiyle eski JavaScript bundle'ı yeniden kullanma riski, mtime stamp'inin kaldırılmasıyla görünür hâle geliyor. Kaynağın yorumunda fonksiyonun özellikle “referans için” tutulduğu da yazıyor. Ölü kod uyarısını kaldırmak için fonksiyonu silmek bu açık cache doğruluğu riskini gizleyebileceğinden güvenli sayılmadı. Sonraki adım, restart'lar arasında kararlı fakat compiler kaynakları değişince farklılaşan build fingerprint tasarlamak ve cache hit/miss regresyonları eklemektir; salt fonksiyon silme yapılmamalı.

### Geniş denetimde bu oturumda yapılan ek tarama

Salt okunur type/LSP incelemesi, `tests/types.rs` içindeki 80 testi ve `tests/cli.rs` LSP kapsamını (6 test) gördü; denetlenmiş alanlarda önceki raporlardaki çalışmaları yinelemeyen yeni ve minimal reproducer'la doğrulanmış bir correctness bug bulmadı. Bu sınırlı tarama, tüm tip sistemi veya LSP'nin eksiksiz denetlendiği anlamına gelmez. Veyboard bu oturumda gerçek tüketici olarak ayrıca `check` ve `test` komutlarıyla çalıştırıldı.

### Takip test kanıtı

- Altı düzeltme testi, `cargo test --test runtime <ad> -- --exact` ile **tek tek 1/1 geçti**.
- Altı beklenti düzeltmesinden sonra `cargo test --test runtime --quiet`: **100 geçti, 0 başarısız**.
- Listener regresyonunu ekleyip düzelttikten sonra `cargo test --test runtime test_summary_installs_only_one_exit_listener -- --exact`: **1 geçti, 0 başarısız**.
- Son `cargo test --all-targets`: **382 geçti, 0 başarısız**. Alt hedefler: main unit 8, CLI 48, dev 9, DOM 15, lang 64, README 4, regressions 43, runtime 101, server 10, types 80.
- `cargo build --quiet`: geçti; kullanılmayan `compiler_stamp()` için `dead_code` uyarısı devam ediyor.
- Veyboard dizininde `../../target/debug/vey check`: **19 page checked**.
- Veyboard dizininde `../../target/debug/vey test`: **83 test geçti, 0 başarısız; 8 dosya geçti**. İlk düzeltme öncesi koşudaki Node listener uyarısı son koşuda yoktu.
- Salt okunur audit ajanı: yeni minimal type/LSP bug doğrulanmadı; type 80/80, LSP kapsamı 6/6 gözlemi.

### Güncellenen git durumu ve sıradaki üç iş

Bu takipte `tests/runtime.rs` ve `src/runtime/core.js` değişti; bu rapor da güncellendi. Önceki README envanter düzeltmesi (`README.md`) korunuyor. Tüm değişiklikler yereldir; commit/push yapılmadı.

Öncelikli sonraki işler:

1. **P1 — Compiler cache fingerprint:** compiler kaynak kodu değişikliğinin aynı semver altındaki eski cache'i kullanmasını engelleyen kararlı build fingerprint ve test tasarla; geliştirme restart'larında cache hit'ini koru.
2. **P1 — Type soundness'ı genişlet:** dependent generic return inference, recursive/constrained aliases, async hata/yayılımı ve modüller arası sınırları yeni minimal pozitif/negatif testlerle incele.
3. **P2 — Tooling ve dil sözleşmesi:** project template'lerini ve `dev/build/run` entegrasyonunu, ayrıca LSP cross-file rename/source range davranışlarını hedefli testlerle denetle; Veyboard sınırlamalarını tasarım/ergonomi/stdlib/application kategorilerine ayrı tut.

Bu takipte dosya izleme/derleme/yeniden başlatma/ready fazları ölçülmedi. Performans iddiası doğrulanmış sonuç olarak sunulmuyor.


## Compiler cache devamı — 23 Ekim 2026, 19:44 (+03:00)

Önceki bölümde açık bırakılan cache doğruluğu riski bu devam çalışmasında giderildi. Yeni `build.rs`, `src/` altındaki compiler/runtime girdilerini ve yollarını deterministik sırada; `Cargo.toml`, varsa `Cargo.lock`, build script'ini, target/profile/feature ve Rust flag ayarlarını ve `rustc --version` çıktısını hash'liyor. Kaynak girdilerine `cargo:rerun-if-changed`, kullanılan derleme ortamı girdilerine `cargo:rerun-if-env-changed` kaydı ekleniyor. Böylece aynı compiler girdileriyle yeniden derlemeler kararlı fingerprint üretirken compiler implementation değişiklikleri yeni fingerprint üretir.

`src/compiler/cache.rs` anahtarına fingerprint dahil edildi; yalnızca mtime'a dayalı kullanılmayan `compiler_stamp()` ise işlevsel alternatifi eklenince kaldırıldı. Böylece önceki bölümdeki “tek başına silmek cache riskini gizler” uyarısı artık geçerli değil: risk yalnızca ölü fonksiyonu silerek değil, kaynak/build fingerprint'i anahtara bağlayarak kapatıldı. Yeni birim testi aynı proje girdilerinde aynı fingerprint'in aynı anahtarı, değişik fingerprint'in farklı anahtarı verdiğini doğruluyor.

### Cache doğrulama kanıtı

- `cargo test compiler_fingerprint_invalidates_cache_without_changing_project_inputs --lib`: **1 geçti**.
- Fingerprint'in compiler kaynak içeriği değişince değiştiğini geçici `src/runtime/core.js` yorumuyla ölçtüm: `cbedb663bef9e3c2` → `b165af50e6036dd0`; yorum kaldırılıp kaynak içeriği eski hâline getirildikten ve Cargo yeniden derledikten sonra ilk değer (`cbedb663bef9e3c2`) geri geldi. Geçici yorum bırakılmadı.
- Ayrı, geçici Vey projesinde aynı kaynakla ikinci `vey build` cache dosyasının timestamp'ini korudu; kaynak içeriği değiştirilince cache dosyası yeniden yazıldı ve derlenen Node çıktısı yeni metni verdi. Bu, aynı compiler çalışması altında hit/miss akışını doğrular.
- Son `cargo test --all-targets`: **383 geçti, 0 başarısız**; hedef dağılımı: lib 1, main 8, CLI 48, dev 9, DOM 15, lang 64, README 4, regressions 43, runtime 101, server 10, types 80.
- Cache kodu değişikliğinden sonra `dead_code` uyarısı kalmadı. `git diff --check` temiz.

Bu doğrulama correctness ve cache hit/miss davranışı içindir; saniye cinsinden performans/soğuk-sıcak build ölçümü yapılmadı. Bu oturumda commit veya push yapılmadı.


## Type-soundness devamı — 23 Ekim 2026, 20:01 (+03:00)

Öncelikli type-system taramasında dört ayrı, minimal örnekle doğrulanan problem bulundu ve regresyon testleriyle düzeltildi:

| Bulgu | Düzeltme | Regresyon kanıtı |
|---|---|---|
| **Imported recursive alias yanlış kabulü (P2, type-safety):** `t.Tree` içindeki recursion başka modülden geldiğinde checker ilgili alias şekline ulaşamıyor, recursive kenarın altındaki hatalı `value: "wrong"` alanını kabul ediyordu. | `ModuleTypeEnv` yerel alias şekillerini sadece internal checking metadatası olarak taşır; import sırasında recursion referansları module-qualified yapılır ve `Opening` bu shape'lerle açılır. Private alias adları `env.types` üzerinden public yazılabilir tipe dönüşmez. | `an_imported_recursive_alias_checks_fields_after_recursive_edges`: geçerli iki seviyeli Tree kabul, daha derindeki string değerli number alanı ret. |
| **Generic interface import false negative (P2):** `export interface Box<T>` başka modülde `t.Box<number>` olarak kullanılamıyordu; genel generic interface yerel kullanımda geçerliydi. | Export edilen generic interface'in type parameter'ları, çözümlenmiş body'si ve constraints'i import ortamındaki generic type metadata'sına eklenir; qualified resolver substitution ve constraint denetimini uygular. | `a_generic_interface_can_be_used_from_another_module`: number örneği kabul; alan tip uyuşmazlığı ve `T: number` ihlali ayrı ayrı ret. |
| **Generic callback inference false negative (P2):** `choose(1, fn(x) { return x + 1 })` ilk argüman `T = number` bilgisini unannotated callback'e aktarmıyor, `x` hâlâ `T` iken `+` reddediliyordu. | Argümanlar sırayla infer edilir; önceki argümanların eşleştirdiği parametreler sonraki argümanların contextual type'ına uygulanır. `substitute_bound` kullanılınca henüz çözümlenmemiş type variables `any`'ye dönüştürülmeden korunur. | `an_earlier_argument_contextualizes_a_later_generic_callback` geçti; ayrıca mevcut `a_type_parameter_may_have_a_constraint` testi de geçti (önceki denemede yanlışlıkla `substitute` kullanımı bound'ları bozmuştu). |
| **`finally` içindeki kesin return false negative (P2):** typed function, `finally` bloğundaki koşulsuz `return` nedeniyle her durumda değer döndürmesine rağmen “her path return etmeli” diye reddediliyordu. | `block_always_exits` ve `stmt_always_exits`, `finally` bloğunun kesin çıkışını dikkate alır; mevcut JS codegen davranışı değişmedi. | `a_return_in_finally_satisfies_the_function_return_type` geçti. |

### Bu devamın doğrulaması

- Her dört sorun için test eklemeden önce minimal kaynak CLI ile çalıştırıldı; sonra ilgili regresyon testi düzeltme öncesi başarısız, düzeltme sonrası başarılı oldu.
- `cargo test --all-targets`: **387 geçti, 0 başarısız**. Hedefler: lib 1, main 8, CLI 48, dev 9, DOM 15, lang 64, README 4, regressions 43, runtime 101, server 10, types 84.
- Önceki tüm hedefler ve yeni type testleri aynı son çalışma ağacında yeniden çalıştırıldı. `git diff --check` temiz.
- Commit veya push yapılmadı. Cache/build fingerprint için performans ölçümü yapılmadı; bu raporda yer alan test kanıtı doğruluk ve davranış kapsamındadır.


## Tooling ve LSP devamı — 23 Ekim 2026, 20:38 (+03:00)

### Doğrulanıp düzeltilen iki davranış

- **`vey dev` proje ayarını yeniden yüklemiyordu (P2, sayfa başlığı):** sayfa sunucusu `vey.toml` ayarını başlangıçta bir kez okuyordu; watcher kaynakları ve public dosyaları izlese de config dosyasını izlemiyor ve reload'da yakaladığı eski title'ı kullanıyordu. `dev_snapshot` şimdi `Config::find` ile aynı arama zincirindeki sekiz `vey.toml` adayını (henüz yok olan dosyalar dahil) izliyor; değişiklikte title config'ten tekrar okunuyor. Komut satırındaki `--title` override'ı önceliğini koruyor. `dev_reloads_the_title_when_project_config_changes` testi eski başlıktan yeni başlığa canlı güncellemeyi doğruluyor.
- **Geçersiz `vey new --template` boş hedef dizini bırakıyordu (P3, CLI yan etkisi):** scaffold kodu template adını doğrulamadan önce `create_dir_all` çalıştırıyordu. Template doğrulaması dizin yaratımından önceye alındı. `an_unknown_new_project_template_does_not_create_the_target` testi hata mesajını ve hedefin hiç oluşmadığını kontrol ediyor.

### Önceden açık LSP bulgusu — imported member cross-file references/rename (düzeltildi)

İlk bulgu **P2** olarak doğrulanmıştı: imported modülün exported üye adındaki references/rename, diğer açık dosyadaki kullanımları bulmuyordu. Her iki dosya da `didOpen` ile LSP'ye gönderilmişti.

Minimal kaynaklar:

```vey
// math.vey
export fn area() { return 1 }
```

```vey
// main.vey
use "./math.vey" as m
print(m.area())
```

Düzeltme öncesi `textDocument/references`, `math.vey` içindeki `area` konumunda (line 0, character 13; `includeDeclaration: true`) yalnızca tanımı döndürüyor; `main.vey` içindeki `m.area` kullanımı eksikti. `textDocument/rename` (`newName: "measure"`) da yalnızca `math.vey` için edit üretiyordu.

**Düzeltme:** `Names` imported aliases, lexical receiver binding identity, exported top-level names ve static module-member uses topluyor. Bu kayıtlar artık `m.area` ve plain/escaped string-literal `m["area"]` / `m["ar\x65a"]` expression'larının yanında README'de belgelenen `m.Card` markup tag'lerini, `m.Box<T>` generic type'larını ve `m.Color` qualified enum/type adlarını da kapsıyor. Type walker annotations, declarations, signatures, constraints, casts/type tests ve explicit generic type arguments içinden qualified names topluyor. LSP import yolunu canonical URI'ye çözüyor; sembolü URI, ad ve declaration konumuyla tanımlayıp aynı adlı başka modül veya shadowed alias'ın yanlış eşleşmesini önlüyor. `definition`, `references` ve per-file WorkspaceEdit rename bu ortak çözümlemeyi kullanıyor.

`the_language_server_resolves_cross_file_members_and_local_type_symbols` dört açık dosyada aynı adları export eden iki modül ile shadowed `m` decoy'larını kullanıyor. Function member `area`, markup component `Card`, generic alias `Box<T>`, enum type `Color` ve variant `Color.Red` için definition/references/rename sınanıyor. Test ayrıca `Shape.Red` adlı aynı isimli, farklı enum variant'ının yanlışlıkla `Color.Red` sonuçlarına karışmadığını; yerel type-name kullanımlarını, match pattern'larını ve astral Unicode öncesindeki UTF-16 konumlarını doğruluyor. Yalnızca hedef modül/gerçek kullanımlar edit'leniyor. Mevcut single-file LSP testi shadowed local binding regresyonunu koruyor.

**Kapsam notu:** Yerel alias ve type-parameter adları için type namespace index'i ile final enum variant segmenti desteği eklendi. Statik string key'ler (escape dizileri dahil, ör. `m["ar\x65a"]`) cross-file navigation'a bağlanır; interpolation içeren string ifadeleri ve dynamic key'ler (`m[key]`) bağlanmaz. Açık olmayan belgeler için genel workspace index'i de kapsam dışıdır. Parser'ın type grammar'ı tek module qualifier taşır; çok segmentli qualified type yazımı ayrıca tasarlanmalıdır.

### Güncel doğrulama

- `cargo test --test cli an_unknown_new_project_template_does_not_create_the_target -- --exact`: **1 geçti**.
- `cargo test --test dev dev_reloads_the_title_when_project_config_changes -- --exact`: **1 geçti**.
- `cargo test --test cli the_language_server_resolves_cross_file_members_and_local_type_symbols -- --exact`: **1 geçti**; ayrıca var olan single-file LSP shadowing test **1 geçti**.
- Son `cargo test --all-targets`: **396 geçti, 0 başarısız**. Hedefler: lib 1, main 8, CLI 50, dev 10, DOM 15, lang 64, README 4, regressions 43, runtime 101, server 10, types 90.
- `git diff --check`: temiz. İlk audit commit'i `0e46312` ve sonraki follow-up'lar `565b3cf`, `cf15808`, `a3ca135`, `93d8559` push edildi; checked-module dynamic-key düzeltmesi, README ve bu rapor güncellemesi mevcut continuation commit/push'unda yer alıyor.

Bu audit performans ölçümü içermez. Canlı config yenileme kapsamı sayfa title'ıdır; dinleyen soketin host/port ayarını çalışan süreçte yeniden bind etme davranışı bu değişiklikle kapsama alınmadı.

### Son LSP devamı — enum variant ve type-symbol kapsamı

Enum variant tanımları ve kullanımları artık ebeveyn enum sembolüyle ilişkilidir. Böylece `m.Color.Red` içindeki `Red` için definition/references/rename, ilgili modüldeki enum variant tanımına gider; match pattern'ları ve yerel `Color.Red` ifadeleri de aynı çözümlemeyi kullanır. Eş adlı `Shape.Red` farklı enum sembolünde tutulur. Imported member occurrence'ları ayrıca hedef modülün export kümesine göre süzülür; export edilmemiş adlara ilişkin hatalı qualified member kullanımları geçerli referans sayılmaz.

Yerel type alias'lar ve generic type parameter'lar için value namespace'inden ayrı symbol map eklendi. Type-name occurrences scoped binding identity ile tutulduğu için aynı yazımlı değer adıyla karışmaz; references/rename type bağlamlarında doğru sembole uygulanır. Regresyon testi enum type/variant tanımları ve kullanımları, pattern'lar, type parameter scope'ları ve ayrı enum'larda tekrarlanan variant adlarını kapsar.

- `cargo test --test cli the_language_server_resolves_cross_file_members_and_local_type_symbols -- --exact`: **1 geçti**.
- Son kaynak/test durumu üzerinde `cargo test --all-targets`: **396 geçti, 0 başarısız**; CLI 50, type 90, runtime 101 ve diğer hedefler de geçti.
- `git diff --check`: geçti. `m["area"]` ve escaped statik key'ler için navigation/type-checking eklendi; checked Vey module alias'ında `m[key]` type-check aşamasında reddedilir.

### Literal computed module key bulgusu ve düzeltmesi

Hedefli bir probe, `m["area"]("not a number")` çağrısının ve `m["noSuchExport"]` erişiminin `vey check` tarafından kabul edildiğini gösterdi. AST, computed key'i `MemberProp::Computed` olarak temsil ediyordu; import alias'ı normal expression değeri olarak `any`'e düştüğü için modülün export imza/varlık doğrulaması atlanıyordu. Bu doğrulanmış bir type-safety kaçamağıydı.

Plain ve escaped statik string key, shadowed olmayan imported module alias üzerinde direct named-member checker'a yönlendirildi. Böylece export varlığı, private üyeler, fonksiyon argümanları ve dönüş tipi nokta erişimiyle aynı kuralları izliyor. Checked Vey module namespace'inde `m[key]` gibi statik olmayan computed erişimler artık açıklayıcı hata verir; unchecked npm package import'ları README'deki `any` sınırında kalır. LSP, literal'in çözülen adıyla birlikte ham kaynak aralığını izleyerek `m["area"]` ve `m["ar\x65a"]` için definition/references/rename sağlıyor; rename string içeriğinin tamamını değiştirip quote'ları koruyor.

- `cargo test --test types literal_computed -- --nocapture`: **2 geçti** (valid call, yanlış argüman ve olmayan export kontrolleri).
- `cargo test --test cli the_language_server_resolves_cross_file_members_and_local_type_symbols -- --exact`: **1 geçti**; computed `m["area"]` için definition, references ve rename doğrulandı.

### Takip denetimi — import shadow ömrü ve escaped literal navigation

Bağımsız checker incelemesi, import alias'ını gölgeleyen herhangi bir yerel binding'in adını checker-wide `HashSet` içine yazdığını ve scope kapanırken silmediğini gösterdi. Örneğin bir `if` bloğunda `let lib = ...` tanımladıktan sonra `lib.add("wrong")` eski davranışta module export imzasına ulaşmayıp `any` üzerinden geçebiliyordu. Shadow set'i artık value scope'larıyla paralel bir stack: blok içindeki yerel dot/computed erişimler yerel değeri kullanıyor, `pop_scope` sonrasında import namespace'i ve fonksiyon imzası yeniden etkinleşiyor.

Bağımsız LSP incelemesi, AST'nin escape dizilerini çözdüğü (`"ar\x65a"` → `area`) halde navigation yardımcısının ham kaynakla çözülen metni eşitlik karşılaştırması yüzünden bu kullanımı atladığını buldu. Yardımcı artık tek parça statik string'in iç kaynak aralığını escape-aware tarıyor; ham karakter genişliği references ve rename'e taşınıyor. UTF-16 range hesaplaması kaynağın ham genişliğini kullanıyor; rename kaçışlı içeriğin tamamını değiştirip tırnakları koruyor.

- `cargo test --test types a_local_import_shadow_only_hides_the_module_inside_its_scope -- --exact`: **1 geçti**; nested scope içindeki local dot/computed access ve scope sonrasındaki direct/computed import çağrılarının imza kontrolü kapsandı.
- `cargo test --test cli the_language_server_resolves_cross_file_members_and_local_type_symbols -- --exact`: **1 geçti**; `m["ar\x65a"]` için definition, references ve ham range genişliğiyle rename doğrulandı.
- Ek Node bundle smoke testinde `m["area"](41)` derlenip `41` yazdı.
- Güncel `cargo test --all-targets`: **396 geçti, 0 başarısız**; `git diff --check` temiz.

### Checked module dynamic-key kaçışının kapatılması

Hedefli probe, `let key: string = "add"; lib[key]("wrong")` ifadesinin checked Vey module export imzasını atlayıp `any` üzerinden kabul edildiğini gösterdi. Checked module alias'ı runtime dictionary değil, derleme zamanında bilinen export namespace'i olduğundan, computed access string literal değilse checker artık `module 'lib' requires a static string key for computed access` hatası veriyor. Literal key direct export checker'ına yönlenmeye devam eder. Unchecked npm imports, README'de belirtildiği gibi bilinmeyen üyeler için `any` davranışını korur. README Modules bölümü statik-key sözleşmesini ve dinamik key gereken durumda export edilen dictionary kullanmayı açıklar.

- `cargo test --test types a_dynamic_computed_module_access_is_rejected -- --exact`: **1 geçti**.
- `cargo test --test types a_literal_computed_call_to_another_module_is_checked -- --exact`: **1 geçti**; statik computed call hâlâ imza denetimine tabi.
- Güncel `cargo test --all-targets`: **396 geçti, 0 başarısız**; README 4, types 90 ve diğer tüm hedefler geçti.

### Fixed-shape, tuple ve unknown computed-index kaçışlarının kapatılması

Probe, bilinen `{ x: number }` shape'inin dinamik string anahtarla okunmasının `any` ürettiğini; tuple'ın dinamik numeric index ve sabit boyut dışındaki literal index'inin de `any`'e düştüğünü gösterdi. `unknown` parametre üstünde computed access de narrowing olmadan aynı fallback'a ulaşıyordu. Artık object/named/interface şekillerinde literal string key dışında computed access reddediliyor; dictionary türü kendi key/value sözleşmesiyle dinamik erişimi koruyor. Tuple erişimi literal, aralık içi numeric index ister; `unknown` ise önce uygun bir tipe daraltılmalıdır. README Type bölümüne bu kurallar eklendi.

- `cargo test --test types a_dynamic_key_on_a_typed_object_needs_a_dictionary_type -- --exact`: **1 geçti**; inline object, named alias ve interface dynamic key'leri reddedildi, dictionary dynamic key'i typed kaldı.
- `cargo test --test types a_tuple_needs_a_literal_in_range_index -- --exact`: **1 geçti**; dinamik ve aralık dışı tuple index'leri reddedildi.
- `an_unknown_value_needs_object_narrowing_before_use_as_object`: computed unknown access da narrowing hatası verdi.
- `cargo test --test types`: **92 geçti**; `cargo test --test readme`: **4 geçti**.
- Son `cargo test --all-targets`: **398 geçti, 0 başarısız**; `git diff --check` temiz.

### List/string/dictionary okuma yok-değer güvenliği

Probe, boş liste `values[0]` ve boş dictionary `values["missing"]` okumalarının `number` olarak kabul edildiğini gösterdi; runtime'da ikisi de `undefined` döndürebilir. `Type::Array`, `Type::Dict` ve string numeric-index okuma türleri artık sırasıyla `T | undefined`, `V | undefined` ve `string | undefined`; dictionary dot-key (`dict.name`) de aynı yok-değer davranışını izliyor. Böylece tüketen kod `??`, narrowing veya union return tipi kullanmalı. Ayrıca computed erişimde desteklenmeyen non-`any` türler artık genel `any` fallback'ına düşmez; primitive veya nullable union önce hata verir ve uygun narrowing gerekir. Optional computed access `null | undefined` union kollarını atlayıp sadece gerçek lookup türünü döndürür.

Okuma ve yazma yolları ayrıldı: `=` hedefi list/dictionary eleman/değer tipini kullanır ve yeni değer yazabilir; compound assignment önce mevcut değeri okuduğu için `undefined` olasılığını korur ve güvenli şekilde reddedilebilir. `let-else` else bloğu şimdi isim bağlanmadan önce denetlenir, mutlaka çıkış yapmalıdır ve başarı dalındaki isimden `null | undefined` çıkarılır. Bu, README'deki ilk list elemanı örneğinin generic dönüş tipinde doğru şekilde daralmasını da sağlar; generic `first` helper'ı `items[0] ?? null` ile yok durumu açıkça ele alır.

- `collection_reads_include_undefined_but_writes_keep_the_value_type`: boş liste, dictionary bracket/dot ve string index okumalarında `undefined` hatası; `??` sonrası güvenli kullanım; basit yazma; compound-write reddi ve let-else narrowing doğrulandı.
- `a_union_with_a_non_indexable_member_must_be_narrowed_before_access`: primitive index reddi, nullable list union'ında narrowing zorunluluğu ve null/undefined için optional computed access doğrulandı.
- Aynı regression'da let-else'in çıkış yapmayan fallback'i reddediliyor; else block içindeyken yeni binding görünür değil.
- `cargo test --test types`: **92 geçti**; `cargo test --test readme`: **4 geçti**; `supports_generics`: **1 geçti**.
- Son `cargo test --all-targets`: **398 geçti, 0 başarısız**; `git diff --check` temiz.


### Optional zincirleme, nullable erişim, `unknown` alanlar ve recursive generic alan tipleri

Takip incelemesinde optional member/call sonuç türlerinin JavaScript runtime'ıyla hizalı olmadığı görüldü: üretilen kod nullish base için `undefined` döndürürken checker `null` ekleyebiliyor ve yalnızca null/undefined olan base'lerde geçerli tipi üretemeyebiliyordu. `?.` artık nullish kolları atlayıp `undefined` sonucunu ekliyor; optional call'ın base'i statik olarak nullish olsa da argüman ifadeleri denetlenmeye devam ediyor. Buna karşılık normal member access ve çağrı, nullable union kollarını sessizce atlamıyor; kullanımdan önce narrowing gerekiyor.

Ayrıca `unknown.field` ve desteklenmeyen ordinary member türleri genel fallback nedeniyle `any`'e düşebiliyordu. Bilinmeyen nesne alanlarına erişim artık object narrowing gerektiriyor; `unknown` erişiminden sonraki tuple `length` gibi destekli property'ler kendi tanımlı türünü koruyor. `Type::Recursion` alanları generic type argümanlarını taşıyacak şekilde genişletildi; local ve imported recursive generic alias'larda (`Node<T>`, `module.GenericTree<T>`) nested alan lookup'ı uygulanmış tip argümanlarıyla çözülüyor. Type substitution, bound substitution, type-variable erasure ve occurrence kontrolleri de recursion argümanlarını koruyor. Böylece recursive alandaki `number`, `any` gibi davranmıyor ve yanlış dönüş tipi reddediliyor.

- `cargo test --test types`: **95 geçti, 0 başarısız**. Yeni regressions optional member/call için `undefined`, statik nullish optional call argümanlarının denetlenmesi, ordinary nullable member/call için narrowing, `unknown` ordinary member reddi ve local/imported generic recursive alias alan tiplerinin korunmasını kapsıyor.
- Node-backed lang regression, nullish optional named/computed/call zincirlerinin runtime'da `undefined` döndürdüğünü doğruluyor; `accepts_gradual_types` fixture'ında callback ile değişebilen değere optional access kullanılıyor.
- Recursive alias runtime fixture'ı olası null `next` alanını `?.` ile okuyor; negative type regression `number` alanının `string` dönüşüne `any` üzerinden kaçmasını yakalıyor.
- Son tam doğrulama: `cargo test --all-targets` **402 geçti, 0 başarısız**; `git diff --check` temiz.


### Generic recursive alias assignability — ek doğrulanmış hata ve düzeltme

Önceki bölümde recursive generic placeholder argümanlarının member lookup ve type utility'lerinde korunması doğrulanmıştı; ek atama probe'u başka bir kenar durumu gösterdi: iç içe değer olarak verilen geçerli bir `GenericTree<number>` için recursive placeholder açılırken alias'ın genel `T` şekli gerçek `number` argümanıyla uzmanlaştırılmıyordu. Böylece geçerli imported generic recursive değer reddediliyordu. Aynı mekanizma yerel recursive alias şekillerinin atanabilirlik kontrolünde de tutarlı biçimde açılmamasına yol açabiliyordu.

`Opening` artık alias adına göre generic parametre adlarını saklıyor; `Type::Recursion(name, args)` açılırken alias şekline gerçek argümanları uyguluyor. Checker, yerel alias şekillerini ve generic parametre sırasını hazırlıyor, imported şekillerde de `ModuleTypeEnv.generic_aliases` parametrelerini taşıyor. Regresyonlar geçerli ve geçersiz nested imported `GenericTree<number>` literal'larını, ayrıca yerel `Tree<number>` recursive collection literal'larını denetliyor.

- Önce genişletilmiş `an_imported_recursive_alias_checks_fields_after_recursive_edges` testinde geçerli nested generic değer düzeltme öncesi reddedildi; uzmanlaştırılmış recursion shape sonrasında test geçti ve yanlış türde nested `value` reddedildi.
- `local_recursive_generic_aliases_check_nested_values`: **1 geçti**; yerel generic recursive collection için geçerli nested değer kabul, string `value` reddedildi.
- Son `cargo test --all-targets`: **403 geçti, 0 başarısız**; `git diff --check` temiz.


### Recursive generic şekillerin modül üzerinden yeniden export edilmesi

İki modüllü probe'da `types.vey` recursive `Tree<T>` ve `Flip<A,B>` alias'larını, `wrapper.vey` ise bunları `Wrapped<T>` / `Flipped<A,B>` içinde yeniden export ediyordu. Geçerli değer kabul edilirken, `Tree<number>` içindeki `value: "wrong"` ve parametreleri yer değiştiren `Flip<B,A>` içindeki yanlış değerler de kabul edildi. Bu, modül sınırından geçen recursive placeholder şekillerinin ve parametre metadata'sının transitif taşınmadığını; ayrıca generic alias içindeki `T -> T` özdeş-substitution'ın cycle guard tarafından `any`'e indirgendiğini gösterdi.

`ModuleTypeEnv` artık internal recursive alias şekillerinin generic parametre adlarını da taşır; imported şekiller ve metadata yeniden export zincirinde korunur, ancak bu internal shape kayıtları yazılabilir type adları hâline getirilmez. `qualified_type` recursion kimliğini her yeni module boundary'de namespace'ler. Ek probe'da wrapper ile consumer aynı import alias'ı (`t`) kullandığında idempotent-prefix kontrolü eski `t.Tree` kimliğini yanlışlıkla koruyordu; recursion kimliği artık her sınırda yeni namespace katmanını alıyor. `substitute_with`, aynı adlı type variable'a özdeş eşlemeyi `any` yerine değişmeden korur. Güçlendirilmiş regresyon, iki modüllü re-export zincirinde aynı alias yazımını tekrar kullanır; geçerli nested `Tree<number>` ve argüman sırası tersine dönen `Flip<number,string>` değerlerini kabul ederken her iki recursive dalda yanlış nested type'ı reddeder.

- Düzeltme öncesi `recursive_generic_shapes_survive_reexport_layers` probe'u hatalı nested değerleri kabul etti; aynı module alias'ını wrapper ve consumer'da tekrar kullanma varyantı da hatayı yeniden üretti. Düzeltme sonrası güçlendirilmiş regression **1/1 geçti**.
- Son `cargo test --all-targets`: **404 geçti, 0 başarısız**; `git diff --check` temiz.


### Recursive placeholder argümanlarında generic function unification

`type Node<T> = { children: [Node<T>] }` örneğinde `T` yalnızca `Type::Recursion` argümanında bulunuyor. `type_has_var` bu argümanları taramadığı için `identity<T>(Node<T>) -> Node<T>` generic değilmiş gibi ele alınıyor; ayrıca `unify_types` recursive placeholder'ları koşulsuz wildcard sayıp argümanlarını karşılaştırmıyordu. Aynı yapıya sahip farklı `Mirror<T>` alias'ı üzerinden yapılan probe `(Mirror<number>) -> Mirror<string>` imzasının da kabul edildiğini gösterdi. Sonraki `Pair<A,B>` probe'u ise argüman sayısı farklı placeholder'ların wildcard fallback'ine düşerek uyumsuz dönüş tipini hâlâ kabul edebildiğini ortaya çıkardı.

`type_has_var` ve `has_type_var` artık recursive placeholder argümanlarını inceliyor. `unify_types` iki recursive placeholder'ın alias adından bağımsız olarak argüman sayılarının eşit olmasını zorunlu kılıyor; eşit uzunluktaki argümanları pairwise unifie ediyor, arity mismatch'i wildcard'a bırakmadan reddediyor. Böylece `T` input'tan `number` olarak bağlanıyor ve output'taki `string` ile uyuşmazlık reddediliyor. Pozitif atamalar hem aynı hem farklı alias adı üzerinden doğrulanıyor; aynı-alias, farklı-alias ve arity-mismatch negatifleri reddediliyor; var-presence yardımcıları doğrudan unit regression ile kapsanıyor.

- `generic_function_assignability_sees_recursive_placeholder_variables`: **1 geçti**; uyumlu aynı/farklı-alias specialization kabul, uyumsuz dönüş tipi atamaları ret.
- `recursive_placeholder_arguments_count_as_type_variables`: helper-level regresyon.
- Son tam doğrulama: `cargo test --all-targets` **406 geçti, 0 başarısız**; `git diff --check` temiz.

### Editörde açık olmayan import hedeflerinde LSP gezinmesi

Diskte mevcut bir `math.vey` dosyasını kullanan `main.vey` açıkken, `math.vey` henüz editörde açılmamışsa `textDocument/definition` `null` dönüyordu. `imported_module_uri` yalnızca açık belge URI'larında hedef arıyor, ayrıca references/rename aralıkları için kaynak metni yalnızca `self.files` içinden alıyordu; dolayısıyla kapalı hedef dosyanın declaration konumu yanıtlara eklenmiyordu.

LSP, açık belge tamponunu öncelikli tutarak kapalı dosya metnini diskteki URI yolundan okuyabiliyor. Import yolu canonical hedef dosyaya çözülüp URI eşleştirmesi yapılabiliyor; isim çıkarımı, references konumları ve rename edit aralıkları aynı kaynak-fallback'ini kullanıyor. `initialize` içindeki `workspaceFolders` veya `rootUri` (eski `rootPath` yedeğiyle) workspace köklerini sağlar; references/rename artık bu köklerdeki `.vey` kaynaklarını tarar, açık tamponları canonical path üzerinden tercih eder ve generated/dependency klasörlerini atlar. Entegrasyon testi yalnızca `main.vey` dosyasını açıp kapalı `math.vey` hedefi ile kapalı ikinci tüketici `other.vey` üzerinde definition, workspace references ve rename yanıtlarını doğruluyor.

Ek same-name probe'unda imported `area` export'undan önceki local helper içindeki `area` binding'i, name-only `defs.find` nedeniyle imported definition hedefi olabiliyordu; cross-file rename eşlemesi de sadece export adıyla çalışırsa local binding'e dış import kullanımları karıştırabilirdi. `Names` artık exported binding kimliğini value/type namespace'iyle birlikte saklıyor; imported member definition, references/rename ve enum variant çözümlemesi doğru export kimliğini kullanıyor. Aynı regression, `area` local değişkeninin kendi iki occurrence'ında kalmasını; imported value rename'in yalnız function export'ını ve iki import kullanımını, type rename'in ise yalnız type export'ını ve type kullanımını değiştirmesini doğruluyor.

LSP `textDocument/references` ayrıca `ReferenceContext.includeDeclaration` değerini yok sayıp declaration'ı her zaman ekliyordu. Artık alan açıkça `false` ise tam hedef declaration konumu çıkarılıyor; `true` veya geriye dönük uyumluluk için alanın yokluğu declaration'ı dahil ediyor. Regresyon `true` ile declaration dahil üç konumu, `false` ile yalnız iki kullanım konumunu doğruluyor; kapsamlı eski test de alan belirtilmeyen istekteki varsayılanı koruyor.

`textDocument/rename` yeni adı doğrulamadan her occurrence'a yazıyordu; reserved keyword veya birden fazla token böylece kaynak kodu parse edilemez hâle getirebiliyordu. Sunucu artık Vey lexer'ı ile yeni adın tam olarak tek bir `Ident` token'ı olduğunu doğruluyor ve keyword/boşluklu değerlerde edit üretmiyor. LSP regresyonu `return` ve `bad name` değerlerinin `null` sonuçla reddedildiğini kontrol ediyor.

Workspace kökü değişiklikleri için sunucu capability'leri ilan ediyor ve `workspace/didChangeWorkspaceFolders` bildiriminde çıkarılan kökleri siliyor, eklenen kökleri canonicalize ederek ekliyor. Aynı entegrasyon regresyonu root değiştikten sonra eski kapalı tüketicinin references/rename sonuçlarından çıktığını, yeni kökteki kapalı tüketicinin girdiğini doğruluyor. Tarama maliyetini azaltmak için workspace-wide dosya keşfi yalnızca export edilmiş sembol veya export enum variant'ı için çalışıyor; yerel sembol araması sadece hedef dosyayı tarıyor. 1.000 küçük `.vey` dosyalı sentetik workspace'te bir seferlik duvar saati kontrolü yerel referans için **0,004 sn**, export sembol için **0,049 sn** ölçtü; bu mikro-benchmark donanım ve dosya boyutuna bağlı yaklaşık kontrol niteliğindedir.

- `the_language_server_navigates_to_unopened_imported_modules`: **1 geçti**; target ve ikinci tüketici kapalıyken definition doğru export/range'i verdi, references `includeDeclaration` true/false değerlerine uydu, rename üç dosyada edit üretti; local `area` referansları ile aynı adlı exported type/function rename kapsamları ve workspace kökü değişikliği doğru işlendi.
- Son tam doğrulama: `cargo test --all-targets` **407 geçti, 0 başarısız**; `git diff --check` temiz.


## Takip denetimi — 24 Ekim 2026, 00:30 (+03:00)

Bu devam, `main` üzerindeki `3e74404` (`Validate LSP rename identifiers`) tabanında yapıldı. Önceki salt-okunur denetim ajanlarının raporları yeniden incelendi; bulgular güncel derleyici/runtime üzerinde tekrar üretildi ve aşağıdaki üç hata kapatıldı.

### LSP tanı aralıklarında UTF-16 sütunları

`problem()` ve `warning_problem()` compiler sütununu doğrudan LSP `character` alanına yazıyordu. Vey sütunları Unicode scalar/karakter sayarken LSP UTF-16 code unit saydığından, tanıdan önceki her astral karakter (ör. emoji) aralığı bir code unit kısa gösteriyordu. Artık tanı aralığı ilgili kaynak satırından hesaplanıyor ve her iki uç da UTF-16 birimine çevriliyor. Kaynak seçimi açık editör tamponunu öncelikli kullanıyor; başka modüldeki hata/uyarı için açık dosya tamponu veya disk kaynağı kullanılıyor.

`diagnostic_ranges_use_utf16_code_units` regresyonu `a😀b` satırında hem hata hem uyarı aralığının başlangıcını 1, bitişini 3 olarak doğruluyor; emoji iki UTF-16 birim kaplıyor. Test, yardımcı sütun dönüşümünü ve iki diagnostic kurucusunu doğrudan kapsıyor.

### Runtime UTF-8 decoder doğrulaması

`src/runtime/shared.js::utf8Decode`, devam baytlarının varlığını ve geçerliliğini varsayıyor, eksik baytları sıfır gibi okuyordu; geçersiz lead-byte aralıkları, overlong kodlamalar, surrogate kod noktaları ve Unicode üst sınırı dışındaki diziler de doğrulanmıyordu. Decoder artık 2/3/4 bayt dizilerinin uzunluğunu, devam baytlarını ve scalar sınırlarını doğruluyor; kesik/geçersiz alt diziler için U+FFFD üretiyor ve geçerli sonraki baytları yeniden işlemeye bırakarak kurtarıyor.

`utf8_decoder_replaces_malformed_sequences_and_recovers` regresyonu kesik 3/4 bayt dizilerini, geçersiz continuation'ı, overlong, surrogate ve U+10FFFF üstü dizileri, bozuk alt dizi sonrasındaki ASCII metni ve geçerli emoji round-trip'ini doğruluyor.

### Spread çağrılarında tip denetimi

Bağımsız type-soundness ajanı, `check_call_named` içindeki spread erken dönüşünün yalnız spread operandını değil, aynı çağrının bütün argümanlarını kontrol dışı bıraktığını doğruladı. Örneğin `fn take(value: number) -> number` için `let values: [string] = ["wrong"]; take(...values)` önce başarılı sayılıyordu. Artık spread operandının iterable türü ve eleman türü belirleniyor; sabit uzunluklu tuple spread'leri bağımsız argümanlara açılıyor. Dinamik uzunluklu spread tek ve son argüman olarak kabul ediliyor; checked fonksiyonda spread öncesinde gerekli parametreler sağlanmalı ve olası kalan parametrelerin tümü eleman türüyle uyumlu olmalı. Böylece aynı çağrıdaki sıradan argümanlar da standart parametre denetiminden geçiyor. `any` hâlâ dilin açık dinamik kaçış kapısıdır; doğrudan bilinmeyen/union callee üzerindeki dinamik spread konservatif biçimde reddedilir.

`checked_spread_calls_validate_elements_and_required_arguments` negatif regresyonları opsiyonel number parametresine string spread'i, required parametreyi dinamik spread'e bırakmayı ve yanlış sıradan argümanı olan karışık çağrıyı reddediyor; pozitif rest-array spread'i kabul ediyor.

### Önceki ajan bulgularının güncel HEAD'de triage edilmesi

- `job_V06sGAYe` spread bypass'ını bağımsız olarak aynı minimal örnekle doğrulamıştı; bu bulgu yukarıdaki değişiklik ve regresyonlarla kapatıldı.
- Eski type-checker raporundaki scoped import shadowing örneği (`job_GicsDoA9`) güncel `main` üzerinde yeniden denendi; dış kapsamdaki imported `lib.add("wrong")` çağrısı artık `argument 1 expects number, found string` ile reddediliyor.
- Generic callback contextual-inference (`job_PjkcI8WB`) ve `finally` dönüş akışı (`job_Y30DvCSU`) örnekleri de güncel compiler ile tekrarlandı; ikisi de `vey check` tarafından kabul edildi. Bu raporlar eski HEAD'e ait açık bulgular değil.
- LSP UTF-16 raporu (`job_k6AH63Oz`) bu devamın ilk maddesindeki tanı aralığı hatasını doğrulamıştı; runtime ajanı (`job_yrJXvjKf`) decoder bulgusunu bağımsız biçimde doğrulamıştı.

### Doğrulama kanıtı

- `cargo test --all-targets --quiet`: **410 geçti, 0 başarısız**. Hedef grupları: library 2, binary 9, CLI 51, dev 10, DOM 15, lang 65, README 4, regressions 43, runtime 102, server 10, types 99.
- Hedefli regresyonlar: `diagnostic_ranges_use_utf16_code_units` **1/1**; `checked_spread_calls_validate_elements_and_required_arguments` **1/1**; `utf8_decoder_replaces_malformed_sequences_and_recovers` **1/1**.
- `git diff --check`: geçti.
- Çalışma değişiklikleri uygulama, regresyon testleri ve bu rapor güncellemesiyle birlikte toplam altı dosyayla sınırlı.


## Takip denetimi — 24 Ekim 2026, 10:43 (+03:00)

Önceki tam doğrulanmış `main` commit'i `e01f6b7` üzerinden tip sistemi, JavaScript runtime ve LSP protokolü için bağımsız salt-okunur denetimler çalıştırıldı. Üç yeni, güncel kaynakta yeniden üretilen bulgu kapatıldı.

### Destructuring ile eksik değerlerin `any`/non-nullable tipe sızması

Array binding pattern'ı `let [head, second]: [number] = [1]` biçiminde ikinci binding'i doğrudan `number` sayıyordu; object pattern da annotation'da olmayan alanı `any` sayıyordu. JavaScript gerçekte her ikisinde `undefined` verir. Denetim örnekleri `bad() -> number` fonksiyonundan `second` veya `{ missing }` binding'ini döndürdü; checker ikisini de kabul etti ve oluşan Node programı `undefined` bastı. Böylece sabit dönüş tipi yanıltıcı olabiliyordu.

Destructuring eleman/alan projeksiyonları artık yokluk ihtimalini `undefined` olarak taşır; optional object alanları ve dictionary property destructuring de bu union'ı içerir. `unknown` değer destructuring sırasında `any`'e çevrilmez. Tuple pattern'ında olmayan indeks `undefined` alır. Destructuring default'u, kaynak tipindeki `undefined` olasılığını binding tipinden çıkarır. Var olan `for [left, right] in [[1, 2], [3, 4]]` gibi biçimlerde literal satırların bilinen tuple şekli ayrıca korunur; bu, düzeltmenin gerçek runtime riski olmayan sabit literal öğelerini gereksiz yere `number | undefined` genişletmesini önler. Heterojen/eksik literal satırlar indeks bazında union ve gerekirse `undefined` üretir.

`destructuring_missing_values_keep_undefined_in_their_types` eksik array öğesini ve olmayan object alanını `number` dönüşüne sokmayı reddediyor; varsayılan verilen destructuring'in `undefined`'ı kaldırmasını kabul ediyor. Eski `destructures_objects_and_arrays` çalışma zamanı testi de literal-shape korumasıyla geçti.

### Asenkron `leave` geçişlerinde yanlış kardeş düğümün kaldırılması

`src/runtime/mount.js` removal loop'u `var leaving` değişkenini promise resolve/reject closure'larında yakalıyordu. Birden çok node aynı güncellemede ayrılır ve `leave` promise'leri beklerse, döngü değişkeni sonraki node'a ilerliyordu; ilk promise yanlış kardeşi erken kaldırıp gerçek düğümü sayfada bırakabiliyordu. İki node'un ayrı çözümlenmemiş promise'leriyle yeniden üretimde önce `a,b`, `a` promise'i çözüldükten sonra yanlış biçimde yalnız `a`, iki promise çözüldükten sonra yine `a` kaldı.

Her callback artık ilgili node ve parent'ı closure'a ayrı parametrelerle yakalıyor. `async_leave_transitions_remove_their_own_nodes` regresyonu `a` geçişi çözülünce `b`'nin hâlâ sayfada olduğunu; `b` geçişi de çözülünce hiçbir düğüm kalmadığını doğruluyor.

### LSP isteklerinde geçersiz parametreye cevap verilmemesi

Hover/completion/definition/references/documentSymbol dallarında eksik ya da yanlış biçimli parametre için kullanılan `?`, `handle()` metodunu yanıtsız `None` ile bitiriyordu; rename ayrıca `newName` yoksa açıkça `None` dönüyordu. JSON-RPC `id` taşıyan bu isteklerde istemci yanıt beklemeye devam eder. Sunucu artık bu hatalarda korelasyon ID'sini koruyarak `-32602 Invalid params` error yanıtı veriyor; ID olmayan notification'lara yanıt vermeme davranışı değişmedi.

`the_language_server_answers_requests_with_invalid_params` gerçek Content-Length JSON-RPC akışında eksik parametreli hover ve documentSymbol ile, geçerli belge/konum verip `newName`'i atlayan rename isteğini gönderiyor; üç isteğin her biri için ID'si doğru `-32602` yanıtını doğruluyor.

### Doğrulama

- `cargo test --all-targets --quiet`: **413 geçti, 0 başarısız**. Hedefler: library 2, binary 9, CLI 52, dev 10, DOM 15, lang 65, README 4, regressions 43, runtime 103, server 10, types 100.
- Yeni hedefli regresyonlar: LSP invalid params **1/1**, typed destructuring **1/1**, async leave **1/1**; mevcut nested-array destructuring testi **1/1**.
- `git diff --check`: geçti. Bu takipte henüz commit/push yapılmadı.
