# Vey kod denetimi — güncel özet

**Tarih:** 23 Ekim 2026
**İncelenen upstream başlangıç revizyonu:** `1626f550865a9694201e1f6d15cb106f49e7c14e` (`origin/main`)
**Çalışma dalı:** `manus/type-safety-audit-2026-10-23`
**Kapsam:** Rust tip denetleyicisi, JavaScript runtime/server, CLI/LSP, regresyon testleri ve mevcut dokümantasyon. Bu rapor denetim bulgularını özetler; tüm eksiklerin kapandığı anlamına gelmez.

## Bu değişiklikte doğrulanıp düzeltilenler

1. **Serbest `TypeVar` herhangi bir tipe atanabiliyordu.** `(Type::Var(_), _) | (_, Type::Var(_)) => true` kuralı kaldırıldı. Checker, etkin generic scope'larda deklarasyonla gelen type bound'ları genişletip assignability ve sayısal/string operatör kontrollerinde kullanıyor. Böylece `T` artık kendiliğinden `number` veya başka somut tip sayılmıyor; `T: number` gibi sınırlar kullanılmaya devam ediyor.
2. **`unknown`, daraltma olmadan `object` kabul ediliyordu.** `Unknown -> AnyObject` istisnası kaldırıldı. `is object` ile daraltılmış kullanım geçerli; daraltılmamış kullanım artık tip hatası.
3. **Promise olmayan `await` sonucu `any` oluyordu.** `await` Promise içini açıyor; Promise olmayan operandın tipini koruyor. Yanlış dönüş tipinin bu yoldan saklanması önlendi.
4. **Mutable alias covariance tip güvenliğini bozuyordu.** Array/tuple elemanları, dictionary key/value türleri ve yazılabilir object/named/interface alanları invariant kontrol ediliyor; bilinmeyen uzunlukta array→tuple dönüşümü reddediliyor. Fresh array/tuple/object literal'ları bağlamsal olarak hedef tipi alırken mevcut nested alias'lar genişletilemiyor.

Regresyon kapsamı: generic identity ve kısıtlı generic dönüş/aritmetik/liste kabul edilir; kısıtsız generic dönüş ve sayı işlemi reddedilir; `unknown` yalnızca `object` olarak daraltılınca kabul edilir; scalar `await` gerçek tipini korur; array/tuple/object/dictionary alias ihlalleri reddedilir ve fresh literal'lar doğrulanır.

## Test ve doğrulama durumu

| Komut | Sonuç |
|---|---|
| `cargo test --test types --test cli --test readme --test runtime --quiet` | **187 geçti, 0 kaldı:** types 74, CLI 31, README 4, runtime 78. |
| `cargo test --test server -- --nocapture --test-threads=1` | **Tamamlanmadı:** `a_malformed_path_does_not_take_the_server_down` başlarken test süreci `SIGTERM` aldı; exit status `143`. |
| `cargo test --all-targets --quiet` | **Tam-suite sonucu doğrulanmadı:** server hedefindeki tekrarlanabilir exit 143 çözülmeden başarı iddia edilemez. |
| `git diff --check` | Geçti. |
| `cargo fmt --all -- --check` | Repo genelinde çok sayıda biçim farkı gösterdi; bu yüzden tüm depo için temiz format sonucu verilemiyor. Denetim sırasında ilgisiz dosyaları topluca yeniden formatlamadım. |

Kod/test commitleri `5fa2002` ve `800c332`, `manus/type-safety-audit-2026-10-23` dalına pushlandı. Server hedefi kaynak kodu kendi alt sürecini process group üzerinden kapatıyor; SIGTERM’in test çalıştırıcısına ulaşma sebebi ayrıca araştırılmalı.

## Henüz düzeltilmemiş, doğrulanmış yüksek öncelikli bulgular

### Runtime/server

