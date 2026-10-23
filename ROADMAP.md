# Vey yol haritası

**Güncelleme:** 23 Ekim 2026
**Başlangıç revizyonu:** `1626f550865a9694201e1f6d15cb106f49e7c14e`

Bu liste, denetim sırasında doğrulanan davranış ve riskleri sıralar; tamamlandı işareti ancak regresyon testi ve ilgili doğrulama sonrası verilir.

## 1. Tip güvenliği ve değer doğruluğu

- [x] Serbest `TypeVar`'ı keyfî tiplere uyumlu sayan kuralı kaldır; deklarasyondaki generic bound'ları kullan.
- [x] `unknown` değerini `object` olarak kullanmadan önce açıkça daraltmayı zorunlu kıl.
- [x] Promise olmayan `await` operandının gerçek tipini koru.
- [x] Mutable array/tuple, dictionary key/value ve yazılabilir object/named/interface alanlarını invariant yap; fresh literal bağlamlamasını ve alias regresyonlarını test et.
- [ ] Kısıtlı generic'lerin tüm işlemlerini, özellikle nested/container ve üye erişimlerini, bound-aware şekilde test et.

## 2. Runtime ve routing doğruluğu

- [x] `mount.update()` statik ↔ fonksiyon geçişlerinde effect'i durdurup yeniden kur; fonksiyon→fonksiyon bağımlılık yenilemesini, cleanup'ı ve DOM davranışını mock DOM ile Chromium'da test et.
- [x] Optional route parametrelerini önce tüketip kalan desen başarısız olursa geri dönerek atla; constraint/literal/required-parametre/wildcard senaryolarını browser ve server testleriyle kapsa; belirsiz wildcard eşleşmelerinde consume-first davranışını koru.
- [x] Oversized `Content-Length` ve chunked body aşımında 413 yanıtı gönderildikten sonra bağlantıyı kapat; incomplete chunked socket kapanışını ve declared-length keep-alive isteğinde `Connection: close` davranışını test et.
- [x] Statik dosya containment denetimini descriptor tabanlı açılış/stream ile güçlendir; `fstat` ve açılmış descriptor hedefini doğrula, tam GET ve byte-range okumalarında aynı descriptor'ı kullan. Path swap, stream öncesi symlink swap ve range-open yarışlarını regresyon testleriyle kapsa.
- [x] `res.sendFile`/`res.download` için dosyayı bir kez aç, `fstat` ile doğrula ve aynı descriptor'dan stream et; HEAD/timeout/error/disconnect cleanup, `drain` backpressure, session finish hook'ları, güvenli download adları ve path-swap regresyonlarını test et.
- [x] HTTP caller abort'unu retry'den ayır; pre-abort/in-flight abort, 5xx ve network-error backoff iptali, toplam timeout sonrası yeniden denememe ve external listener cleanup senaryolarını test et.
- [ ] SSE backpressure için sınırlı tampon veya yavaş istemciyi kapatma politikası ekle.

## 3. LSP doğruluğu ve protokol uyumu

- [ ] Rename'i metinsel ad yerine çözümlenmiş sembol/kapsam kimliğiyle uygula; shadowing testi ekle.
- [ ] `exit`, bozuk/eksik frame ve bilinmeyen JSON-RPC yöntemi davranışlarını protokole uygun biçimde sonlandır/yanıtla; subprocess entegrasyon testleri ekle.
- [ ] LSP `position.character` ve `Range` değerlerini UTF-16 birimlerine dönüştür; token ortası cursor ve astral Unicode testleri ekle.
- [ ] `file://` URI percent-decoding'i UTF-8 byte dizilerini doğru çözecek biçimde düzelt; Unicode dosya adı testi ekle.
- [ ] Completion'ı cursor ve kapsam-duyarlı hâle getir; bunu mevcut rename/position düzeltmelerinden ayrı özellik işi olarak takip et.

## 4. CLI, build ve yayınlama davranışı

- [ ] `deploy` hedefinde kaynak build'de olmayan stale dosyaların kontrollü ve güvenli temizliğini tanımla; hedefte kullanıcı dosyaları koruma testleri ekle.
- [ ] `migrate` dry-run exit status sözleşmesini belirle ve CI kullanımına göre test et.
- [ ] `[build].minify` ayarı için gerçek build davranışı ekle veya config şemasından tamamen çıkar; şu an dokümantasyonda no-op olduğu belirtiliyor.
- [ ] Lint'in config dosyasını hedef dosyanın proje kökünden çözmesini sağla.
- [ ] CLI option parser'ında komut-başına geçerli seçenekleri doğrula; etkisiz ve yanlış yazılmış seçeneklerin sessizce yutulmasını önle.

## 5. Doğrulama ve dokümantasyon

- [x] Tam `cargo test --all-targets` çalıştır: **339 geçti, 0 kaldı**; runtime 98, server hedefindeki dört test (`a_malformed_path_does_not_take_the_server_down` dahil) başarılı. Önceki tekil `SIGTERM/143` bu çalıştırmada tekrarlanmadı; kök neden ancak yeniden üretilebilirse ayrıca incelenecek.
- [x] `EKSIKLER.md` içindeki tamamlanmış LSP/config/lint/build iddialarını güncel kaynakla uzlaştır; eski bulguları tarihsel etiketle.
- [ ] Rust format kontrolünü temiz hâle getir veya repository'nin format sözleşmesini belirleyip tüm mevcut dosyaları ayrı, gözden geçirilebilir değişiklikte formatla.
- [ ] Her yüksek riskli bulgu için yeniden üretim testi ya da açıkça belgelenmiş “statik audit, henüz test edilmedi” durumu tut.
