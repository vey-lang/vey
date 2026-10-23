# Vey kod denetimi — güncel özet

**Tarih:** 23 Ekim 2026
**İncelenen upstream başlangıç revizyonu:** `1626f550865a9694201e1f6d15cb106f49e7c14e` (`origin/main`)
**Çalışma dalı:** `main`
**Kapsam:** Rust tip denetleyicisi, JavaScript runtime/server, CLI/LSP, regresyon testleri ve mevcut dokümantasyon. Bu rapor denetim bulgularını özetler; tüm eksiklerin kapandığı anlamına gelmez.

## Bu değişiklikte doğrulanıp düzeltilenler

1. **Serbest `TypeVar` herhangi bir tipe atanabiliyordu.** `(Type::Var(_), _) | (_, Type::Var(_)) => true` kuralı kaldırıldı. Checker, etkin generic scope'larda deklarasyonla gelen type bound'ları genişletip assignability ve sayısal/string operatör kontrollerinde kullanıyor. Böylece `T` artık kendiliğinden `number` veya başka somut tip sayılmıyor; `T: number` gibi sınırlar kullanılmaya devam ediyor.
2. **`unknown`, daraltma olmadan `object` kabul ediliyordu.** `Unknown -> AnyObject` istisnası kaldırıldı. `is object` ile daraltılmış kullanım geçerli; daraltılmamış kullanım artık tip hatası.
3. **Promise olmayan `await` sonucu `any` oluyordu.** `await` Promise içini açıyor; Promise olmayan operandın tipini koruyor. Yanlış dönüş tipinin bu yoldan saklanması önlendi.
4. **Mutable alias covariance tip güvenliğini bozuyordu.** Array/tuple elemanları, dictionary key/value türleri ve yazılabilir object/named/interface alanları invariant kontrol ediliyor; bilinmeyen uzunlukta array→tuple dönüşümü reddediliyor. Fresh array/tuple/object literal'ları bağlamsal olarak hedef tipi alırken mevcut nested alias'lar genişletilemiyor.
5. **`mount.update()` node türü geçişinde effect lifecycle'ı hatalıydı.** Statik→fonksiyon geçişinde effect başlatılmıyor; fonksiyon→fonksiyon geçişinde eski bağımlılık seti yenilenmiyor; fonksiyon→statik geçişinde effect abonelikleri bırakılmıyordu. Update artık eski effect'i durdurup yeni node fonksiyonu için yeniden kuruyor.
6. **Optional route parametresi sonraki segmenti yanlışlıkla tüketiyordu.** Route matcher artık optional parametreyi önce tüketmeyi dener; kalan desen eşleşmezse parametreyi atlayarak geri döner. Başarısız `(patternIndex, pathIndex)` durumları memoize edilir. Aynı eşleştirici browser, server ve middleware tarafından kullanılır. İki yorumun da geçerli olduğu optional-before-wildcard durumunda önce tüketme kuralı korunur.
7. **Body limiti aşıldığında request stream'i ve keep-alive bağlantısı bırakılıyordu.** `Content-Length` ön kontrolü ve streaming/chunked aşımında 413 yanıtı hazırlanırken Node response keep-alive dışı işaretleniyor ve `Connection: close` gönderiliyor. Böylece yanıt flush edildikten sonra okunmamış istek gövdesi olan bağlantı güvenle kapatılıyor; saldırgan kontrollü, sonu gelmeyen bir gövdeyi drain etmeye çalışılmıyor.
8. **Statik dosya servisinde containment kontrolü ile path'i yeniden açma arasında TOCTOU vardı.** Dosya canonical path kontrolünden sonra bir kez açılıyor; `fstat` ile normal dosya ve dosya kimliği doğrulanıyor. Platform destekliyorsa açılmış descriptor'ın `/proc/self/fd` veya `/dev/fd` hedefi root içinde ayrıca doğrulanıyor; `O_NOFOLLOW` mevcutsa kullanılıyor. Tam GET stream'i ile byte-range okumaları aynı açılmış descriptor'ı kullanıyor; HEAD/304/error yollarında descriptor kapatılıyor.
9. **`res.sendFile` / `res.download` path'i kontrol ettikten sonra yeniden açıyordu.** `stat` ile alınan kimlik açılmış descriptor'ın `fstat` sonucu ile karşılaştırılıyor; yalnızca doğrulanan descriptor metadata, tam gövde ve stream için kullanılıyor. HEAD/error/erken tamamlanma yollarında descriptor kapanıyor; aktif kaynak response finish/close ile durduruluyor ve `drain` gelene kadar pause ediliyor. Stream ilk byte'ı göndermeden önce finish hook'ları çalışıyor. İndirme adlarındaki kontrol karakterleri ve header ayraçları temizleniyor; Unicode adlar ASCII fallback ve RFC 5987 `filename*` ile aktarılıyor.
10. **Harici HTTP abort sinyali retry tetikleyebiliyordu.** Önceden iptal edilmiş signal fetch'i başlatmıyor; in-flight iptal ve abort edilebilir retry beklemesi yeni denemeyi durduruyor. Genel timeout sonrası futile retry yapılmıyor ve istek tamamlanınca harici signal listener'ı kaldırılıyor.

