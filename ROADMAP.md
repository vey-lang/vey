# Vey yol haritası

**Güncelleme:** 23 Ekim 2026
**Başlangıç revizyonu:** `1626f550865a9694201e1f6d15cb106f49e7c14e`

Bu liste, denetim sırasında doğrulanan davranış ve riskleri sıralar; tamamlandı işareti ancak regresyon testi ve ilgili doğrulama sonrası verilir.

## 1. Tip güvenliği ve değer doğruluğu

- [x] Serbest `TypeVar`'ı keyfî tiplere uyumlu sayan kuralı kaldır; deklarasyondaki generic bound'ları kullan.
- [x] `unknown` değerini `object` olarak kullanmadan önce açıkça daraltmayı zorunlu kıl.
- [x] Promise olmayan `await` operandının gerçek tipini koru.
- [x] Mutable array/tuple, dictionary key/value ve yazılabilir object/named/interface alanlarını invariant yap; fresh literal bağlamlamasını ve alias regresyonlarını test et.
- [x] Kısıtlı generic üye erişimini bound-aware yap; object/interface, nested/chained bound'ları ve missing/unbounded/recursive durumlarını test et.
- [x] Kısıtlı generic array/dictionary/union indekslemesini bound-aware yap; nested alan indeksleri, dictionary key/value ve unbounded/cyclic bound'ları test et.
- [x] `T: U` call constraint'lerini callee'nin transitive bound'ları ve explicit/inferred type argümanlarıyla doğrula; uyumsuz argümanları reddet.
- [x] Generic alias argümanlarında etkin caller bound'larını kullan; uyumlu `U`'yu kabul edip uyumsuz bound'u reddet.

## 2. Runtime ve routing doğruluğu

- [x] `mount.update()` statik ↔ fonksiyon geçişlerinde effect'i durdurup yeniden kur; fonksiyon→fonksiyon bağımlılık yenilemesini, cleanup'ı ve DOM davranışını mock DOM ile Chromium'da test et.
- [x] Optional route parametrelerini önce tüketip kalan desen başarısız olursa geri dönerek atla; constraint/literal/required-parametre/wildcard senaryolarını browser ve server testleriyle kapsa; belirsiz wildcard eşleşmelerinde consume-first davranışını koru.
- [x] Oversized `Content-Length` ve chunked body aşımında 413 yanıtı gönderildikten sonra bağlantıyı kapat; incomplete chunked socket kapanışını ve declared-length keep-alive isteğinde `Connection: close` davranışını test et.
- [x] Statik dosya containment denetimini descriptor tabanlı açılış/stream ile güçlendir; `fstat` ve açılmış descriptor hedefini doğrula, tam GET ve byte-range okumalarında aynı descriptor'ı kullan. Path swap, stream öncesi symlink swap ve range-open yarışlarını regresyon testleriyle kapsa.
- [x] `res.sendFile`/`res.download` için dosyayı bir kez aç, `fstat` ile doğrula ve aynı descriptor'dan stream et; HEAD/timeout/error/disconnect cleanup, `drain` backpressure, session finish hook'ları, güvenli download adları ve path-swap regresyonlarını test et.
- [x] HTTP caller abort'unu retry'den ayır; pre-abort/in-flight abort, 5xx ve network-error backoff iptali, toplam timeout sonrası yeniden denememe ve external listener cleanup senaryolarını test et.
- [x] SSE `write()` backpressure'ında stream ve heartbeat'i durdurup yavaş istemciyi kapat; tek frame'i 1 MiB ile sınırla ve iki durumu deterministik regresyonlarla test et.

## 3. LSP doğruluğu ve protokol uyumu

- [x] Rename/definition/references işlemlerini metinsel ad yerine çözümlenmiş binding kimliğiyle yap; nested shadowing, sibling scopes, assignment read/write, import alias ve exact-token edit testleri ekle.
- [x] `exit`/`shutdown` yaşam döngüsünü, bozuk/eksik frame parse-error davranışını ve unknown request/notification ayrımını JSON-RPC'ye uygunlaştır; Content-Length sınırı ve subprocess testleri ekle.
- [x] LSP `position.character` ve `Range` değerlerini UTF-16 birimlerine dönüştür; token içi cursor ve astral Unicode rename/range testleri ekle.
- [x] `file://` URI yollarını UTF-8 byte dizileriyle decode et, diagnostic URI'larını encode et; malformed escape, UTF-8, local/remote authority ve percent-encoded Unicode-path testleri ekle.
- [x] Completion'ı cursor ve kapsam-duyarlı hâle getir; bunu mevcut rename/position düzeltmelerinden ayrı özellik işi olarak takip et.

## 4. CLI, build ve yayınlama davranışı

- [x] `deploy --target` için sürümlü/validasyonu yapılmış sahiplik manifesti ekle; yalnızca önceki Vey deploy'undan kalan stale dosyaları sil, unmanaged host dosyalarını koru, collision/traversal/symlink ve hardlink alias senaryolarını regresyonla test et.
- [x] `migrate` dry-run'da değişiklik varsa exit status 1 döndür; dosyanın yazılmadığını ve güncel dosyanın exit 0 verdiğini regresyonla doğrula.
- [x] `[build].minify` no-op davranışını kaldır: `true` static, Node ve deploy build'lerinde açıkça reddedilir; gerçek minification desteklenmiyor.
- [ ] Gerçek minification eklenirse JS/CSS/HTML kapsamını ve source map doğruluğunu tanımlayan güvenli parser/tool tabanlı bir uygulama seç.
- [x] Lint'in config dosyasını hedef dosyanın proje kökünden çözmesini sağla.
- [x] CLI option parser'ında komut-başına geçerli seçenekleri doğrula; etkisiz ve yanlış yazılmış seçeneklerin sessizce yutulmasını önle.

## 5. Doğrulama ve dokümantasyon

- [x] Tam `cargo test --all-targets` çalıştır: **371 geçti, 0 kaldı**; CLI 48, types 80, runtime 100 ve server hedefindeki dört test (`a_malformed_path_does_not_take_the_server_down` dahil) başarılı. Server test harness'ındaki `kill` komutuna `--` eklendi ve process group lideri reap edilmeden kapatılıyor; seri server hedefi 0,49 saniyede geçti, `SIGTERM/143` tekrarlanmadı.
- [x] `EKSIKLER.md` içindeki tamamlanmış LSP/config/lint/build ve deploy iddialarını güncel kaynakla uzlaştır; eski bulguları tarihsel etiketle.
- [ ] Rust format kontrolünü temiz hâle getir veya repository'nin format sözleşmesini belirleyip tüm mevcut dosyaları ayrı, gözden geçirilebilir değişiklikte formatla.
- [ ] Her yüksek riskli bulgu için yeniden üretim testi ya da açıkça belgelenmiş “statik audit, henüz test edilmedi” durumu tut.