- `mount.update()` fonksiyon düğümüne geçişte reactive effect kurmuyor, fonksiyon düğümünden statik değere geçişte eski effect'i durdurmuyor (`src/runtime/mount.js:489-516`). DOM güncellenmeyebilir veya gereksiz yeniden çizilebilir.
- Ortadaki opsiyonel route parametresi sonraki literal segmenti yutabiliyor (`src/runtime/shared.js:584-593`); `/users/{id?}/posts` ve `/users/posts` eşleşmesinde.
- Büyük chunked request reddedilince request pause ediliyor fakat okunmayan veri drain/destroy edilmiyor (`src/runtime/server.js:272-284, 510-518`); bağlantı/kaynak tüketimi riski.
- Statik dosyada containment kontrolüyle `sendFile`'ın dosyayı açması arasında symlink swap TOCTOU penceresi var (`src/runtime/server.js:333-355, 415, 941-970`).
- Harici abort signal, retries açıksa geçici hata gibi yeniden denenebiliyor (`src/runtime/http.js:313-317`). SSE backpressure da ele alınmıyor (`src/runtime/server.js:817-839`).

### CLI/LSP ve deploy

- LSP rename aynı metinsel adı dosyanın tamamında topluyor; shadowed/ayrı scope'lardaki sembolleri de değiştirebilir (`src/cli/lsp.rs:317-365`). Rename capability ilan ediliyor; görünür regresyon testi yok.
- LSP `exit` bildirimi döngüyü sonlandırmıyor; bozuk/eksik frame EOF gibi sessizce ele alınıyor. Bilinmeyen yöntemler de JSON-RPC error yerine `result: null` dönüyor (`src/cli/lsp.rs:39-73, 93-98, 175`).
- LSP konumları lexer sütununu doğrudan kullanıyor; UTF-16 birimleri ve token içi cursor konumu doğru ele alınmıyor (`src/cli/lsp.rs:179-187, 787-800, 819-835`). Percent-encoded UTF-8 URI dosya yolu da byte'ları ayrı Unicode karakterlere çeviriyor (`src/cli/lsp.rs:924-950`).
- `deploy` hedefi senkronize etmediği için artık build'de olmayan eski dosyalar hedefte kalıyor (`src/cli/deploy.rs:53-89`).
- `migrate` dry-run değişiklik gerektiğini söylemesine rağmen exit status 0 veriyor (`src/cli/migrate.rs:124-128`). `vey.toml` içindeki `build.minify` okunuyor ama build akışında kullanılmıyor (`src/cli/config.rs:24-28, 98-107`; `src/cli/build.rs:46-55`).

Bunlar static audit bulgularıdır; her madde için ilgili mevcut regresyon testi bulunmadığından yeni testlerle doğrulanıp ayrı değişikliklerde ele alınmalıdır. LSP completion derinliği, cross-file rename/definition, codemod ve sağlayıcıya deploy gibi maddeler ise tek başına bug değil, özellik kapsamı eksikleridir.

## Dokümantasyon durumu

`EKSIKLER.md` içinde 22 Ekim snapshot'ından kalan bazı iddialar vardı; yönetici özeti ve kanıt tablosundaki doğrudan çelişkiler düzeltildi, eski sonuç bölümleri tarihsel olarak etiketlendi. `ROADMAP.md` güncel riskleri sıralıyor. Mutable-container covariance maddesi kapandı; kısıtlı generic üye erişimi, LSP scope/Unicode/protokol sorunları, deploy stale files, `build.minify` ve migrate dry-run sorunları hâlâ açık.

## Öncelikli sonraki işler

1. `mount.update()` için statik ↔ reactive geçişlerde effect lifecycle'ını düzeltip iki yönlü DOM regresyon testi eklemek.
2. LSP rename'i ad metni yerine sembol/kapsam kimliğiyle çözmek; shadowing testi eklemek.
3. LSP UTF-16 position/URI çözümleme ve `exit`/bozuk frame protokol davranışını düzeltmek.
4. Oversized request için açık drain/destroy politikası belirlemek; keep-alive ve eşzamanlı bağlantı testleri eklemek.
5. Optional route matching'i tüket/atla dallarıyla çözmek; optional-middle/literal testleri eklemek.
6. Deploy hedefi için güvenli stale-file temizleme/senkronizasyonu ve `migrate` dry-run exit status sözleşmesini tanımlamak.
7. Server integration testindeki process-group cleanup kök nedenini bulup ardından tam `cargo test --all-targets` çalıştırmak.
8. Kısıtlı generic'lerin üye erişimi ve nested alan birleşimlerinde bound-aware davranışını ayrıca denetlemek.