Regresyon kapsamı: generic identity ve kısıtlı generic dönüş/aritmetik/liste kabul edilir; kısıtsız generic dönüş ve sayı işlemi reddedilir; `unknown` yalnızca `object` olarak daraltılınca kabul edilir; scalar `await` gerçek tipini korur; mutable-container alias ihlalleri ve `mount.update()` lifecycle geçişleri doğrulanır; optional route ve body-limit senaryoları browser/server testleriyle kapsanır; statik servis için containment/path/stream/range symlink yarışları sınanır. `sendFile` için path swap, HEAD descriptor kapanışı, yanıt timeout/finish ve client disconnect sonrası cleanup, `drain` backpressure, session cookie; `download` için kontrol karakteri sanitizasyonu ve Unicode header encoding test edilir. HTTP için pre-abort, in-flight abort, 5xx/network retry backoff iptali, toplam timeout ve listener cleanup doğrulanır.

## Test ve doğrulama durumu

| Komut | Sonuç |
|---|---|
| `cargo test --test runtime --test dom --test types --test cli --test readme --quiet` | **222 geçti, 0 kaldı:** CLI 31, DOM 15, README 4, runtime 98, types 74. |
| `cargo test --all-targets` | **339 geçti, 0 kaldı.** `main` unit 3, CLI 31, dev 5, DOM 15, lang 62, README 4, regressions 43, runtime 98, server 4, types 74. Önceki `SIGTERM/143` bu çalıştırmada tekrarlanmadı. |
| `an_oversized_incomplete_chunked_body_gets_413_and_closes_the_connection` ve declared-length body-limit testi | Geçti: düzeltme öncesi raw TCP repro 413 + `Connection: keep-alive` döndürüp bağlantıyı 1 saniye açık bırakıyordu; düzeltme sonrası 413 + `Connection: close` ve socket kapanışı doğrulanıyor. Declared-length istek de istemci keep-alive talep etse dahi kapatılıyor. |
| Optional route browser/server regresyonları | Geçti: constrained optional, literal, required-parametre fallback'ı, wildcard skip/consume ve consume-first tie-break doğrulanıyor. |
| `mount_update_transitions_work_in_headless_chromium_when_available` | Geçti: Vey'in birleştirilmiş runtime kaynağı gerçek Chromium 154 DOM'unda çalıştırıldı; tarayıcı yoksa test açıkça skip eder. |
| `git diff --check` | Geçti. |
| `cargo fmt --all -- --check` | Repo genelinde önceden var olan biçim farkları gösterdi; bu yüzden tüm depo için temiz format sonucu verilemiyor. İlgisiz dosyalar topluca yeniden formatlanmadı; yeni HTTP/sendFile test bloklarında ek rustfmt farkı kalmadı. |

Tip-güvenliği commitleri `5fa2002` ve `800c332`, mount lifecycle düzeltmesi `f3d9967`, ilgili durum raporu `0f39b00`, route düzeltmesi/testleri `33b9269` ve body bağlantı politikası `7a0236f` `main` dalına pushlandı. Route durumu dokümantasyon commit'i `5561d80` de `main`'dedir; statik dosya TOCTOU düzeltmesi de `main`'e pushlandı. Server hedefi kaynak kodu kendi alt süreci process group üzerinden kapatıyor; `SIGTERM/143` bu tam-suite çalıştırmasında tekrarlanmadı.

## Henüz düzeltilmemiş, doğrulanmış yüksek öncelikli bulgular

### Runtime/server

- SSE yanıtlarında yavaş istemcilere karşı backpressure/buffer sınırı politikası hâlâ ele alınmıyor (`src/runtime/server.js:908-922`). HTTP caller abort ve `sendFile`/`download` descriptor yaşam döngüsü regresyonlarla kapatıldı.

### CLI/LSP ve deploy

- LSP rename aynı metinsel adı dosyanın tamamında topluyor; shadowed/ayrı scope'lardaki sembolleri de değiştirebilir (`src/cli/lsp.rs:317-365`). Rename capability ilan ediliyor; görünür regresyon testi yok.
- LSP `exit` bildirimi döngüyü sonlandırmıyor; bozuk/eksik frame EOF gibi sessizce ele alınıyor. Bilinmeyen yöntemler de JSON-RPC error yerine `result: null` dönüyor (`src/cli/lsp.rs:39-73, 93-98, 175`).
- LSP konumları lexer sütununu doğrudan kullanıyor; UTF-16 birimleri ve token içi cursor konumu doğru ele alınmıyor (`src/cli/lsp.rs:179-187, 787-800, 819-835`). Percent-encoded UTF-8 URI dosya yolu da byte'ları ayrı Unicode karakterlere çeviriyor (`src/cli/lsp.rs:924-950`).
- `deploy` hedefi senkronize etmediği için artık build'de olmayan eski dosyalar hedefte kalıyor (`src/cli/deploy.rs:53-89`).
- `migrate` dry-run değişiklik gerektiğini söylemesine rağmen exit status 0 veriyor (`src/cli/migrate.rs:124-128`). `vey.toml` içindeki `build.minify` okunuyor ama build akışında kullanılmıyor (`src/cli/config.rs:24-28, 98-107`; `src/cli/build.rs:46-55`).

Bunlar static audit bulgularıdır; her madde için ilgili mevcut regresyon testi bulunmadığından yeni testlerle doğrulanıp ayrı değişikliklerde ele alınmalıdır. LSP completion derinliği, cross-file rename/definition, codemod ve sağlayıcıya deploy gibi maddeler ise tek başına bug değil, özellik kapsamı eksikleridir.

## Dokümantasyon durumu

`EKSIKLER.md` içinde 22 Ekim snapshot'ından kalan bazı iddialar vardı; yönetici özeti ve kanıt tablosundaki doğrudan çelişkiler düzeltildi, eski sonuç bölümleri tarihsel olarak etiketlendi. `ROADMAP.md` güncel riskleri sıralıyor. Mutable-container covariance, `mount.update()` effect lifecycle, optional-route backtracking, oversized-body connection cleanup, statik dosya TOCTOU, `sendFile`/`download` descriptor yaşam döngüsü ve HTTP caller-abort/retry maddeleri regresyon testleriyle kapandı. SSE backpressure, kısıtlı generic üye erişimi, LSP scope/Unicode/protokol sorunları, deploy stale files, `build.minify` ve migrate dry-run sorunları hâlâ açık.

## Öncelikli sonraki işler

1. LSP rename'i ad metni yerine sembol/kapsam kimliğiyle çözmek; shadowing testi eklemek.
2. LSP UTF-16 position/URI çözümleme ve `exit`/bozuk frame protokol davranışını düzeltmek.
3. SSE backpressure için sınırlı buffer veya yavaş istemciyi kapatma politikası belirlemek ve regresyon testi eklemek.
4. Deploy hedefi için güvenli stale-file temizleme/senkronizasyonu ve `migrate` dry-run exit status sözleşmesini tanımlamak.
5. Kısıtlı generic'lerin üye erişimi ve nested alan birleşimlerinde bound-aware davranışını ayrıca denetlemek.
6. Rust format kontrolü için depo sözleşmesini netleştir; önceden var olan repo-geniş format farklarını ayrı değişiklik olarak ele al.
