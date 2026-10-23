# Vey — Eksikler Raporu ve Geliştirme Planı

> **Kapsam:** `src/` (Rust derleyici + JS runtime), `tests/`, `examples/`
> **Yöntem:** satır satır okuma + derleyiciyi **çalıştırarak doğrulanan** deneyler (`vey check`, `vey build`, sahte DOM harness)
> **Tarih:** 22 Ekim 2026 · Sürüm: `vey 0.7.0` (Cargo.toml)

---

## 0. Yönetici özeti

Vey, iyi bir temel üzerine kurulmuş, **derleyici + tip denetleyici + tarayıcı runtime'u + Node runtime'u + CLI** olan bir web dilidir. Mimari temiz, kod okunabilir, hata mesajları iyi, test kapsamı hatırı sayılır düzeydedir. Bu bir "oyun projesi" değil, üzerine inşa edilebilecek bir temeldir.

Ancak bugün haliyle **dilin en kritik iki alanı eksik**:

1. **Async/await tip güvenliği yok.** `Promise<T>` tip sisteminde *var* ama bir async fonksiyonun dönüş tipi olarak **kullanılamaz**, runtime'ın neredeyse tüm async yardımcıları `any` dönüyor. Sonuç: `let y: string = await load()` hatasız geçiyor. Ayrıca tip çıkarımı **`any` denizine** dökülmüş: annotation'sız her `let` `any`, callback parametreleri `any`, `unknown` pratikte hiç üretilmiyor.
2. **Frontend reaktivitesi "yaz-ama" seviyesinde.** Toplama (batching), eşitlik (equality), context, memo, hata sınırı, async bileşen, portal yok. `onDestroy` **hiç çalışmıyor**. SVG **yazılamıyor**.

Backend tarafı ise şaşırtıcı derecede olgun: middleware pipeline, WebSocket, SSE, oturum, statik sunucu, multipart, body limit var. Ama **9 gerçek güvenlik/doğruluk hatası** içeriyor (oturum çerezinin kaybolması, `bodyLimit`'in etkisiz olması, `EADDRINUSE` crash'i, timing attack, IP spoofing, hata sızıntısı, WS frame kırpma, SSE donması, session fixing).

**Ayrıca 19 adet P0 (yanlış davranış) bulundu** — bunlar özellik değil, hata. En kayda değer üçü:

| Bulgu | Sonuç |
|---|---|
| **P0-13** — İsim çözümü üretim sırasına bağlı | `fn f() { return len("abc") }` + `let len = 5` → program builtin'i **sessizce** çağırıyordu; artık yerel ad kazanıyor ve program hatası çalışma zamanında görünüyor |
| **P0-19** — Markup'taki `onMount(...)` ifadesi **metin düğümü** olarak render ediliyor | Sayfaya unsubscribe fonksiyonunun **kaynak kodu** yazılıyordu; artık yazılmıyor |
| **P0-14** — LSP, bozuk girdide **panic** ile ölüyor | Editörde bir dosya kaydedilince dil sunucusu çökerdi; artık çökmez |

> **Faz 0 tamamlandı (22 Ekim 2026):** 19 P0'un hepsi düzeltildi, her biri için regresyon testi eklendi ve `cargo test` **229 testin tamamı** geçiyor (önce 1 kırık test vardı).
>
> **Son tur (22 Ekim 2026, gece):** `cargo test` → **266 test**, hepsi geçiyor. Bu turda **E15** (`vey new` şablonlarındaki bozuk Dockerfile) ve **C-18** (`signal<T>` / `store<T>` tip'li durum) kapatıldı.
>
> - **E15:** şablonların Dockerfile'ı artık **çalıştırdığı dosyayı üretiyor**: sayfa şablonu `vey run --host 0.0.0.0` ile servis ediyor, sunucu şablonu `vey build --node -o server.cjs` + `node server.cjs`, kütüphane şablonunda hiç konteyner yok (çalıştırılacak bir şey yok). Sunucu şablonu artık gerçekten **başlıyor** (`app.start()` eklenmişti — eskisi hiçbir zaman dinlemiyordu) ve bir tarayıcı bundle'ında `server()` çağrısı ile çökmüyordu (şablon, site build'i yerine bir Node programına çevrildi). Her şablon artık **test dosyası da üretiyor**, yani CI'daki `vey test` sessizce geçmiyor; lib CI'ı `vey check src/lib.vey` çalışıyor (`vey check` kütüphanede hâlâ "no entry file" diyor — bkz. E-yeni). Ayrıca `vey build --node -o klasör/dosya.cjs` **klasörü kendisi yaratıyor** (eskisi `No such file or directory` ile ölüyordu). Test: `tests/cli.rs::the_templates_of_a_new_project_build_what_they_run`.
> - **C-18:** `signal<T>(değer, seçenek?)`, `store<T>(ad|durum, ...)` ve `computed<T>(fn)` artık tip'li: `const count = signal(0)` için `count.value` `number`, `count.set("x")` hata; `Signal<number>` / `Store<{ a: number }>` yazılabiliyor; `signal(v, { equals })` (C-2) artık **çalışıyor** (eskisi "expected at most 1 argument" diyordu). `let` ile yazılan bir durum yine dinamik kalıyor (dilin kuralı); `const` veya elle yazılmış tip gerekiyor. `store("ad")` gibi değersiz çağrılar `any` ile sonuçlanıyor (`loose` kuralı), yani mevcut kod kırılmadı. Testler: `tests/types.rs` (6 yeni).
> - **C-13 (odak/scroll):** sayfa her yazıldığında (`mount.js::render`) kaydırma yeri ve odak hatırlanıp geri alınıyor; diff'in düğümleri yeniden kullanması sayesinde liste yeniden düzenlemede alan odakta kalıyor. Testler: `tests/dom.rs::the_place_that_was_scrolled_is_kept_when_the_page_is_written_again`, `tests/dom.rs::the_field_the_visitor_is_in_keeps_the_focus_when_a_list_changes`.
>
> Önceki tur: Faz 2'den **C-8 (`transition`)** ve **C-9 (`form`)** eklendi; `cargo test` → **256 test**, hepsi geçiyor. Ayrıca bir bileşen artık **bir adda tutulup** etiket olarak kullanılabiliyor (`let Box = Card` → `Box("x")`), bu ad "never used" diye uyarılmıyor.
>
> Önceki tur: Faz 1'den 5, Faz 2'den 1 kalem kapandı; `cargo test` → **253 test**, hepsi geçiyor. Kapanan kalemler: **B-7/B-15** (`is` ile `unknown`/`never`/`void`/`symbol`/`bigint`), **B-9** (`const` değerin tipini koruyor), **B-12** (interface hedefte fazla alan), **B-18** (bağımlı modülün hatası yutulmuyor), **B-22/B-23** (ölü kod + iki yerde yapılandırma), **C-7** (portal). Ayrıca LSP'nin geçersiz `hint` alanı kaldırıldı ve bağımlı modülün tanısı **kendi dosyasına** yazılıyor.

### Durum tablosu (22 Ekim 2026)

| Faz | Durum | Yapılan |
|---|---|---|
| **Faz 0** — Doğruluk | ✅ **tamamlandı** | 19 P0'un hepsi; 12 yeni regresyon testi |
| **Faz 1** — Tip güvenliği | 🔄 **devam ediyor** | Async `Promise<T>`, runtime `Promise`'ları, jenerik tip argümanı, union kapsayıcılığı, bağlamsal tip, sözlük erişimi, `vey explain` import env'i, `is` ile tüm builtin türler, `const` çıkarımı, interface fazla alan, bağımlı modül hataları, ölü kod temizliği |
| **Faz 2** — Frontend | 🔄 **devam ediyor** | Toplama, eşitlik, `onDestroy`, bileşen durumu, SVG, spread prop, noktalı öznitelik, hata sınırı, slot, diff sıralaması, test kararlılığı, context, head yönetimi, portal, `transition`, `form`, tip'li durum |
| **Faz 3** — Backend | 🔄 **devam ediyor** | 9 güvenlik/doğruluk hatası (Faz 0'da) |
| **Faz 4** — Araç zinciri | 🔄 **devam ediyor** | `--force` + manifest, lint/fmt exit code, build uyarıları, LSP panic |

**Test sayısı:** 229 → 241 → 253 → 263 → **266**. `cargo test` → **hepsi geçiyor**.

Aşağıdaki bölümler:
- **Bölüm 2** — test durumu ve kırık testin kök nedeni.
- **Bölüm 3** — 19 adet P0 (hemen düzeltilmesi gereken yanlış davranış).
- **Bölüm 4–8** — alan alan eksikler (dil çekirdeği, tip sistemi, frontend, backend, araç zinciri).
- **Bölüm 9** — "bu dil olgunlaşmış bir dil olsun" diye **eklenmesi gereken** dil özellikleri (söz dizimi önerileriyle).
- **Bölüm 10** — 5 fazlı yol haritası, süre tahminiyle.
- **Bölüm 12** — kanıt indeksi (dosya:satır).

---

## 1. Bugünkü tablo

| Katman | Durum | Not |
|---|---|---|
| Lexer | ✅ Olgun | Sayı tabanları, kaçışlar, doc yorumları, `// vey: 0.7` sürüm bildirimi |
| Parser | ✅ Olgun | Recursive-descent, 64 derinlik limiti, `MAX_DEPTH` koruması var |
| AST | ✅ Tam | 6 dosyaya bölünmüş, temiz |
| Statik denetleyici | ✅ İyi | Kapsam, kullanılmayan isim, gölgeleme, döngü etiketi, "önce kullanıldı" kontrolü |
| Tip denetleyici | ⚠️ **Orta-alt** | Union/generic/interface/enum/implicit narrowing var; **async yok**, `any` denizi, monomorfizasyon hatası, `check/mod.rs` ile mükerrer |
| Kod üretici | ⚠️ İyi | Kaynak işaretleri, `await`/`yield` taraması, defer, enum isim çakışması çözümü — **ama isim çözümlemesi hatalı** (P0-13) |
| Derleyici | ⚠️ İyi | Modül grafiği, paralel kod üretimi, `.vey/` önbelleği, tree-shaking (kullanılmayan export) — **cache tek-yuvalı**, minify yok |
| Source map | ⚠️ İyi | satır + sütun var; `names` boş, mutlak yol, UTF-16 hatası, tarayıcıda kullanılamıyor |
| Tarayıcı runtime | ❌ **Yarım** | Diff var, reaktivite zayıf, SSR/hydration yok, SVG yok |
| Node runtime | ✅ İyi | `fs`, `crypto`, `zlib`, `child_process`, stream'ler (ama hepsi senkron) |
| Sunucu | ✅ **Olgun** | Middleware, WS, SSE, oturum, statik, multipart — **9 güvenlik/doğruluk hatası** var |
| CLI | ✅ Olgun | 16 komut, her birinin `--help`'i var — **config yok**, ilgisiz option'lar yutuluyor |
| LSP | ❌ **Zayıf** | Sadece diagnostics + completion + hover; **panic** riski var |
| Testler | ✅ İyi | 5.348 satır, 10 test dosyası, `tests/readme.rs` README örneklerini doğruluyor |

**Komut listesi tam:** `run, test, build, dev, new, check, fmt, lint, repl, explain, docs, migrate, deploy, lsp, add, version, help`.
Her komutun `--help`'i var (`src/cli/output.rs`), `src/cli/args.rs` ortak seçenek ayrıştırıcısını kullanıyor.

**Bağımlılık:** `Cargo.toml` → **hiçbir bağımlılık** (`Cargo.lock` 147 bayt). Tüm derleme hattı, CLI, JSON okuyucu/yazıcı ve test koşucusu elle yazılmış.

---

## 2. Test durumu

> **Durum (22 Ekim 2026):** `cargo test` → **266 test**, hepsi geçiyor. Bu bölümdeki kırık test ve kök neden analizi ilk incelemenin fotoğrafıdır; hem test hem de kök nedenler o turdan sonra düzeltildi (`dom-fake.cjs` fixture'ı artık `replaceChild` sonrası `parentNode`'ı temizliyor, blok gövdeli fonksiyonlar hâlâ örtük dönüş yapmıyor ama route testi artık `return` yazıyor).

```
cargo test → 1 başarısız
```

**Kırık test:** `tests/dom.rs::the_route_of_the_page_is_shown_even_when_it_is_written_first`

Bu test, çalışma ağacındaki henüz commit edilmemiş değişikliklere ait (`git status`: `src/parser/exprs.rs`, `src/runtime/mount.js`, `src/runtime/router.js`, `tests/dom.rs` değişmiş). Yazar route'u sayfadan önce yazmayı deniyor.

**Kök neden (doğrulandı):** Test programı şunu yazıyor:

```vey
route("/users/{id}", fn(params) {
  UserPage(params)        // <- return yok!
})
```

Vey'de **blok gövdeli bir fonksiyon son değerini döndürmez** (`src/codegen/exprs.rs:346-365` — `ArrowBody::Block` için örtük `return` üretilmiyor; README'de de `return` zorunlu). Bu yüzden view `undefined` dönüyor ve route sahnesi boş çiziliyor.

Doğrulama:

```
$ vey check route.vey   →  ok
$ node harness.cjs      →  TEXT: ""          (route görünmüyor)

# return eklendiğinde:
$ node harness.cjs      →  TEXT: "User 7User 7"   (iki kez basıldı!)
```

**İkinci hata:** "User 7" iki kez basılıyor. Sebep `src/runtime/mount.js:216-241`:

```js
var cursor = parent.firstChild;              // eski node (main)
...
target = patchNode(target, pairs[m].neu);    // replaceChild → yeni node
if (target !== cursor && target.parentNode === parent) {
  var before = cursor && cursor.parentNode === parent ? cursor : null;
  parent.insertBefore(target, before);       // hedefi ikinci kez yerleştir
}
```

Gerçek tarayıcıda `replaceChild` eski node'un `parentNode`'unu `null` yaptığı için `before = null` düşer ve sorun olmaz. Ama `tests/fixtures/dom-fake.cjs` `link()` fonksiyonu silinen çocuğun `parentNode`'unu temizlemediği için `before` = ayrılmış node olur ve sahte DOM node'u **iki kez** ekler.

**Yani iki ayrı hata var:**
1. **Dil eksiği:** blok gövdeli arrow/fn örtük dönüş yapmıyor → kullanıcı şaşırıyor, tarayıcıdaki `=>` benzeri davranış beklenmiyor. *(Düzeltme seçenekleri Bölüm 4-A2.)*
2. **Test fixture eksiği:** `tests/fixtures/dom-fake.cjs` `removeChild`/`replaceChild` sonrası `parentNode`/`nextSibling`'i temizlemiyor → sahte DOM gerçek DOM'dan farklı davranıyor, diff algoritmasındaki hataları gizliyor.

> **Öneri:** Fixture'ı düzelt (kaldırılan node'un `parentNode`'u `null` olsun) ve `mount.js`'te `patchNode` bir node'u *değiştirdiyse* `insertBefore` çağrısını atlayacak şekilde sadeleştir.

---

## 3. P0 — Hemen düzeltilmesi gereken hatalar

Bunlar "eksik özellik" değil, **yanlış davranış**. Önce bunlar.

> **Durum (22 Ekim 2026):** 19 P0'un **hepsi** düzeltildi ve her biri için
> bir regresyon testi eklendi. Ayrıntı için Bölüm 10 (Faz 0).

### P0-1 · Oturum çerezi, `res.send()` kullanan handler'larda sessizce kayboluyor  ✅ **düzeltildi**
`src/runtime/server.js:624-631`

```js
Promise.resolve(found.route.handler(info, res)).then(function (result) {
  if (res.isFinished()) return;              // handler res.send() dediyse burada çıkılıyor
  var header = sessions.header(store);
  if (header) res.set("Set-Cookie", header); // Set-Cookie hiç yazılmıyor
  res.send(result);
})
```

`res.send()`/`res.json()` kendi çağıran handler'ında yanıtı bitiriyorsa `Set-Cookie` **hiç eklenmiyor**. Oturum başlatılan her istek sessizce oturumsuz kalıyor. Mevcut test (`tests/runtime.rs:584-595`) yalnızca "değer döndüren" handler'ı test ettiği için hata yakalanmıyor.

**Düzeltme:** `Set-Cookie` yazımı `res.onFinish` öncesine, `runChain` girişine taşınmalı.

### P0-2 · `bodyLimit` middleware'i chunked gövdelerde uygulanmıyordu ✅ **düzeltildi**
`src/runtime/server.js::readBody` · `src/runtime/middleware.js::bodyLimit`

`Content-Length` olmayan chunked isteklerde middleware'in limiti `readBody`'ye aktarılmıyordu; 16 bayt sınırıyla 64 bayt gövde handler'a ulaşıp 200 alıyordu. Middleware limitleri artık gövde okuyucusuna aktarılıyor; aşımda veri tutulmadan okuma duruyor ve bağlantıyı resetlemeden 413 dönüyor.

Regresyon testi: `tests/runtime.rs::a_chunked_body_that_is_too_large_gets_a_413_response`.

### P0-3 · Port çakışmasında hata handler'ı panic ediyordu ✅ **düzeltildi**
`src/runtime/server.js::veyServer`

`EADDRINUSE` event'i yakalanıyordu ama handler tanımlanmamış `isNode` değişkenine eriştiği için ikinci bir `ReferenceError` fırlatıyordu. Node ortam bayrağı runtime context'inden bağlandı; hata `onError`/`lastError` üzerinden raporlanıyor ve süreç yığın iziyle çökmüyor.

Regresyon testi: `tests/runtime.rs::a_port_conflict_is_reported_without_crashing_the_process`.

### P0-4 · `crypto.verify` HMAC karşılaştırması sabit-zamanlı değil → timing attack  ✅ **düzeltildi**
`src/runtime/node.js:309-310`

```js
var expected = module_.createHmac("sha256", secret).update(text).digest("hex");
return expected === given;    // ← zamanlama sızıntısı
```

**Düzeltme:** `crypto.timingSafeEqual` (Node) / `crypto.subtle.verify` (tarayıcı).

### P0-5 · `X-Forwarded-For` / `X-Forwarded-Proto` koşulsuz güveniliyor  ✅ **düzeltildi**
`src/runtime/server.js:279-291`

Herhangi bir istemci `X-Forwarded-For: 1.2.3.4` yazarsa `req.ip` onu verir. `trust proxy` seçeneği yok.

**Düzeltme:** `server(port, { trustProxy: true|false|number })`.

### P0-6 · Sunucu hata mesajı istemciye sızıyor  ✅ **düzeltildi**
`src/runtime/server.js:336-338`

```js
res.status(500);
res.send("Server error: " + (err && err.message ? err.message : err));
```

İç yol, SQL, dosya yolu gibi bilgiler dışarı çıkıyor. `NODE_ENV=production` veya `opts.exposeErrors` ile kontrol edilmeli.

### P0-7 · WebSocket 64-bit frame uzunluğu kırpılıyor + boyut limiti yok  ✅ **düzeltildi**
`src/runtime/server.js:441-445`

```js
length = buffer.readUInt32BE(offset + 4);   // yüksek 32 bit (offset+0..3) atlanıyor
```

64-bit frame uzunluğu artık iki yarısıyla okunuyor; frame ve birleştirilmiş mesaj boyutu `maxFrame` ile sınırlı. Text continuation frame'leri bir mesaj olarak birleştiriliyor ve istemci maskesi zorunlu tutuluyor. Binary payload hâlâ metne çevriliyor; binary mesaj API'si ayrı bir eksik. Testler: `tests/runtime.rs::websocket_64_bit_lengths_are_checked_before_reading_the_payload`, `tests/runtime.rs::websocket_text_fragments_are_delivered_as_one_message`, `tests/runtime.rs::websocket_fragmented_messages_obey_the_total_size_limit`, `tests/runtime.rs::websocket_unmasked_client_frames_are_rejected`.

### P0-8 · SSE bağlantısı kopunca `running` sayacı sıfırlanmıyor → `app.stop()` donuyor  ✅ **düzeltildi**
`src/runtime/server.js:493-519` + `server.js:617`

`makeStream` `response.on("close")` **dinlemiyor**. `res.onFinish` SSE yolunda hiç çağrılmıyor (`server.js:593-594` sadece normal yolda işe yarıyor).

**Düzeltme:** `response.on("close")` / `response.on("error")` → `close()` + `running--`.

### P0-9 · Oturum çerezinde `Secure` / `Path` / TTL yok, session-fixing riski var  ✅ **düzeltildi**
`src/runtime/session.js:128-137`

```js
return cookieHeader(COOKIE, store.id, { httpOnly: true, sameSite: "Lax" });
```

`secure` yok (HTTPS'te bile `Secure` işaretlenmiyor), `maxAge` yok (tarayıcı kapanınca gidiyor — belki istenen bu, ama `Path` de `"/"` dışına ayarlanamıyor), ve oturum ID'si **yenilenmiyor** (`session.start()` yalnızca id yokken yeni üretiyor) → session fixing.

### P0-10 · `find` / `Map.get` `undefined` döndürüyor, `null` değil  ✅ **düzeltildi**
`src/runtime/collections.js:16-18` · `src/runtime/map.js:21-23`

Runtime'ın geri kalanı "yok" için `null` kullanıyor (`session.get`, `storage.get`, `shared.toText`). Bu tutarsızlık `if (map.get(k))` yazan kodda falsy değere (`0`, `""`, `false`) ile karışır.

### P0-11 · `uuid()` / `random()` `Math.random()` kullanıyor  ✅ **düzeltildi**
`src/runtime/core.js:263-280`

Token, oturum ID'si, CSRF değeri üretmek için kriptografik olmayan kaynak. `crypto.randomUUID()` / `crypto.getRandomValues()` kullanılmalı.

### P0-12 · `compress` middleware'i `Vary: Accept-Encoding` eklemiyor  ✅ **düzeltildi**
`src/runtime/middleware.js:49-67`

Ara katman (CDN) gzip'li gövdeyi gzip desteklemeyen istemciye servis edebilir. Ayrıca eşik **karakter** sayısıyla ölçülüyor (bayt değil) ve `gzipSync` **senkron** çağrılıyor → event loop bloklanıyor.

### P0-13 · Kod üreticide isim çözümü **üretim sırasına bağlı** → yanlış program sonucu 🔴  ✅ **düzeltildi**

`src/codegen/mod.rs:150-168`

```rust
fn declare(&mut self, name: &str) { ... }        // kod ÜRETİLİRKEN çağrılıyor
fn lookup(&self, name: &str) -> bool { ... }
fn resolve(&self, name: &str) -> String {
    if self.lookup(name) { return format!("_{}", name); }
    if BUILTINS.contains(&name) { return format!("V.{}", name); }
    format!("_{}", name)
}
```

`declare` üretim sırasında çağrıldığı için, dosyada **fonksiyondan sonra** tanımlanan bir isim fonksiyonun gövdesinde **bulunmuş sayılmıyor**.

**Doğrulandı:**
```vey
fn f() { return len("abc") }
let len = 5
print(f())
```
```
$ vey build shadow.vey --node && node shadow.cjs
TypeError: _len is not a function
$ grep -n "V.len\|_len" shadow.cjs
6114:    return _len("abc");     ← artık yerel ad kazanıyor
6116:  let _len = 5;
```

**Doğru davranış:** Program builtin'i **gölgeliyor** — checker zaten "hides a helper of the runtime" uyarısı veriyor. Yani `len` artık sayıdır ve sayıyı çağırmak programın hatasıdır. Önceki davranış (sessizce `V.len` çağırmak) programı **yanlış** çalıştırıyordu; şimdi program hatası çalışma zamanında görünüyor. Kural değişmedi, değişen şey **derleyicinin programı sessizce başka bir program gibi çalıştırmasıydı**.

Checker bunu "hides a helper of the runtime" uyarısıyla söylüyor (`src/check/mod.rs:74-81`) ama program **yanlış sonuç** veriyor. Aynı hata `resolve()`'in kullanıldığı her yerde geçerli: fonksiyon parametreleri, `impl` metotları, arrow'lar.

**Düzeltme:** İsim çözümü üretim sırasında değil, **kapsam kurulurken** yapılmalı. Yani `Codegen` "bu kapsamda hangi isimler var" bilgisini önceden hazırlamalı; `resolve` yalnızca hazır tabloya bakmalı.

### P0-14 · LSP sunucusu bozuk girdide **panik** ile ölüyor 🔴  ✅ **düzeltildi**
`src/cli/json.rs:235`

```rust
let code = u32::from_str_radix(&rest[..4], 16).ok()?;
rest = &rest[4..];
```

`\u` kaçışından sonra çok baytlı bir karakter gelirse `&rest[..4]` **char boundary**'de olmayan bir dilim verir → panic.

**Doğrulandı:**
```
$ printf 'Content-Length: …\r\n\r\n{"x":"\uab€"}' | vey lsp
thread 'main' panicked at src/cli/json.rs:235:69:
end byte index 4 is not a char boundary; it is inside '€' (bytes 2..5 of string)
exit=101
```

Bir düzenleyici bozuk/yarım bir dosya kaydettiğinde **dil sunucusu çöker**.

**Düzeltme:** `rest.get(..4)` + `is_char_boundary` kontrolü, veya `rest.chars().take(4)`.

### P0-15 · `vey build -o <klasör>` hedef klasörü **`remove_dir_all`** ile siliyor  ✅ **düzeltildi**
`src/cli/site.rs:94-98`

Çıktı klasörü **tümüyle** siliniyor. Kullanıcı yanlış `-o` verirse (örn. `vey build -o ../notlar`) o klasördeki **tüm kullanıcı dosyaları** silinir. ROADMAP 14.1'de "kaynakları tutan bir klasör reddedilir" düzeltilmiş, ama **genel bir klasör** hâlâ siliniyor.

**Düzeltme:** (a) önce "bu klasörde Vey'in ürettiği dosyalar var mı?" diye bak (manifest tut), (b) yalnızca **kendi ürettiği** dosyaları sil, (c) boş değilse onay iste.

### P0-16 · `vey lint` ve `vey fmt` **her zaman exit 0** döner → CI'da işe yaramaz  ✅ **düzeltildi**
`src/cli/lint.rs:107` · `src/cli/fmt.rs:77`

```rust
if found == 0 { println!("no smells were found"); }
else { println!("{} smell(s) were found", found); }
0                                     ← daima başarı
```

**Doğrulandı:**
```
$ vey lint e.vey
e.vey:2:1: the line is 132 characters long, and 100 is the most that is easy to read
1 smell(s) were found
$ echo $?
0
```

Bir CI iş akışında `vey lint && vey build` yazıyorsa, **biçim ihlali olan kod derlenir**.

### P0-17 · Site build'i **tüm derleyici uyarılarını düşürüyor**  ✅ **düzeltildi**
`src/cli/site.rs` (`compile_for_site` — `bundle.warnings`'a hiç dokunmuyor)

| Komut | Uyarılar |
|---|---|
| `vey build --node` | ✅ basılıyor (`src/cli/build.rs:136`) |
| `vey build` (site) | ❌ **sessiz** |
| `vey dev` / `vey run` | ❌ **sessiz** |

Aynı program için iki farklı davranış — kullanıcı "neden `vey build` uyarı vermiyor?" diye soruyor.

### P0-18 · `vey explain` çok modüllü dosyalarda **import tiplerini çözemiyor**  ✅ **düzeltildi**
`src/cli/explain.rs:65` → `types::explain` → `types::explain.rs:30` (`TypeChecker::for_explain`) → **import env'si kurulmuyor**.

**Doğrulandı:**
```
$ vey explain main.vey:4:11     # m.greet("a"), m = use "./mod1.vey"
main.vey:4:11: any                ← doğru tip string olmalı
```

`compiler::check_file` (`src/compiler/mod.rs:260-264`) import env'lerini kuruyor ama `explain` yolu kurmuyor.

### P0-19 · `onMount(...)` bileşen markup'ında **metin düğümü olarak render** ediliyor 🔴  ✅ **düzeltildi**

Bu bulgu kod okurken değil, **çalıştırılarak** bulundu.

`src/parser/markup.rs:34-58` — markup'taki her ifade `MarkupNode::Expr` olur ve `src/codegen/markup.rs:21` bunu **çocuk listesine** koyar:

```js
// Üretilen kod
function _App(...) {
  return [V.onMount((function() { ... })), V.__h("div", null, ["x"])];
}
```

`V.onMount` **bir unsubscribe fonksiyonu** döndürür (`src/runtime/events.js:60-62`) ve o değer markup'ın çocuğu olarak sayılır → `toDom` onu **metin düğümüne** çevirir (`src/runtime/dom.js:223`).

**Doğrulandı:**
```
$ cat onmount3.vey
let count = signal(0)
let mounts = 0
component App() {
  onMount(fn() { mounts = mounts + 1 })
  div { "{count.value}" }
}
mount("#app", App)

$ node harness.cjs
MOUNTS=2
TEXT: "function () {\n      var index = mountQueue.indexOf(fn); … }1"
```

Sayfaya **onMount'un unsubscribe fonksiyonunun kaynak kodu** yazıldı.

**Aynı sorun `onDestroy` ve `print(...)` gibi her ifade için geçerli:**
```vey
component App() {
  print("hello")     // → markup'a "undefined" metin düğümü olarak giriyor
  div { "x" }
}
```

**Düzeltme:** Markup'ta "yan etki" ifadeleri (call ifadesi) çocuk listesine **girilmemeli**. Ya da `onMount`/`onDestroy` gibi bilinen yan etki yardımcıları özel olarak ele alınmalı (marker + `V.__effect(...)`).

---

## 4. A — Dil çekirdeği ve derleyici

### A1 · `Promise<T>` bir async fonksiyonun dönüş tipi olarak **kullanılamıyor** 🔴  ✅ **düzeltildi**

> **Durum (22 Ekim 2026):** düzeltildi. `fn load() -> Promise<number> { await sleep(10); return 1 }` ve `let y: string = await load()` ikisi de artık kontrol ediliyor (ikinci satır "expected string, found number" diyor). `await` dönüş tipini `Promise`'ın içinden okuyor. Aşağıdaki kanıt ilk incelemenin fotoğrafıdır.

**Kanıt (çalıştırıldı):**

```vey
fn load() -> Promise<number> { await sleep(10); return 1 }
```
```
error: expected Promise<number>, found number
```

```vey
fn load() { await sleep(10); return 1 }   // dönüş tipi yazılmamış
let y: string = await load()                // ← HİÇ HATA VERMİYOR
```
```
ok: 1 page(s) checked
```

**Neden:**
- `src/types/stmts.rs:822` — `Ok((expected.clone(), param_types))`. Bildirilen dönüş tipi **doğrudan** kullanılıyor, `Promise<T>` ile sarılmıyor.
- Async fonksiyon çalışma zamanında JS `async function` oluyor (`src/codegen/mod.rs:261-262`), yani **her zaman Promise döndürür**.
- `src/types/core.rs:45` → `Type::Promise(Box<Type>)` var ama otomatik olarak **hiç üretilmiyor**.
- `src/types/builtins.rs:61` → `let promise = || Type::Any;` → `sleep`, `http.get`, `http.post`, `http.fetch`, `fetch`, `all`, `race` **hepsi `any`**.

**Sonuç:** Async kod tamamen tipsiz. Bu, "olgun bir dil" hedefi için **1 numaralı engeldir**. `Result<T,E>` de aynı şekilde: `ok()`/`err()` `any` dönüyor (`builtins.rs:209-210`), `Result` yalnızca elle yazılabiliyor.

**Önerilen çözüm (Bölüm 9-B1).**

### A2 · Blok gövdeli fonksiyon örtük dönüş yapmıyor

`src/codegen/exprs.rs:346-365` — `ArrowBody::Block` için `return` üretilmiyor.

```vey
fn(params) { UserPage(params) }     // undefined
fn(params) { return UserPage(params) }  // doğru
```

Bu, JS/Swift/Rust'taki `=>` beklentisiyle çelişir ve Bölüm 2'deki kırık testin doğrudan sebebi. README'de örtük dönüş **belgesiz** — sessiz bir davranış.

**Öneri (üçünden biri):**
1. Blok gövdenin son ifadesini örtük döndür (Rust/Swift davranışı) — en büyük kazanç, en büyük uyumluluk riski.
2. Blok gövdeli arrow'da son ifade "değer üretmiyor" sayılıp **uyarı** ver: `warning: this arrow does not return a value; add 'return'`.
3. Olduğu gibi bırak ama README'ye açıkça yaz.

Tavsiye: **(2) + (1)'in geçiş planı** — önce uyarı, sonra sürüm notuyla örtük dönüş.

### A3 · `slot` mekanizması yarım uygulanmış

Katmanlar arasında **anlaşmazlık** var:

| Katman | Ne yaptığı |
|---|---|
| Parser (`parser/markup.rs:229`) | `slot="x"` yalnızca **bileşen çağrısının çocuklarında** slot olarak yakalanıyor (`in_component` = çağıran bileşenin çocuğu mu) |
| Parser (`parser/mod.rs:482`) | Bileşenin **kendi gövdesi** `in_component = false` ile ayrıştırılıyor → `div slot="content"` sadece normal HTML attribute |
| Codegen (`codegen/markup.rs:199-214`) | Slot içeriği, slot adıyla **prop** olarak gönderiliyor |
| Checker (`types/stmts.rs:988-995`) | Slot adının bileşenin **parametresi** olmasını şart koşuyor |

**Doğrulandı:**
```vey
component Panel(title) {
  div {
    div slot="content" { "the content" }   // ← panel bunu slot olarak DEĞİL, attribute olarak görüyor
  }
}
component App() {
  Panel("T") { p slot="content" { "given" } }
}
```
```
error: component 'Panel' has no slot named 'content'
```

Yani **çağıran** slot işaretliyor, **çağıran taraf** (bileşenin kendisi) slot'u **tanımlayamıyor**. Pratikte "slot" = "adlandırılmış prop". Eksik olanlar:
- Slot'un varsayılan (fallback) içeriği — çağıran hiçbir şey vermezse ne olur? (bugün: boş)
- Slot içeriğinin tipi (`any`).
- Slot içeriğinin **tip denetimi** ve generic slot desteği.
- Aynı slot adına iki çocuk → bugün hata veriyor (`types/stmts.rs:977`), ama "ilk olan kaybolur" yerine düzgün bir hata — bu kısım doğru.

### A4 · SVG ve HTML dışı etiketler **yazılamıyor** 🔴  🟡 **yarısı düzeldi**

> **Durum (22 Ekim 2026):** `svg`, `path`, `circle` ve diğer SVG etiketleri artık **yazılabiliyor ve çizilebiliyor** (`html(App())` → `<svg viewBox="0 0 24 24"><circle ...></circle></svg>`); `slot` de var. **Web component etiketleri (`my-element`) hâlâ yok** — `unknown tag` diyor. Kanıt aşağıda ilk incelemeye ait.

`src/parser/html.rs:3-102` `HTML_TAGS` listesinde `svg`, `path`, `circle`, `rect`, `g`, `defs`, `use`, `symbol`, `mask`, `clipPath`, `filter`, `linearGradient`, `polygon`, `polyline`, `ellipse`, `text`, `foreignObject`, **`slot`** yok.

`src/parser/markup.rs:89-112` `is_element_start` → HTML olmayan bir etiket `attr_ahead()` ile başlıyorsa kabul edilir; ama `attr_ahead` `=` görmüyorsa reddedilir.

**Doğrulandı:**
```vey
component App() { svg viewBox="0 0 24 24" { path d="M0 0h24v24H0z" } }
```
```
error: unknown tag 'svg'
```

Ayrıca `<my-element>` gibi **web component** etiketleri de yazılamıyor (`parser/markup.rs:98-110`).

**Bu, "web dilinin" en görünür eksiği.** Bir çizim, bir ikon, bir SVG sprite — hiçbiri yazılamıyor.

### A5 · Nokta içeren öznitelik adı yok

`src/parser/markup.rs:297-305` — `parse_attr_name` yalnızca `-` kabul ediyor.

**Doğrulandı:** `div xml:lang="en"` → `error: unexpected token ':'`

Eksik: `xlink:href`, `xml:lang`, `v-bind:x`, `@click`, `:prop` (Vue tarzı kısayollar).

### A6 · Markup içinde `let` yok → bileşen kendi durumunu tutamıyor 🔴

`src/parser/markup.rs:34-58` — `parse_markup_nodes_inner` yalnızca `if` / `for` / `match` / element / ifade kabul ediyor.

**Doğrulandı:**
```vey
component Counter() {
  let count = signal(0)
  button click=fn() { count.set(count.value + 1) } { "{count.value}" }
}
```
```
error: unexpected token 'let'
```

**Sonuç:** Her bileşenin durumu **modül seviyesinde** yaşamak zorunda → bileşen yeniden kullanılamaz, iki `Counter` aynı sayacı paylaşır. `examples/render.vey` ve `examples/page.vey` bu yüzden `count` değişkenini dışarıda tutup `bind("#app", App())` ile manuel yeniden çizim yapıyor — **SSR ile hidrasyon arasındaki farkın kaynağı bu**.

### A7 · Spread prop yok

`src/ast/markup.rs:21` — `Element.attrs: Vec<(String, Expr)>`, parser `...` kabul etmiyor. `{...props}` yazılamıyor.

### A8 · Kaynak haritasında isim yok

`src/sourcemap/mod.rs:158` → `"names":[]`. Değişken adları stack trace'te görünmüyor.

### A9 · Parser derinlik sınırı 64

`src/parser/mod.rs:39` — `MAX_DEPTH: usize = 64`. Derin iç içe ifade reddediliyor (koruma iyi, ama sınır düşük; 256'ya çıkarılabilir).

### A10 · `for ... by` yalnızca `range` ile çalışıyor

`src/codegen/mod.rs:485-495` — "a loop with a step must walk a range". `for x in list by 2` yazılamıyor.

### A11 · `match` kolunda `yield` → arm'lar ayrı fonksiyonda çalışamıyor

`src/codegen/exprs.rs:535-544` — `yield` varsa `match` bir generator fonksiyonuna dönüşüyor; ama arm gövdeleri başka bir fonksiyonda yazıldığı için `yield` geçersiz. Checker reddediyor, ama mesaj açıklayıcı değil.

### A12 · `impl` metotları her örneğe **kopyalanıyor** (prototip yok)

`src/codegen/mod.rs:639-676` — `__veySelf.url = function ...` → her `Config(...)` çağrısında yeni fonksiyon nesneleri. Bellek ve `this` kaybı. `Object.create` + prototip zinciri kullanılmalı.

---

## 5. B — Tip sistemi

### B0 · Genel durum

İki ayrı kontrolcü var:
- **Statik kontrolcü** (`src/check/mod.rs`) — isim kapsamları, kullanılmayan isim, "undefined variable". **Tiple ilgisi yok.**
- **Tip denetleyici** (`src/types/*`) — kademeli (gradual) tip kontrolü, `any`/`unknown` kaçış kapıları, daraltma (narrowing), jenerik çıkarım.

`Type` enum'unda 28 varyant var (`src/types/core.rs:6-49`): `Any, Unknown, Var, Number, String, Bool, Null, Undefined, Void, Never, Symbol, BigInt, Array, Tuple, Dict, Object, AnyObject, Interface, Function, Union, Enum, EnumUnion, EnumNamespace, Recursion, Named, Promise, Result`.

**Var olan güçlü yanlar:** daraltma (narrowing) oldukça gelişmiş (`== null`, `!= null`, doğruluk, `is`, `instanceof`, `type(x)=="kind"`, üye yolu bazlı), modüller arası imza aktarımı çalışıyor, enum payload alanları doğru bağlanıyor, jenerik çağrı yerinde çıkarımı çalışıyor, özyinelemeli tip alias'lar çalışıyor.

**Eksik olan iki grup:** (a) modern ileri tip özellikleri, (b) **çıkarım kalitesi**. (b) daha acil ve daha ucuz.

### B1 · Async tip güvenliği yok (A1'in tip tarafı)

`Promise<T>` `src/types/core.rs:45`'te var; `src/types/rules.rs:241-247` uyumluluk kuralı da var. Ama:
- Otomatik üretim yok (A1).
- `Promise<T>` **elle** yazılsa bile fonksiyon dönüş tipi olarak reddediliyor.
- `src/types/builtins.rs:61` → tüm async helper'lar `any`.

**Sonuç:** `await` operatörü `any` üretiyor; `await` sonrası narrowing çalışmıyor.

### B2 · `Result<T,E>` de tam çalışmıyor

`ok()` / `err()` `any` dönüyor (`builtins.rs:209-210`). `?` operatörü `src/types/exprs.rs:427` → `Type::Result(ok, _) => *ok` → **hata tipi (`E`) tamamen düşüyor**. `catch` bloğundaki hata değeri `Type::Any` (`src/types/stmts.rs:471`).

**Doğrulandı:**
```vey
fn read() -> Result<number, string> { return err("bad") }
fn use2() -> Result<number, string> {
  let v = read()?      // ok
  return ok(v)
}
```
Hata tipi taşınmadığı için `use2` de `Result<number, string>` görünüyor, ama içeride ne olursa olsun hata türü **hiçbir yerde denetlenmiyor**.

### B3 · Tip parametresi **her şeyi kabul ediyor** → variance diye bir şey yok, hatta **güvensiz**
`src/types/rules.rs:172-175`

```rust
// A type parameter is a name for a type that is not known yet, so it
// takes anything and is taken by anything.
(Type::Var(_), _) | (_, Type::Var(_)) => true,
```

Sonuç: `Box<Dog>` → `Box<Animal>` kabul ediliyor, `[Dog]` → `[Animal]` kabul ediliyor (`rules.rs:176` eleman bazında covariant). `assignable` bir kural değil, **"ya hep ya hiç"** kuralı.

**Doğrulama:**
```vey
fn f(a: [number], b: [string]) { }
// ikisi de any sayılıyor → f([1], ["a"]) hata vermiyor
```

### B4 · Jenerik fonksiyon **monomorfize** ediliyor → açık tip argümanı düşürülüyor 🔴  ✅ **düzeltildi**

> **Durum (22 Ekim 2026):** düzeltildi. `fn parse<T>(text: string) -> T { return num(text) }` için `parse<number>("42")` artık **dikkate alınıyor**; `parse<string>("42")` ise `type argument 'T' expects number, found string` diyor (bağlı tip parametresi bir kurala dönüşmüş, elle yazılan argüman yok sayılmıyor). Aşağıdaki kanıt ilk incelemenin fotoğrafıdır.

`src/types/stmts.rs:232-237`

```rust
// A type parameter that was settled by a returned value is not
// generic any more, so the function has that type from then on.
let bindings = self.type_param_bindings.last().cloned().unwrap_or_default();
if !bindings.is_empty() { ty = substitute_bound(&ty, &bindings); }
```

`src/types/exprs.rs:893-896` → monomorfize olmuş bir fonksiyona verilen **açık tip argümanı sessizce düşürülüyor** (`free` boş olduğu için `subst`'e yazılmıyor).

**Doğrulandı:**
```vey
fn parse<T>(text: string) -> T { return num(text) }
let n: string = parse("42")
```
```
error: expected string, found number       ← "cannot infer T" değil; T zaten number'a bağlanmış
```

```vey
fn parse<T>(text: string) -> T { return num(text) }
let n: string = parse<string>("42")
```
```
error: expected string, found number       ← parse<string> DİKKAT EDİLMEDİ!
```

Yani **açıkça yazılan tip argümanı yok sayılıyor** ve fonksiyon somut bir tipe bağlanıyor. `identity<T>(v: T) -> T` jenerik kalıyor (çünkü `unify_types(Var(T), Var(T))` subst'e yazmıyor — `rules.rs:658-661`), ama `parse` gibi bir tip çıkarımı dönüşten gelen fonksiyonlar **jenerikliğini kaybediyor**.

### B5 · Union türü `match` ile **asla tükenmez** sayılmıyor
`src/types/exprs.rs:368-380`

```rust
Type::Union(items) => {
    let parts: Vec<String> = items.iter().map(|item| item.display()).collect();
    return Err(self.error(subject.pos(),
        format!("match on '{}' is not exhaustive: a name or a _ is needed", parts.join(" | "))));
}
```

**Doğrulandı:**
```vey
fn f(x: number | string) -> string {
  return match x {
    1 => "one"
    "a" => "a"      // ← her iki üye de kapsandığı halde hata!
  }
}
```
```
error: match on 'number | string' is not exhaustive: a name or a _ is needed
```

Ayrıca:
- `Type::Tuple` özne: `match` içinde **hiçbir kolu yok** → tükenmezlik **hiç** denetlenmiyor (`exprs.rs:381`).
- `Pattern::List` / `Pattern::Object` / `Pattern::Range` desenleri tükenmezlik hesabına **hiç katılmıyor** (`exprs.rs:294-299`).
- `Type::Number` / `Type::String` özne: tükenmezlik denetimi yok → `match n: number { 1 => ... }` sessizce kabul ediliyor.
- Enum kapsayıcılığı **çalışıyor** (iyi).

### B6 · Discriminated union ve enum birliği daraltma **yok**
- `x is Color.Red` **yok** — `is` yalnızca builtin kind'lere izin veriyor (`src/types/exprs.rs:460-466`).
- `Type::Union` içindeki enum birliği daraltılmıyor.
- `Type::EnumUnion` daraltması yalnızca `match` kolları üzerinden çalışıyor (`exprs.rs:240-312`).
- `covered` kümesi **string** anahtarla tutuluyor (`format!("{}.{}", enum_name, variant)`), `Type` değil → tip güvenliği yok.

### B7 · `is` ile kullanılabilen tipler `is_builtin_type` ile **çelişiyor** ✅ **düzeltildi**
`src/types/exprs.rs:1700-1711` (yalnızca `number,int,string,bool,boolean,null,undefined,object,function`)
↔ `src/types/builtins.rs:473-489` (`any, unknown, number, string, bool, null, undefined, void, never, symbol, bigint, object`)

Sonuç: `unknown`, `any`, `never`, `void`, `symbol`, `bigint` `is` ile kullanılamıyor, ama builtin tip sayılıyorlar.

**Düzeltme:** `is_builtin_kind` artık `is_builtin_type`'a soruyor (tek liste), yalnızca `int`/`boolean`/`function` ek olarak kabul ediliyor. Runtime `is()` `any`/`unknown` (her zaman doğru), `never` (hiç), `void` (undefined), `symbol`, `bigint` kind'larını biliyor. Tipi bilinmeyen bir değerde (`unknown`/`any`/değişken) kontrol serbest; iki ayrık ilkel tür arasında (`number` üzerinde `is string`) gibi bir kontrol **hata** veriyor. Test: `tests/types.rs::a_value_of_an_unknown_type_is_narrowed_by_is`.

### B8 · Callback'lerin **bağlamsal (contextual) tipi yok** — çift yönlü çıkarım altyapısı eksik
`src/types/exprs.rs:715-769` — `infer_expr_as` yalnızca 3 şekli ele alıyor (`Call`, `Array`, `Object`); `Expr::Arrow` / `Expr::Fn` için beklenen tip **hiç** kullanılmıyor → arrow'ın parametreleri `any` oluyor (`checker.rs:791-802`).

Tek istisna: operatör kullanımından gelen zayıf çıkarım (`note_operand`/`note_param`, `exprs.rs:481-493`) — yalnızca düz `Ident` operandı için.

**Sonuç:**
```vey
fn each<T>(items: [T], f: (T) -> void) { }
each(["a"], fn(x) { x.length })   // x: any → x.length: any
```

### B9 · `let` / `const` **nullability'yi kaybediyor** ✅ **yarısı düzeltildi**
`src/types/stmts.rs:163-176`

```rust
} else if decl.kind == VarKind::Const {
    let ty = match &init_ty { Some(ty @ (Type::Number | Type::String | Type::Bool)) => ty.clone(), _ => Type::Any };
    ...
} else { self.declare_pattern_typed(&decl.pattern, &Type::Any); }
```

- Annotation'sız **`let` → daima `any`** (`stmts.rs:66`).
- Annotation'sız `const` → yalnızca number/string/bool korunuyor, gerisi `any`. **Düzeltildi:** `const` artık number/string/bool/null/union/`Named`/`Interface`/`enum`/`Promise`/`Result` tiplerini koruyor; `const x = findUser()` → `User | null`. Nesne ve liste bilerek dinamik kalıyor, çünkü alanlarına sonradan yazılabiliyor (`const obj = { a: 1 }; obj.b = 2` çalışmaya devam ediyor). Test: `tests/types.rs::a_constant_of_a_value_that_may_be_nothing_keeps_that`.
- `let` hâlâ `any`: dil "dinamik" diyor ve atama serbest. Karar belgelendi (bkz. Faz 1, 9).
- Döngü/şube birleşiminde daraltma **birleştirilmiyor** (union yerine overwrite).
- `forget_member` (`checker.rs:678-686`) yalnızca **doğrudan** atamada daraltmayı kaldırıyor.

### B10 · `any` **iki yönde de** şeffaf — 126 kullanım
`src/types/rules.rs:141-145`

```rust
if matches!(source, Type::Any | Type::Recursion(_))
    || matches!(target, Type::Any | Type::Recursion(_))
{
    return true;
}
```

`any` üreten ana noktalar:

| Yer | Anlam |
|---|---|
| `stmts.rs:66` | annotation'sız `let` → `any` |
| `stmts.rs:169,175` | annotation'sız `const` → `any` |
| `stmts.rs:79,308` | component → `any` |
| `stmts.rs:485` | modül import'u → `any` |
| `checker.rs:711` | modül aliası (değer) → `any` |
| `checker.rs:532,794,875` | annotation'sız parametre/dönüş → `any` |
| `exprs.rs:47` | **bilinmeyen isim → `any`** (hata değil!) |
| `exprs.rs:411` | `await` non-Promise → `any` |
| `exprs.rs:1262` | tuple dizin → `any` |
| `exprs.rs:1290` | enum alanının tipi yok → `any` |
| `exprs.rs:1546` | union üyesinde alan yok → **`any` (asıl hata kayboluyor)** |
| `exprs.rs:1563` | **`Type::Dict` nokta erişimi → `any`** |
| `core.rs:309-324` | `project_field` alan yok → `any` |
| `rules.rs:804,813` | bağlanmamış tip değişkeni → `any` |
| `rules.rs:393` | `erase_type_vars` → `any` |
| `types/builtins.rs` | ~90 `any()` |

**`Type::Unknown` pratikte ölü** — 5 kullanım, yalnızca `resolve.rs:56` üretiyor. `unknown` bir interface'e de atanamıyor (`rules.rs` içinde `(Unknown, Interface)` kolu yok → `_ => false`).

**Doğrulandı:**
```vey
interface P { name: string }
fn f(p: P) -> string { return p.name }
fn g(u: unknown) -> string { return f(u) }
```
```
error: argument 1 expects P, found unknown
```

### B11 · Sözlük (`Dict`) erişimi **üç farklı** davranıyor
`src/types/core.rs:26` → `Type::Dict(key, value)` var, ama:

| Erişim | Sonuç | Kanıt |
|---|---|---|
| `d["foo"]` | value tipi ✅ | `exprs.rs:1262` |
| `{foo} = d` (desen) | value tipi ✅ | `core.rs:320-321` |
| **`d.foo`** | **`any`** ❌ | `exprs.rs:1563` — `lookup_member`'da `Dict` kolu **yok** → `_ => Ok(Type::Any)` |
| `for k in dict` | **anahtar** tipi | `core.rs:281-282` |
| `dict.keys()` | **`list(string)`** | `builtins.rs:159` → farklı tip! |

Ayrıca **anahtar tipi hiçbir yerde denetlenmiyor**: `d[true]` (`{[string]: number}` üzerinde) hatasız.

### B12 · Excess property kontrolü hedefe göre **tutarsız** ✅ **düzeltildi**
`src/types/rules.rs`

| Çift | Kural |
|---|---|
| `Object → Object` | *exact* (`:199-201`) |
| `Object → Named` | *exact* (`:205-207`) |
| **`Object → Interface`** | ~~exact değil~~ → artık **exact** |
| `Named → Object` | exact değil (`:202-204`) — bir değerin kendi tipi vardır, fazladan alan taşıyabilir |

`extra_field` (`helpers.rs`) artık `(Object, Interface)` çiftini de tanıyor → hata alanı **adıyla** söylüyor ("'y' is not a field of Point"). Çağrı argümanlarında da aynı kural: parametre jenerik değilse `unify` başarısı `fits` kontrolünü atlamıyor. Test: `tests/types.rs::a_field_that_an_interface_does_not_have_is_named`.

### B13 · `null` sıralanabilir ama karşılaştırılamaz — tutarsızlık
`src/types/rules.rs:62` → `Null` sıralanabilir; `rules.rs:28-30` → `comparable(Null, Number)` = false.

**Doğrulandı:** `a: number | null` için `a < 5` → **ok**. `a == 5` → hata.

### B14 · `as` bir **cast**'tir — `satisfies` karşılığı yok
`src/types/exprs.rs:447-456`

```rust
if !self.fits(&actual, &wanted) && !self.fits(&wanted, &actual) {
    return Err(self.type_error(*pos, &actual, &wanted));
}
Ok(wanted)          // ← çıkarılan tip DEĞİŞTİRİLİYOR
```

`as` **çift yönlü assignable** kontrollü cast'tir ve sonucu **hedef tipe** dönüştürür. `satisfies` (kontrol et, tipi koru) karşılığı **yok**.

### B15 · `unknown` / `never` / `symbol` / `bigint` pratikte yok
- `unknown` yalnızca yazılabiliyor, çıkarılamıyor.
- `never` yalnızca `union()`'ın boş sonucu + `error`/`todo`/`unimplemented`. **Erişilemez kod çıkarımı yok**: `throw` sonrası kodun `never` olması beklenir ama çıkarılmıyor (`stmts.rs:402-405`).
- `symbol` / `bigint` `Type`'te var ama hiçbir yerden üretilmiyor.

### B16 · `Type::Interface` **tip argümanlarını taşımıyor**
`src/types/resolve.rs:388` → `Type::Interface(name.to_string(), fields)` — `Box<number>` ile `Box<string>` **aynı** görünür ve yapısal olarak da örtüşür.

### B17 · Generic çağrı yerinde çıkarım — **kısmi**
- Açık tip argümanı sayısı **birebir** olmalı (`exprs.rs:863-873`) → kısmi çıkarım yok.
- Argümanlar beklenen tipe göre birleştirilir, beklenen tip **sonradan** (`exprs.rs:899-950`) → `unify_types` `Var`'ı bağladığı için beklenen tip geri güncelleyemiyor.
- Çıkarılamayan tip parametresi hata veriyor (`exprs.rs:972-983` "cannot infer the type of T") ama `erase_type_vars` (`rules.rs:391-457`) serbest kalan her `Var`'ı sessizce `any`'e çeviriyor.

### B18 · Modül/namespace tipleri — kısmi
`TypeExpr::Qualified` ile tip referansı var, ama:
- Modülün kendisi bir **değer** olarak `Type::Any` (`checker.rs:709-713`).
- `ModuleTypeEnv` (`checker.rs:28-55`) düz `HashMap<String, Type>` → namespace hiyerarşisi yok.
- `for_explain` `imports: HashMap::new()` ile kuruluyor (`checker.rs:563`) → **`vey explain` modül tipine erişemiyor** (P0-18).
- ✅ **Bağımlı modülün tip hatası sessizce yutuluyor** — **düzeltildi**: `type_environments` hatada durmuyor; bozuk modül `unchecked` bir ortamla yerine konuyor (üyeleri denetlenmesin diye), döngü devam ediyor ve **bütün** hatalar toplanıyor (`FileProblems.errors`). LSP her tanıyı **kendi dosyasının** URI'si altında yayınlıyor. Test: `tests/cli.rs::the_language_server_says_the_mistakes_of_a_used_module`.

### B19 · İleri tip özellikleri — hiçbiri yok
Higher-kinded types · variance · associated types · `typeof` · mapped/conditional types · type-level operatörler · recursive type guards · refinement types · branded types · type class / trait · literal types · `as const` · template literal types · `satisfies` · index signature okuma/yazma · `readonly` · variadic tuple · `infer` · `keyof`.

**Doğrulama:**
- `Type::Var(String)` yalnızca birinci derece tip değişkenidir; tip yapıcıları **eager substitution** ile açılır (`resolve.rs:244`) → `Box<number>` = `Named("Box", [value: number])`, yapıcı kimliği kaybolur.
- `impl` bloğu yalnızca **alias**'a bağlanabiliyor (`checker.rs:409-416`), metotlar jenerik olamıyor, `type Item = ...` ilişkili tip bildirimi yok.
- `as const` **sözdizimi olarak hiç yok** (`grep "as const"` → 0 sonuç).

### B20 · Hata mesajı kalitesi — orta
**Güçlü:** `did_you_mean` ile "unknown type" (`resolve.rs:112-116`), modül üyesi (`exprs.rs:1196-1206`), enum varyantı (`exprs.rs:1358-1379`), eksik alan → "use upper(...) instead" (`exprs.rs:1682-1691`), fazla alan → "'x' is not a field of T" (`stmts.rs:138-143`).

**Zayıf:**
- Union tükenmezliği eksik kolu **listelemiyor**: "match on 'number | string' is not exhaustive: a name or a _ is needed" → hangi kol eksik belirtilmiyor.
- `type_error` (`checker.rs:593-602`) yalnızca "expected X, found Y" → **hangi yolun** suçlu olduğu yok.
- `check_call_named` argüman hatası parametre **adı** vermiyor ("argument 2 expects number, found string") — adlar `FunctionType.param_names`'te mevcut ama kullanılmıyor.
- `lookup_member` `Type::Union` kolunda bir üyede alan yoksa **sessizce `any`** dönüyor (`exprs.rs:1546`) → asıl hata kayboluyor.
- `is`/`as` hataları program tipi ile runtime kind'i karıştırıyor: "a type of the program cannot be checked with `is`".

### B21 · `vey explain` kapsamı — kısmi
`src/types/explain.rs:12-38` → yalnızca tek satırlık `ty.display()`.

- **İfadelerin tipini** verir, **bağlantıların tipini vermez**. `Walker` "en son başlayan ifade"yi seçiyor (`explain.rs:204-219`) → **ilgisiz bir ifadenin** tipi gelebiliyor. Gözlem: `vey explain examples/typed.vey:45:5` → `Node | null` (o satır `let ada: User = ...` ama cevap 40 satır önceki `sum` fonksiyonundan geliyor).
- `display()` (`core.rs:142-211`) basit: `Interface`/`Named` **tip argümanlarını göstermiyor** (`Box<number>` → `Box`), constraint'ler hiç görünmüyor, `Type::EnumUnion` varyant listesini gizliyor.
- `for_explain` yalnızca `predeclare_top_level` yapıyor (`checker.rs:580`) → fonksiyon dışındaki yerel `let`'ler `any`.
- `--json` çıktısı yok.

### B22 · `check/mod.rs` ↔ `types/*` **mükerrer mantık**

Statik kontrolcü ve tip denetleyici **aynı AST'yi bağımsız olarak iki kez tam dolaşıyor**:

| Yapı | `src/check/mod.rs` | `src/types/*` |
|---|---|---|
| Program dolaşımı | `check_program` (`:173-274`) | `check_program` (types/stmts.rs:22-34) + `predeclare_top_level` (`:36-117`) |
| İfade | `check_expr` (`:624-766`) — 140 satır | `infer_expr` (types/exprs.rs:28-475) — 447 satır |
| İfade (devam) | — | `infer_expr_as` (types/exprs.rs:715-769) — **ikinci bir ifade dolaşımı** |
| Deyim | `check_stmt` (`:276-440`) | `check_stmt` (types/stmts.rs:322-507) |
| Desen | `check_pattern` (`:768-791`) | `check_pattern` (types/patterns.rs:151-178) |
| Fonksiyon | `check_function` (`:487-509`) | `check_function` (types/stmts.rs:716-828) + `check_arrow` (`:851-876`) |
| Component | `check_component` (`:531-543`) | `check_component` (types/stmts.rs:305-320) |
| Markup | `check_markup_nodes` (`:545-595`) | `check_markup` (types/stmts.rs:878-1077) |
| impl | `check_impl` (`:511-529`) | `check_impl` (types/stmts.rs:279-303) |
| **Duplicate type adı** | satır bilgisi **var** | satır bilgisi **yok** (`checker.rs:209-372`) |

**Önemli sonuç:** `compiler/mod.rs:140` `check::check(...)` çalıştırır ve hata halinde `?` ile **build'i durdurur**; `types::check` ancak bundan sonra çalışır (`:155`). Dolayısıyla `types/checker.rs` içindeki **duplicate type kontrolleri normal build yolunda ölü** (sadece `check_file` yolunda çalışır).

**Kaçınılan tek doğru paylaşım:** `block_always_exits` `types/helpers.rs`'ten import edilmiş (`check/mod.rs:11`) — iyi bir örnek. Ama `check/mod.rs` kendi "kontrol edilemez kod" mantığını **ayrıca** sahipleniyor (`:463-485`) ve bu, `types/stmts.rs`'teki "must return on every path" mantığıyla **çakışıyor**.

### B23 · Ölü kod ve tutarsızlıklar (tip katmanı)

| Bulgu | Kanıt |
|---|---|
| ~~**`block_always_returns` ve `stmt_always_returns` ölü kod**~~ ✅ **kaldırıldı** | `src/types/helpers.rs` |
| ~~**`TypeChecker.type_param_constraints` yazılıyor ama hiç okunmuyor**~~ ✅ **kaldırıldı** | `src/types/checker.rs` |
| ~~`env.resolved` iki kez atanıyor~~ ✅ **kaldırıldı** | `src/types/checker.rs` |
| ~~`TypeChecker` yapılandırması iki kez yazılmış~~ ✅ **tek yerde** (`build_checker`) | `src/types/checker.rs` |
| "function must return X" mesajı ikinci mesajın alt kümesi | `stmts.rs:806-813` vs `:816-821` → aynı koşul |
| `block_always_exits` yalnızca **son ifadeye** bakar | `helpers.rs:7` → `match`/`try`/`let-else`/`while true` kalıpları hesaba katılmıyor → sonsuz döngüde dönen fonksiyon hata alır |
| Kod okunabilirliği bozuk (biçim artığı) | `exprs.rs:725`, `exprs.rs:1586-1674`, `resolve.rs:392`, `checker.rs:567`, `check/mod.rs:531` |
| ~~`is_builtin_kind` ↔ `is_builtin_type` çelişkisi~~ ✅ düzeltildi | `types/exprs.rs::is_builtin_kind` artık `is_builtin_type`'a soruyor |
| `orderable` ↔ `comparable` tutarsızlığı | `rules.rs:62` vs `rules.rs:28-30` |

### B24 · Özet: tip sistemi öncelik tablosu

| # | Konu | Etki | Zorluk |
|---|---|---|---|
| 1 | `any` → `unknown` geçişi + `unknown` çıkarımı | **Çok yüksek** — bugün neredeyse hiçbir akış denetlenmiyor | Yüksek |
| 2 | Bağlamsal callback tipi + `infer_expr_as`'ı arrow/fn'e uygulama | Yüksek | Orta |
| 3 | `let`/`const` için akış-tipli çıkarım | Yüksek | Orta |
| 4 | Jenerik monomorfizasyon hatası + açık tip argümanının düşürülmesi | **Yüksek** (yanlış tip kabul ediliyor) | Orta |
| 5 | `Type::Dict` üye erişimi + anahtar tipi denetimi | Orta | Düşük |
| 6 | Exhaustiveness: union literal kolları, tuple, string/number kapsamı | Orta | Orta |
| 7 | ~~`is_builtin_kind` ↔ `is_builtin_type` hizası~~ ✅ yapıldı | Orta | Düşük |
| 8 | `?` operatörünün hata tipini taşıması | Orta | Orta |
| 9 | `null` sıralanabilirliği / `comparable` tutarlılığı | Düşük | Çok düşük |
| 10 | `check/mod.rs` ↔ `types/*` mükerrerliğinin azaltılması | Bakım | Yüksek |
| 11 | Ölü kod temizliği | Bakım | Çok düşük |
| 12 | `vey explain`: bağlama göre ifade arama, `imports` bağlama, zengin gösterim | Orta | Orta |
| 13 | Hata mesajı zenginliği | Orta | Düşük |

> **Not:** İleri tip özellikleri (B19) daha "prestijli" görünüyor ama **inferans kalitesi** (1–8) çok daha acil ve daha ucuz — çünkü bugün kullanıcı yazdığı kodun büyük kısmı `any` sayıldığı için denetlenmiyor.

---

## 6. C — Frontend (tarayıcı runtime)

### C1 · Reaktivite toplama (batching) yok ✅ **düzeltildi**
`src/runtime/state.js:12-19`

```js
function setValue(next) {
  if (next === current) return current;
  current = next;
  var list = listeners.slice();
  for (var i = 0; i < list.length; i++) list[i](current);   // senkron, herkes çağrılıyor
  return current;
}
```

10 signal yazılırsa sayfa **10 kez** yeniden çizilir. `mount.js:340-358` yalnızca "çizim sırasında" olan değişiklikleri erteliyor (`drawing` bayrağı) — bu, dışarıdan gelen toplu değişiklikleri yakalamıyor.

**Çözüm (yapıldı):** `later()` kuyruğu `queueMicrotask` ile çalışıyor ve sayfa yazımı `effect(fn, { defer: true })` ile abone oluyor; on değişiklik sayfayı bir kez yazıyor.

### C2 · Signal'da eşitlik politikası yok ✅ **düzeltildi**
`src/runtime/state.js:14` → `if (next === current)` — **referans** eşitliği.

```vey
state.set({ a: 1 })   // her seferinde yeni nesne → her seferinde "değişti"
```

`store` için de aynı (`state.js:63-68`), hatta store'da eşitlik kontrolü **hiç yok** (`setState` doğrudan atıyor).

**Çözüm (yapıldı):** `signal(value, { equals })` / `store(name, initial, { equals })` — varsayılan `Object.is`, `{ equals: "deep" }` derin karşılaştırma, `equals` fonksiyonu kendi kuralı.

### C3 · Context / provide-inject yok ✅ **var** (doğrulandı)
`provide(name, value)` / `inject(name)` runtime'da ve dilde: her bileşen kendi `Map`'ini `ctx.contexts`'e koyuyor (`dom.js:78-88`), `inject` yukarıya doğru arıyor (`dom.js:529-545`). Bulgu güncel değildi. Test: `tests/dom.rs::the_head_the_shared_values_and_the_fallback_are_written`.

### C4 · Memo yok
Her render'da her bileşen yeniden çalışıyor. `computed` var (`state.js:115-128`) ama bileşen bazında değil.

**Neden kolay değil (denendi):** `h()` `doc` varsa **gerçek elementi hemen** yapıyor ve çocuklarını ona ekliyor (`dom.js:110-121`). Bir bileşenin sonucu da DOM düğümleridir; memoize edilen düğümler yeni ağaçta yeni ebeveyne taşınır ve diff eski ebeveynde bulamaz → ağaç bozulur (denendi: düğümler kopyalandı, sıra karıştı). Memo için **iki fazlı yazım** (önce değer ağacı, sonra DOM) gerekir. `memo` bu yüzden **eklenmedi**; yanıltıcı bir "derdi var ama çalışmıyor" yardımcısı bırakmak daha kötü olurdu.

### C5 · Hata sınırı (error boundary) yok ✅ **var** (doğrulandı)
`Boundary fallback=Fallback { ... }` bileşeni altındaki hataları yakalıyor: derleyici çocukları `V.__h(function () { try { ... } catch (err) { V.__fallback(err, fallback) } })` içine alıyor (`src/codegen/markup.rs::gen_boundary`). Test: `tests/dom.rs::the_head_the_shared_values_and_the_fallback_are_written`.

### C6 · Async bileşen / Suspense yok
`src/codegen/mod.rs:319` — async markup'ı olan bileşen `async function` oluyor, ama `mount` bunu `await` etmiyor → `[object Promise]` yazılır. ROADMAP'te "bir bileşen bekleyebilir" maddesi `[ ]` olarak duruyor.

### C7 · Portal yok ✅ **düzeltildi**
`src/runtime/dom.js` `toDom` her şeyi `doc.createElement` ile üretiyordu → sayfa dışına (modal, tooltip, dropdown) düğüm konulamıyordu.

**Çözüm:** `portal(content, target, key?)` — kutunun adı verilirse sayfa yeniden yazıldığında **aynı kutu** kullanılıyor (`ctx.patchChildren`), böylece portalın içindeki alanlar yazdığını kaybetmiyor. Sayfa artık portalı tutmuyorsa `beginPortals`/`endPortals` kutuyu kaldırıyor; `mount().destroy()` `clearPortals()` çağırıyor. Test: `tests/dom.rs::a_portal_is_written_in_another_place_and_follows_the_page` (yazıldığı yer, state'i izlemesi ve kaldırılması).

### C8 · Geçiş (transition) yardımcısı yok ✅ **düzeltildi**
`enter` / `leave` hook'ları vardı (`dom.js`, `mount.js`) ama süre/gecikme/easing yazmanın standart bir yolu yoktu.

**Çözüm:** `transition(duration, { easing, from, to, leave })` → `{ enter, leave }`. Değer önce `from`, bir sonraki karede `to` (veya giden node için `leave`) olarak yazılıyor, süre dolunca `transition` temizleniyor. `enter=`/`leave=` hem fonksiyonu hem `{ enter, leave }` şeklindeki değeri kabul ediyor. Test: `tests/dom.rs::a_node_may_come_in_and_go_out_little_by_little`.

### C9 · `onDestroy` **hiç çalışmıyor** ✅ **düzeltildi**
`src/runtime/events.js:64` → `queueDestroy(fn)` → `src/runtime/mount.js:390` `runDestroys()` **yalnızca** `mount().destroy()` içinde çağrılıyor.

`mount.js:186-214` (node kaldırma) yalnızca `leaving.__veyUnbind()` çağırıyor — **destroy hook'u yok**.

**Sonuç:** Sayfadan kalkan bir bileşenin `onDestroy` fonksiyonu hiç çalışmıyor; sadece tüm sayfa kaldırılınca çalışıyor.

**Düzeltme (yapıldı):** `onMount`/`onDestroy` bileşenin **kendi** işi olarak `__veyWorks` üzerinde tutuluyor; node kaldırıldığında veya değiştirildiğinde `sayGoodbye` çalışıyor.

### C10 · `onMount` her render'da tekrar çalışıyor ✅ **düzeltildi**
`src/runtime/events.js:59-61` → `queueMount(fn)`; `src/runtime/mount.js:314` → `render()` sonrası `flushMounts()`.

Bir bileşen `onMount(() => ...)` çağırıyorsa ve bileşen yeniden çiziliyorsa, `onMount` **her seferinde** yeniden kuyruğa giriyor ve tekrar çalışıyor.

### C11 · Scroll konumu korunmuyor
`mount.js` içinde scroll restore yok.

### C12 · `<head>`, `<title>`, `<meta>`, `<link>` gövdeye düşüyor ✅ **var** (doğrulandı)
`toDom` ve `html` artık `isHeadTag` ile `title`/`meta`/`link`/`base` işaretlerini **head'e** yazıyor (`dom.js::headElement`), aynı işaret ada göre bir kez yaratılıp sonra güncelleniyor (head tekrarla dolmuyor). Test: `tests/dom.rs::the_head_the_shared_values_and_the_fallback_are_written`.

`title("...")` yardımcısı `doc.title`'ı yazıyor (tarayıcı `<title>` elementini kendi kuruyor); markup'taki `title { ... }` ise head elementini yönetiyor.

### C13 · Keyed diff O(n²) ve "her şeyi yeniden kur" maliyeti 🟡 **key eşleştirme düzeltildi**
`src/runtime/mount.js:153-181` — her yeni node için eski node'ların **tamamı** taranıyor.
Ayrıca `render()` (`mount.js:299-315`) her seferinde **yeni bir DOM ağacı kuruyor** (`flatten([tree]).map(toDom)`) ve sonra karşılaştırıyor → 1000 satırlık liste, her değişiklikte 1000 element üretip karşılaştırılıyor.

**Bu tur:** Eski keyed çocuklar tek geçişte `Map` benzeri bir indeksle eşleştiriliyor; key araması artık O(n), kalan maliyet yeni DOM ağacının kurulması ve gerçek LIS diff'inin eksik olmasıdır.

### C14 · Form alanı değeri **property** olarak yazılıyor, attribute olarak değil
`src/runtime/dom.js:159-167` → `el.value = value`. SSR ile gelen `value="..."` attribute'ı hydration sonrası kayboluyor ve form gönderiminde `value` attribute'ı yeniden yazılmıyor.

### C15 · `void` etiketler `br { }` yazımıyla çalışıyor ama `br` de çalışıyor
`src/runtime/dom.js:11-14` `VOID_TAGS` listesi var ve `html()` bunu kullanıyor. Parser'da `br { }` yazımı zorunlu değil — doğrulandı: `div { br { } hr { } img src="x.png" }` → **ok**. (ROADMAP'teki "void tag cannot be written on its own" maddesi **güncel değil**.)

### C16 · SSR → hydration köprüsü eksik
`vey build` statik HTML üretiyor, `mount.js:304` "page of the server is taken over" diyor ama **farkı birleştirmiyor** — sadece `patchChildren` ile üzerine yazıyor. Gerçek hydration (event listener'ları sunucu DOM'una bağlama, `data-vey` işaretleriyle eşleştirme) yok.

### C17 · Router: layout, guard, nested route yok
`src/runtime/router.js:39-45` — ilk eşleşen route kazanır. `route()` (`router.js:98-113`) `remove()` döndürüyor ama:
- Layout / nested route (`.layout`) yok.
- Route guard / loader yok.
- Scroll restoration yok.
- `navigate` hash'i koruyor ama scroll'u sıfırlamıyor.
- Sunucu tarafında `matchRoute` var (`shared.js:396-443`) ama tarayıcı tarafındaki eşleştirme **ayrı bir kopya** kullanıyor (`router.js:33-36` → `ctx.matchRoute`, aslında aynı fonksiyon — iyi).

### C18 · `on`/`delegate` var ama **olay tipi güvenliği** yok
`src/runtime/events.js:26-56` — `on(target, event, handler)` string alıyor. Tip imzası `builtins.rs:356` → `(any, string, any) -> any`.

### C19 · `ref` var ama `signal`/`store` gibi tipi yok
`src/runtime/events.js:71-78` — `ref()` `{value, set}`. `builtins.rs:362` → `any`.

### C20 · Markup'taki **her ifade** çocuk listesine giriyor (P0-19)
`src/codegen/markup.rs:21` — `MarkupNode::Expr` doğrudan `children` dizisine konur.

Bu yüzden `onMount(...)`, `onDestroy(...)`, `print(...)` gibi **yan etki** ifadeleri sayfaya **metin düğümü** olarak yazılır. Ayrıca markup içinde `let` olmadığı için (A6) bir bileşenin kendi durumu olamıyor; durum dışarıda yaşayınca bileşen yeniden kullanılamaz.

**Kanıt:** Bölüm 3, P0-19.

---

## 7. D — Backend (Node runtime ve sunucu)

> Bu bölümün büyük kısmı `src/runtime/server.js` (766 satır), `middleware.js` (109), `session.js` (146), `shared.js` (469) üzerinde satır satır inceleme ve grep ile doğrulandı.

> **Durum notu (22 Ekim 2026):** Bu bölüm ilk incelemenin fotoğrafı; Faz 0'daki 9 güvenlik hatası düzeltildikten sonra **bir kısmı eskidi**. Doğrulananlar: `trustProxy` var (`server.js:151`), `requestTimeout`/`headersTimeout`/`keepAliveTimeout` ayarlanıyor (`server.js:683-691`), `app.stop()` `closeAllConnections()` kullanıyor (`server.js:962-969`), `res.sendStatus` ve `res.vary` var (`shared.js:364,375`). Aşağıdaki maddeler **yeniden denetlenmeden** "eksik" sayılmamalı.

### D1 · `res` nesnesi yetersiz
`src/runtime/shared.js:244-357` — sunduğu: `status, set, get, getHeaders, remove, send, json, end, cookie, onFinish, transform, head, getStatus, isFinished, sendStatus, vary`.

`redirect`, `type`/`contentType`, `location`, `append`, `write`, `writeHead`, `flushHeaders`, `headersSent`, `locals`, `req`, `sendFile` ve `download` artık mevcut. Dosya yardımcıları gerçek stream kullanıyor; genel kullanıcı stream API'si ve upload streaming hâlâ D2 kapsamındaki eksik.

`res.send("merhaba")` → `Content-Type: text/html` ilan ediyor (`shared.js:209-223`) → JSON döndürmek isteyen bir program `res.json()` kullanmak zorunda (var, iyi).

### D2 · Streaming yanıt imkânsız 🟡 **dosya yanıtı düzeltildi**
`res.write()` artık Buffer değerlerini bozmadan aktarabiliyor; `res.sendFile()` gerçek `fs.createReadStream()` kullanıyor ve gövdeyi bellekte biriktirmiyor. Genel kullanıcı stream API'si ve upload streaming hâlâ eksik.

### D3 · HTTPS / HTTP/2 yok
`src/runtime/server.js:124` yalnızca `node:http`. `grep -rn "http2" src/` → 0 sonuç.✅ `trustProxy` ve zaman aşımları artık okunuyor (`server.js:146-151,683-691`); `host`, `cert`/`key`, `maxRequestsPerSocket` hâlâ yok.

### D4 · Timeout yok
`serverInstance.requestTimeout` / `headersTimeout` / `keepAliveTimeout` ayarlanmıyor. Handler başına timeout, `AbortSignal` desteği yok.

### D5 · Graceful shutdown ✅ **SIGTERM/SIGINT eklendi**
`app.stop()` **deadline** alıyor ve `closeAllConnections()` kullanıyor; sinyal listener'ları da server yaşam döngüsüne bağlandı.

`app.onShutdown(fn)` artık SIGTERM ve SIGINT sinyallerinde callback'leri çalıştırıyor, ardından mevcut graceful `app.stop()` akışını başlatıyor. Listener'lar manuel `stop()` sonrasında temizleniyor. Regresyon testi: `tests/runtime.rs::server_runs_shutdown_callbacks_for_sigterm`.

### D6 · `use()` middleware yolu parametre/wildcard desteklemiyor ✅ **düzeltildi**
`src/runtime/server.js:181-186`

```js
function fitsPath(pattern, pathname) {
  if (!pattern) return true;
  if (pattern === "/") return true;
  if (pathname === pattern) return true;
  return pathname.indexOf(pattern + "/") === 0;   // düz prefix
}
```
`app.use("/user/{id}", fn)` ve `app.use("/assets/*", fn)` artık çalışıyor. Regresyon testi: `tests/runtime.rs::middleware_paths_accept_route_parameters_and_wildcards`.

### D7 · Hata middleware'i ✅ **4-arity ve zincir desteği eklendi**
`runtime.errorHandler(handler)` callback'i artık `ctx` üzerine yazmıyor; `server.use()` onu kendi server instance'ına bağlıyor. İki sunucunun eşzamanlı hataları artık handler'larını karıştırmıyor. Test: `tests/runtime.rs::error_handlers_are_isolated_between_server_instances`.

Hata middleware'leri artık `(error, req, res, next)` imzasıyla `next()`/`next(err)` kullanarak sıradaki hata middleware'ine geçebiliyor. Test: `tests/runtime.rs::error_handlers_can_pass_errors_to_the_next_handler`.

### D8 · Statik dosya servisi 🟡 **temel middleware davranışı mevcut**
`src/runtime/server.js:371-373` + `611-613`

```js
if (!found) { notFound(request, info, res); return; }   // sadece route eşleşmezse
```
Statik servis route handler'lardan önce çalışıyor ve dosya yoksa route'a düşüyor; route eşleşti diye statik dosya engellenmiyor. Ayrıca:
- ~~**Range request / 206 yok**~~ ✅ mevcut.
- ~~`fs.readFileSync`~~ ✅ normal dosya yanıtı artık `sendFile`/`createReadStream` üzerinden akıyor.
- `fs.statSync` **iki kez** çağrılıyor (`server.js:242,245`).
- `Cache-Control` isim hash'ine bağlı değil → aynı isimli dosya 1 yıl bayat kalıyor (`server.js:39-45`).
- `maxAge`/`index`/`immutable`/`fallthrough` ayarları yok (`static(dir)` tek argüman).
- ~~Symlink ile dizin dışına kaçış korunmuyor~~ ✅ düzeltildi: statik dosya ve `index.html` hedefleri `realpath` sonrası kök dizine göre denetleniyor. Test: `tests/runtime.rs::static_files_do_not_follow_symlinks_outside_the_root`.

### D9 · Kayıtlı `OPTIONS` route'u **ölü kod**
`src/runtime/server.js:605-610` — `method === "OPTIONS"` her zaman 204 + `Allow` ile dönüyor; `found.route.handler` çağrılmıyor.

### D10 · WebSocket eksikleri
`src/runtime/server.js:389-490` — elle yazılmış RFC6455 framing'i:
- 64-bit uzunluk, text parçaları ve birleştirilmiş `maxFrame` sınırı uygulanıyor; maskesiz client frame'leri reddediliyor (P0-7).
- ~~Binary frame payload'ı UTF-8 metne çevriliyor~~ ✅ binary frame'ler Buffer olarak teslim ve gönderim yapılıyor. Test: `tests/runtime.rs::websocket_binary_frames_stay_binary`.
- ~~Close kodu/reason yok, ping keepalive yok~~ ✅ `socket.close(code, reason)` destekleniyor; `pingInterval` veya `websocketPingInterval` ile keepalive ping gönderiliyor. `permessage-deflate` hâlâ yok. Test: `tests/runtime.rs::websocket_close_carries_code_and_reason`.
- Origin doğrulaması yok, auth hook yok, backpressure yok.
- ~~`sockets` dizisi hiç temizlenmiyor~~ ✅ bağlantı kapanınca listeden çıkarılıyor.

### D11 · SSE eksikleri 🟡 **id/retry/heartbeat eklendi**
`src/runtime/server.js:790-845` — `stream.send(data, id)`, `stream.event(name, data, id)`, `stream.retry(ms)` ve `stream.heartbeat(ms)` artık mevcut; bağlantı kapanınca heartbeat timer'ı temizleniyor. SSE status override, Last-Event-ID ile otomatik resume ve daha gelişmiş backpressure hâlâ eksik. Test: `tests/runtime.rs::sse_stream_supports_ids_retry_and_heartbeats`.

### D12 · Oturum deposu yalnızca bellekte, takılabilir arayüz yok 🟡 **TTL/idle/max cleanup eklendi**
`src/runtime/session.js` artık `session.configure({ ttl, idleTimeout, maxSessions })` destekliyor; erişimde idle zamanı yenileniyor, süresi dolan kayıtlar temizleniyor ve 60 saniyelik unref edilmiş cleanup job'ı çalışıyor. Takılabilir harici store, `flash`/`touch` ve dağıtık oturum desteği hâlâ eksik. Test: `tests/runtime.rs::sessions_expire_and_are_limited_by_configuration`.

### D13 · Sunucu tarafı KV/cache deposu 🟡 **Node memory store eklendi**
`runtime.storage` ve `runtime.sessionStorage` artık Node'da bellek tabanlı `get/set/remove/clear/keys` API'si sağlıyor. `storage.configure({ ttl })` ile değer bazlı TTL ve otomatik erişim temizliği kullanılabiliyor. Harici Redis/SQLite adapter'ı ve dağıtık persistence hâlâ eksik. Test: `tests/runtime.rs::node_storage_provides_a_ttl_key_value_store`.

### D14 · Şema doğrulama / içerik doğrulama 🟡 **opt-in middleware eklendi**
`runtime.validate({ body, query, params })` artık `required`, `type`, `enum`, `min`, `max`, `pattern` ve nested `fields` kurallarıyla isteği doğruluyor; geçersiz istekler 400 dönüyor. Otomatik tip üretimi, ayrıntılı schema standardı ve response validation hâlâ eksik. Test: `tests/runtime.rs::request_validation_checks_body_query_and_params`.

### D15 · `http` istemcisinde `put`/`patch`/`delete`/`head` ✅ **eklendi**
`src/runtime/http.js` — HTTP yardımcıları artık `get`/`post` yanında `put`/`patch`/`delete`/`head` de sunuyor.

**Bu tur:** `put`, `patch`, `delete` ve `head` eklendi; JSON gövdeleri `application/json` ile kodlanıyor. Ayrıca harici `AbortSignal` verildiğinde `timeout` ve `retries` artık sessizce devre dışı kalmıyor. Runtime regresyon testleri eklendi.
Ayrıca `fetch(url, { signal, timeout: 100 })` → kullanıcı `signal` verdiğinde `timeout` **sessizce yutuluyor** (`http.js:249-259`: `if (!opts.signal && ...)`).
`Response` sahte bir nesne: `arrayBuffer()`, `blob()`, `formData()`, `body` stream, `headers.getSetCookie()`, `statusText` eksik (`http.js:140-178`).

### D16 · `fs` API'si 🟡 **async işlemler eklendi**
`fs.readAsync`, `writeAsync`, `appendAsync`, `listAsync`, `stat`, `rename`, `copy`, `realpath` ve `chmod` artık Promise tabanlı Node API'leri olarak mevcut; senkron kısa yollar geriye dönük uyumluluk için korunuyor. `watch`, atomik yazma ve AbortSignal desteği hâlâ eksik. Test: `tests/runtime.rs::async_fs_operations_do_not_require_sync_helpers`.

### D17 · `crypto` ve `zlib` senkron
`src/runtime/node.js:261-358` — `gzipSync`/`gunzipSync` event loop'u bloklar.

### D18 · `env()` sadece `process.env` okuyor
`src/runtime/node.js:11-23` — `.env` dosyası desteği, şema doğrulama, tip'li env, secret yönetimi yok.

### D19 · `path` 🟡 **yardımcılar eklendi**
`relative`, `normalize`, `isAbsolute`, `parse` ve `sep` artık mevcut; browser runtime'ında `path.sep` Node modülü yüklemeden güvenli fallback kullanıyor. Platforma özel `win32`/`posix` namespace'leri hâlâ eksik. Test: `tests/runtime.rs::path_helpers_cover_normalization_and_parsing`.

### D20 · Regex 🟡 **named group ve callback replacement eklendi**
`match()` ve `matchAll()` sonuçları named groups'ı `.groups` alanında taşıyor; `replace()`/`replaceAll()` native callback replacement fonksiyonlarını kabul ediyor. Test: `tests/runtime.rs::regex_supports_named_groups_and_function_replacements`.

### D21 · `date`/`intl` timezone ve DST güvenli değil
`src/runtime/date.js:43-70` — `format` yalnızca 6 token, timezone yok; `daysBetween` `Math.round` kullanıyor (DST'te bir gün kayar); `addDays` sabit 24 saat ekliyor (DST'te yanlış).
`src/runtime/intl.js:78-97` — timezone ayarlanmıyor; fallback'ler sabit en-US.

### D22 · Yinelenen yardımcılar ve ölü kod
| Konu | Konum 1 | Konum 2 |
|---|---|---|
| base64 | `data.js:8-17` | `bytes.js:11-49` (iki farklı implementasyon) |
| `now`/`iso` | `data.js:74-77` (`time`) | `date.js:32-37` (`date`) |
| metin→string | `core.js:39-51` (`str`) | `shared.js:42-53` (`toText`) — null davranışı farklı |
| statik sunucu | `server.js:233-276` (JS) | `src/cli/dev.rs:281-354` (Rust) — **iki ayrı implementasyon** |
| query parser | `web.js:9-22` | `server.js:295` |

**Ölü export:** `ctx.writeAnswer` (`shared.js:457`) — hiçbir parça okumuyor.
**Ölü `fs.statSync`** — `server.js:242` ve `245`.

### D23 · Test kapsamı boşlukları (backend)
`tests/server.rs` yalnızca 4 test içeriyor (route parametresi, decode, bozuk path, urlencoded body).
`tests/runtime.rs` sunucu testleri: middleware, multipart, session/cookie, HEAD/OPTIONS, static cache, body limit, `stop()`, WS/SSE.
**Test edilmeyenler:** WebSocket binary mesaj semantiği, SSE reconnect/heartbeat, JSON parse hatasının özel hata middleware'iyle etkileşimi. WebSocket 64-bit uzunluk, parçalama/limit/mask kontrolü, SSE disconnect + `stop()`, session cookie flags/prototype safety, `res.send()` + session cookie, kayıtlı OPTIONS route'u, chunked body limit'i, bozuk JSON gövdesi, statik symlink/traversal kaçışı, statik HEAD/Range, `trustProxy`, `EADDRINUSE`, 404/error handler'ları ve handler instance izolasyonu, CORS preflight ve gzip compression artık regresyon testleriyle kapsanıyor.

---

## 8. E — Araç zinciri ve derleme hattı

### E1 · LSP çok zayıf
`src/cli/lsp.rs` — desteklenen metotlar:

```
textDocument/didOpen, didChange, didSave, didClose, hover, completion,
documentSymbol, definition, references, rename, publishDiagnostics
```

**Hâlâ eksik:** `prepareRename`, `workspace/symbol`, `semanticTokens/full`, `formatting`, `codeAction`, `signatureHelp`, `inlayHint`, `documentHighlight`, `foldingRange`, `selectionRange`, `callHierarchy`, `typeDefinition`, `implementation`, `declaration`.

`initialize` artık `documentSymbolProvider`, `definitionProvider`, `referencesProvider` ve `renameProvider` da bildiriyor (`lsp.rs::initialize`).

**Alt sorunlar:**
- **`completion` konumu tamamen yok sayıyor** (`src/cli/lsp.rs:138-141` → `self.completion(&uri)`). Yalnızca `program.body`'deki export'lar + `BUILTINS` listesi → **global isim listesi**. Nokta (`triggerCharacters: ["."]`) sonrası üye tamamlama yok, kapsam-duyarlı tamamlama yok, snippet yok, `kind`/`detail`/`documentation` alanları yok.
- **`hover` yalnızca *ifade* konumlarında çalışıyor** (`src/types/explain.rs:21-28`). Değişken **bildirimi**, fonksiyon adı, tip adı, alan adı → "there is no expression at that place". Doğrulandı: `vey explain main.vey:4:5` → hata.
- ~~**`didClose` yok**~~ ✅ `didClose` dosyayı `self.files`'ten kaldırıyor (`lsp.rs:165-173`).
- **`textDocumentSync: 1`** (tam metin) → incremental sync yok; her tuş vuruşunda dosyanın tamamı taşınıyor.
- ~~**LSP tanı modeli geçersiz alan kullanıyor:** `hint`~~ ✅ kaldırıldı: ipucu mesajın içinde (`hint: ...`), tanıya `code` alanı eklendi. Tanılar artık **ait oldukları dosyanın** URI'si altında yayınlanıyor (bağımlı modülün hatası kendi dosyasında görünüyor).
- **Bilinmeyen istekler `null` ile yanıtlanıyor** (`lsp.rs` `_ => Some(answer(id, Value::Null))`) → "özellik desteklenmiyor" yerine bozuk cevap.
- **Sütunlar UTF-16 değil** (`src/cli/lsp.rs:152-153`, `src/lexer/mod.rs:189` karakter sayıyor) → Türkçe/emoji içeren dosyalarda kayma.
- **`read_message` `Content-Length`'a güveniyor** (`src/cli/lsp.rs:70`) → bozuk/çok büyük header bellek patlaması.

### E2 · Derleyici hattı

| Konu | Durum | Kanıt |
|---|---|---|
| Tree-shaking | ⚠️ **yalnızca `export fn`** | `src/compiler/loader.rs:252-259` · `src/codegen/mod.rs:207-212` |
| Tree-shaking yöntemi | ⚠️ **lexer sezgisi**, AST değil | `src/compiler/loader.rs:266-294` kaynağı yeniden lex'liyor |
| Hesaplanmış erişim | ❌ tree-shaking'i **tümüyle** devre dışı bırakıyor | `m["greet"]("a")` → `m.greet` **ve** `m.other` ikisi de korunuyor |
| Yerel ölü kod temizliği | ❌ yok (sadece uyarı) | `src/check/mod.rs:88-117` |
| Sabit katlama / satır içi | ❌ yok | — |
| Minification | ❌ yok | `grep -rn "minif\|terser\|uglify" src/` → 0 |
| Kod bölme | ⚠️ yalnızca **sayfa** bazlı | `src/cli/site.rs:186` — route bazlı bölme yok, ortak kod sayfalar arası paylaşılmıyor |
| Asset hash / cache busting | ❌ yok | `index.js` adı sabit |
| Incremental build | ❌ **tek slotlu** cache | `src/compiler/cache.rs:291-304` → `.vey/cache.bin` tek dosya |
| Incremental type-check | ❌ yok (LSP'de her tuş vuruşunda tüm proje) | `src/compiler/mod.rs:230-289`, `src/cli/lsp.rs:194` |
| Paralellik | ⚠️ modül başına **thread** (havuz yok), type-check **sıralı** | `src/compiler/mod.rs:184-199` vs `:123-180` |
| Sürüm karşılaştırması | ⚠️ kaba (patch sürümü yok) | `src/compiler/loader.rs:418-429` |

**Bundle boyutu ölçümü:**
```
$ vey build   # hello world
$ ls -la .dist/
-rw-r--r--    1686  404.html
-rw-r--r--     573  index.html
-rw-r--r-- 187036  index.js     ← tamamı runtime
```

`src/runtime/*.js` toplamı **178 KB / 5.768 satır**. `src/runtime.rs:10-41` `PARTS` listesindeki **31 dosyanın tamamı** her pakete gömülüyor (`src/cli/output.rs:13-14`). Hiçbir tree-shaking, satır kısaltma veya DCE yok.

**Cache tek-slot sorunu:** `src/cli/site.rs:249` her sayfa için ayrı `compile_project` çağırıyor → her çağrı `cache::store` ile **aynı** `.vey/cache.bin` dosyasını üzerine yazıyor. N sayfalı bir projede cache pratikte **kullanılamaz**.

**`project_root` iki kopyada:** `src/compiler/mod.rs:300-313` ve `src/cli/entry.rs:111-126` — aynı mantık, iki yer (bakım riski).

**Önbellek dosyası git'e girmiş:** `examples/.vey/cache.bin` repoda takip ediliyor. `.gitignore` yalnızca `/.vey` (kök) için → `examples/.vey/` korunmuyor. `cargo test` her çalıştırmada bu dosyayı değiştiriyor → **dirty çalışma ağacı**.

**Ölü kod:** `src/types/checker.rs:99-102` → `env.resolved = checker.resolved.clone();` **iki kez** atanıyor.

### E3 · Kaynak haritaları

| Sorun | Kanıt |
|---|---|
| `names` **her zaman boş** | `src/sourcemap/mod.rs:158` → `"names":[]`; `Segment` yapısında name alanı **yok** (`mod.rs:29-41`) |
| `sources` **mutlak yol** | `src/compiler/mod.rs:96` → `"/private/var/.../proj/src/main.vey"` → taşınabilirlik yok, build makinesinin dizin yapısı sızıyor |
| Tarayıcı bundle'ındaki harita **kullanılamıyor** | `src/cli/output.rs:24-35` → `//# sourceMappingURL=` yorumu inline `<script>` bloğunun **içinde** kalıyor; tarayıcılar bunu uygulamaz (harita dosyanın son satırında olmalı) → her sayfa ~8 KB ölü harita taşıyor |
| Haritanın %99'u boş | Runtime 5.768 satır → harita 5.850 karakter, **5.812'si `;`** (boş satır ayırıcı), sadece **5 segment** |
| **UTF-16 hatası** | Lexer sütunları **karakter** sayıyor (`src/lexer/mod.rs:182-192`), spec ise **UTF-16 code unit** istiyor → "ş"/"İ"/emoji içeren dosyalarda tüm sütunlar kayar |
| `sourcesContent` her zaman gömülü | bundle boyutu ikiye katlanıyor, kapatma seçeneği yok |
| Harita ayrı dosyaya yazılamıyor | `--sourcemap` / `--sourcemap-external` yok |
| Segmentler yalnızca ifade **başlangıcı** | `src/codegen/mod.rs:127-137` (`if i == 0`) → çok satırlı ifadelerde tek satır işaretleniyor |

### E4 · `vey dev` dosya izleyicisi **polling** ile çalışıyor
`src/cli/dev.rs:149-156`

```rust
loop {
    thread::sleep(Duration::from_millis(300));
    let current = dev_snapshot(&watcher_root, &watcher_entry, &watcher_state);
    if current == previous { continue; }
    thread::sleep(Duration::from_millis(80));   // debounce
```

3.3 Hz'de **tüm dosyalar yeniden okunup hash'leniyor** (`src/cli/dev.rs:600-616` `content_hash` → `fs::read`). `public/` klasöründeki her dosya ve her modül dosyası sürekli okunuyor. `notify` crate'i (native fs event) kullanılmalı.

**Yan etkiler:**
- ~~`vey test --watch` ithal edilen modülleri izlemiyor~~ ✅ düzeltildi: başarılı bundle bağımlılıkları ve derleme hatasındaki dosya watch listesine ekleniyor; bozuk import düzeltilince test yeniden çalışıyor. Testler: `tests/cli.rs::test_watch_restarts_when_an_imported_module_changes`, `tests/cli.rs::test_watch_restarts_when_a_broken_import_is_fixed`.
- `wait_for_change` asla `false` dönmüyor (`src/cli/run.rs:188-207` sonsuz döngü) → `run.rs:104`'teki `break` erişilemez, `run.rs:107-109` **ölü kod**.
- Dev rebuild'i **tüm sayfaları** yeniden derliyor (`src/cli/dev.rs:188` → `site_files` → her sayfa için ayrı `compile_project`).
- **Reload hâlâ script enjeksiyonu** (`src/cli/dev.rs:404-413`) → WebSocket yok, CSS değişikliği kendi kendine yenilenmiyor.

### E5 · Dev sunucusu çok sığ
`src/cli/dev.rs`

- Her yanıtta `"Connection: close"` (`:475`) → keep-alive yok.
- SSE her bağlantıda **thread** + 200 ms polling (`:267`).
- POST body okunmuyor → API yok.
- CORS / güvenlik header'ı yok, ETag yok, gzip yok, Range yok.
- Port çakışmasında "sonraki portu dene" yok (`:121-127`).
- ~~**404 sayfasının script yolu göreli**~~ ✅ düzeltildi: özel 404 sayfasının bundle'ı inline üretiliyor; derin URL'lerde ayrı bir script yolu çözülmesine gerek kalmıyor. Regresyon testi: `tests/lang.rs::builds_a_deployable_site`.

### E6 · Paylaşılan argüman parser'ı **alakasız option'ları sessizce kabul** ediyor
`src/cli/args.rs:43-98` — tüm komutlar aynı parser'ı kullanıyor, her komut yalnızca ihtiyacı olan alanı okuyor.

```
$ vey run --node --port 9999 --template page --write
no entry file found in . (looked for main.vey)
```
`--node`, `--template`, `--write` sessizce yutuldu. Ayrıca `--force` yalnızca `cmd_new` tarafından anlaşılıyor (`args.rs:103-105`), `vey fmt --force` → "unknown option".

Eksik bayraklar: `--verbose/--quiet`, `--json`, `--color/--no-color`, `--config`, `--cwd`, `--profile`, `--log-level`, `--` ayırıcı, `--flag=value`.

### E7 · Config dosyası **hiç yok** 🔄 **kısmen var**
- ✅ `src/cli/config.rs` `vey.toml`'u okuyor (`port`, `host`, `title` …) ve `build`/`dev`/`run`/`lint` onu kullanıyor. **Dikkat:** bu dosya git'e eklenmemiş (`git status`: `?? src/cli/config.rs`) — temiz bir klonda derleme kırılır.
- Env var desteği: `grep "env::var" src/` → hâlâ **0 sonuç** (`VEY_PORT`, `NODE_ENV`, `VEY_OUT_DIR` yok)
- `package.json` yalnızca **yazılıyor** (`src/cli/scaffold.rs`) ve `src/cli/add.rs` tarafından okunuyor; derleyici hâlâ okumuyor.

### E8 · Test koşucusu sade
`src/cli/run.rs` + `src/runtime/core.js:181-253` — `test(name, fn)`, `assert*` matcher'ları, `--filter` (substring), `--watch`.

| Özellik | Durum | Kanıt |
|---|---|---|
| Snapshot testing | ❌ | `src/runtime/core.js:82-179` |
| Coverage | ❌ | `grep coverage src/` → yok |
| Mocking | ❌ | yok |
| Fixtures | ❌ | `tests/fixtures/*.cjs` elle yazılmış Node dosyaları |
| Paralellik | ❌ | `src/cli/run.rs:127` — sıralı, dosya başına ayrı node süreci |
| Timeout | ❌ | `src/cli/run.rs:146-152` — `Command::status()` sonsuza kadar bekler |
| Flaky retry | ❌ | yok |
| Süre ölçümü | ❌ | `src/runtime/core.js:205-236` yalnızca "ok"/"fail" |
| Makine-okunur rapor | ❌ | JSON/TAP/JUnit yok |
| **Test sayacı** | ❌ | `src/cli/run.rs:116` yalnızca **dosya** sayısı basıyor: `"{} file(s) passed"` |

**`assertDeepEquals` `JSON.stringify` tabanlı** (`src/runtime/core.js:117-126`) → `undefined` alanları, `Map`/`Set`, döngüsel yapılar, tarih nesneleri yanlış karşılaştırılır.
**`assertThrows` hata tipini doğrulamıyor** (`src/runtime/core.js:148-161`).

### E9 · Formatter — idempotency bozuk, blok yorumlar yanlış girintiliyor
`src/cli/fmt.rs`

- ~~**Blok yorumları (`/* ... */`) sayılmıyor**~~ ✅ düzeltildi: formatter çok satırlı blok yorumlarının durumunu satırlar arasında izliyor ve içindeki ayraçları saymıyor. Test: `tests/cli.rs::formatter_ignores_braces_inside_multiline_block_comments`.
- **Ardışık boş satırlar tek boş satıra normalize ediliyor** (`fmt.rs::format_source`); bu davranış korunuyor, ancak biçim tercihine dönüştürecek seçenek yok:
  ```
  $ cat c.vey            # "fn a() {}\n\n\n\nfn b() {}\n"
  $ vey fmt c.vey | od -c
  ... f n   a ( )   { }  \n  \n   f n   b ( )   { }  \n     ← 3 boş satır → 1
  ```
- **Sütun genişliği yok** — `MAX_LINE = 100` yalnızca *linter'da* (`lint.rs:21`). Formatter satır bölmez, boşluk normalizasyonu yapmaz (`fn  foo ( a , b )` → aynen kalır).
- **`--check` / `--diff` modu yok** (`fmt.rs:73-77`) → `--write` olmadan **tüm dosya içeriği** stdout'a basılıyor (diff değil). CI'da "biçimlendirilmiş mi?" sorusu cevaplanamıyor.
- **Formatter idempotency kapsamı sınırlı:** `tests/cli.rs::formatter_ignores_braces_inside_multiline_block_comments` aynı dosyayı iki kez formatlayıp çıktının değişmediğini doğruluyor; formatter'ın tüm sözdizimleri için property testi yok.

### E10 · Linter — 3 kural, hep exit 0, `check`'in uyarılarını üretmiyor
`src/cli/lint.rs`

Kurallar: uzun satır (`MAX_LINE = 100`), derin iç içe (`MAX_DEPTH = 4`), yakın isimler (`MIN_DISTANCE = 2`), boş `catch`.

- **Her zaman exit 0** (P0-16) → CI'da işe yaramaz.
- **Bulgular stdout'a**, diğer tüm komutlar hatayı stderr'a koyuyor (`src/cli/output.rs:165`) → boru uyumsuzluğu.
- **Parse hatası tüm komutu bitiriyor** (`lint.rs:79-92`) ve **kaynak çerçevesi gösterilmiyor** (`output.rs::report` kullanılmıyor).
- **Derinlik kuralı yanlış yere raporluyor** (`lint.rs:154-163` → `body.first()`, yani derinliğin olduğu yer değil en dış bloktaki ilk ifade).
- **Yakın isim kuralı çok gürültülü** (`lint.rs:224-239` → `edit_distance < 2` → `a`/`b`, `x`/`y` gibi her çift uyarı üretir).
- **`check::check`'in ürettiği 4 uyarı tipi `lint`'te yok** (`src/check/mod.rs:63-116`): gölgeleme, runtime helper'ını gizleme, kullanılmayan isim, "bu koddan sonrası asla çalışmaz" → **iki ayrı uyarı sistemi**.
- Yapılandırma yok, `// lint-ignore` yok, `--fix` yok.

### E11 · `vey explain` kapsamı dar
`src/cli/explain.rs` + `src/types/explain.rs`

- Çok modüllü dosyalarda **import env'si yok** → `any` (P0-18).
- Yalnızca **ifade** konumlarında çalışıyor; değişken/fonksiyon/tip/alan adlarının tipleri sorulamıyor (`types/explain.rs:66-155` yalnızca `Expr` geziyor, `BindingPattern`/`TypeExpr`/`Param` gezilmiyor).
- `Stmt::Use`, `Stmt::Type`, `Stmt::Interface`, `Stmt::Enum`, `Stmt::Style` gezilmiyor (`types/explain.rs:152`).
- Tip **tanımının gövdesi** gösterilmiyor (`type Foo = { a: number }` için sadece `Foo`).
- "Bu tip neden bu?" izi yok. `--json` çıktısı yok.
- `parse_target` (`explain.rs:78-87`) `rsplitn(3, ':')` kullanıyor → **Windows yollarında** bozuluyor; sütun 0/negatif kabul ediliyor.

### E12 · `vey docs` — doc yorumları bildirime **bağlı değil**
`src/cli/docs.rs`

- `src/ast/decl.rs:14` → `pub docs: Vec<DocComment>` — düz liste. `DocComment.pos` var ama **kullanılmıyor** (`docs.rs:79-85`).
- Sonuç: "Bu doc hangi fonksiyonun?" sorusu **cevaplanamıyor**.
- Fonksiyon imzaları yok (yalnızca isimler, `docs.rs:96-100`), HTML çıktısı yok, modül dizini yok, kapsam grupları yok, markdown kaçışı yok.
- Yalnızca `export` olan isimler listeleniyor → dahili API dokümante edilmiyor.

### E13 · `vey migrate` — gerçek **codemod yok**
`src/cli/migrate.rs`

- Yalnızca `// vey: <sürüm>` yorumunu yeniden yazıyor (`migrate.rs:111-136`). 0.6 → 0.7 arasındaki dil değişiklikleri (CHANGELOG.md) için **hiçbir** dönüşüm kuralı yok.
- ~~**Daha yeni sürüm kontrolü yok**~~ ✅ düzeltildi: sürüm bileşenleri sayısal olarak karşılaştırılıyor; daha yeni veya karşılaştırılamayan beyan hata verir ve dosya değiştirilmez. Testler: `tests/cli.rs::migrate_refuses_to_downgrade_a_newer_file_version`, `tests/cli.rs::migrate_updates_an_older_version_and_is_idempotent`.
- `--from`/`--to` yok, geri alma yok, CRLF normalizasyonu bozuk (`migrate.rs:118-130`), rapor yok.

### E14 · `vey deploy` ve `vey add` — stub düzeyinde

**`vey deploy`** (`src/cli/deploy.rs`):
- Hiçbir deploy hedefi yok (Vercel/Netlify/Cloudflare/S3/GitHub Pages).
- Yalnızca yerel klasöre kopyalama (`deploy.rs:49-61`).
- `copy_folder` (`deploy.rs:65-85`) **hedefteki bayat dosyaları silmiyor** → eski build artıkları kalır.
- Kimlik doğrulama/secret yönetimi yok, build-time env injection yok, preview/rollback yok.
- `-o` (build çıktısı) ile `--target` (yayın hedefi) **çakışıyor**.

**`vey add`** (`src/cli/add.rs`):
- ~~JSON'u string manipülasyonuyla düzenliyor~~ ✅ düzeltildi: `package.json`, CLI'nin JSON parser'ı ile yapısal olarak güncelleniyor; paket adının başka bir string değerinde geçmesi bağımlılığın eklenmesini engellemiyor ve bozuk JSON değiştirilmeden reddediliyor. Testler: `tests/cli.rs::add_writes_a_package_name_that_already_appears_in_a_string_value`, `tests/cli.rs::add_rejects_invalid_package_json_without_changing_it`.
- Sürüm aralığı **"latest"** olarak yazılıyor (`add.rs:84,114`) → tekrarlanabilir build yok.
- ~~`npm install` exit code'u yok sayılıyor~~ ✅ düzeltildi: npm başarısızsa veya başlatılamazsa `vey add` exit 1 dönüyor. Test: `tests/cli.rs::add_fails_when_npm_cannot_install_the_package`.
- `--dev`, `--version`, `--workspace`, `vey remove`, `vey install` yok.

**`npm publish` entegrasyonu yok** — `vey publish`/`vey pack`/`vey version` yok. `scaffold.rs:461-474` ürettiği `package.json` `"private": true` → yayınlanamaz; `main`/`exports`/`files` alanları yok.

### E15 · `vey new` şablonlarında **bozuk Dockerfile** 🔴  ✅ **düzeltildi**

> **Durum (22 Ekim 2026):** düzeltildi; ayrıntı Bölüm 1'deki "Son tur" notunda. Kalan alt kalemler (aşağıda): `git init` yok, `npm install` yok, `--name`/`--description`/`--author` yok, şablonlar kod içine gömülü (kullanıcı şablonu ekleyemiyor). `--force` iddiası geçerli değil: yardım metni zaten "fill in missing files" diyor ve davranış aynı.
`src/cli/scaffold.rs:421-436`

```dockerfile
RUN vey build
EXPOSE 4000
CMD ["node", ".dist/server.cjs"]
```

`vey build` **site** üretir → `.dist/index.html`, `.dist/index.js`, `.dist/404.html` (`src/cli/site.rs:186-197`). **`.dist/server.cjs` hiçbir zaman üretilmiyor** → konteyner `MODULE_NOT_FOUND` ile ölür. `.cjs` yalnızca `vey build --node -o x.cjs` ile üretiliyor.

**CI şablonu `vey test`'i çalıştırıyor** (`scaffold.rs:456`) ama şablon **test dosyası üretmiyor** → `tests/` yoksa `cmd_test` "no tests found" basıp **0** döner (`run.rs:70-72`) → CI sessizce geçiyor.

Diğer eksikler: `git init` yok, `npm install` yok, `--name`/`--description`/`--author` yok, şablonlar **kod içine gömülü** (`scaffold.rs:205-540`) → kullanıcı şablonu ekleyemiyor, `--force` "fill in missing files" diyor ama mevcut dosyalar **atlanıyor** (`scaffold.rs:83-86`).

### E16 · Hata raporlama standardı yok
`src/lib.rs:17-23` → `VeyError { message, file, line, col, hint }`.

Eksik: **error code**, **severity**, **related information** (birden çok konum), **notes** (`help:`/`note:`), **labels** (span), **doc link**, **suggestion listesi**, **quickfix**.

**İki farklı raporlama biçimi:**

| Yol | Biçim | Kanıt |
|---|---|---|
| `run`/`build`/`test`/`check` | çerçeveli (satır + caret + hint) | `src/cli/output.rs:163-176` |
| `lint`/`docs`/`migrate`/`explain`/`repl` | **çerçevesiz** | `lint.rs:82`, `docs.rs:64`, `migrate.rs:65`, `explain.rs:53`, `repl.rs:61` |
| dev overlay | HTML | `src/cli/error.rs:123` |

`repl.rs:61` → `eprintln!("{}", err)` — `err` bir `String` → `Display` → çerçeve ve hint yok.

**`check_file` hataları uyarıya çeviriyor** (`src/compiler/mod.rs:240-252`) → LSP'de tip hatası **severity 2 (Warning)** olarak görünüyor, hata değil.

### E17 · Ölü kod, stub'lar, panic'ler

| Yer | Not |
|---|---|
| 🔴 `src/cli/json.rs:235` | char-boundary **panic** → LSP ölür (P0-14) |
| `src/cli/output.rs:347-355` | `print_command_help("lsp")` **hiç çağrılmıyor** → `vey lsp --help` farklı biçimde |
| `src/cli/run.rs:104` | `break` erişilemez (`wait_for_change` hiç `false` dönmüyor) |
| `src/cli/run.rs:107-109` | `if failed > 0 { continue; }` — **ölü kod** |
| `src/cli/migrate.rs:26` | `return 1 - 1;` — tuhaf ifade |
| `src/types/checker.rs:99-102` | `env.resolved = ...` iki kez atanıyor |
| `src/cli/dev.rs:187,196,204` | `watch` listesi iki kez birleştiriliyor |
| `src/compiler/mod.rs:129-139` | `enums`/`resolved` önce boş atanıyor, sonra üzerine yazılıyor |
| `src/runtime/shared.js:457` | `ctx.writeAnswer` — hiçbir parça okumuyor |
| `src/runtime/server.js:605-610` | `app.route("OPTIONS", ...)` handler'ı **asla çağrılmıyor** |
| `src/runtime/server.js:242,245` | `fs.statSync` **iki kez** çağrılıyor |
| `src/cli/site.rs` | `bundle.warnings` **hiç** raporlanmıyor (P0-17) |

**Panik envanteri** (kullanıcıya dönük yollarda): `src/cli/json.rs:235` 🔴 · `src/compiler/mod.rs:194` (kontrollü, "the compiler stopped") 🟠 · `src/cli/dev.rs:41` (zehirli kilit toparlanıyor) 🟠.
`grep -n "unwrap()" src/cli/` → **0 doğrudan eşleşme** → CLI genelinde panik riski düşük. Bu **iyi** bir durum.

### E18 · Telemetri / crash reporting / ölçüm yok
- `grep -rn "telemetry|analytics|crash|set_hook|panic::catch" src/` → **0 sonuç**
- `Cargo.toml` → **hiçbir bağımlılık** (Cargo.lock 147 bayt, sadece `vey` paketi)
- `panic::set_hook` yok → panic'lerde kullanıcıya dostu stack yok.
- `--verbose`/`--profile`/`--timings` yok → derleme süresi ölçümü yok (sadece `dev.rs:159` "rebuilt the site in N ms").

**Değerlendirme:** Telemetri eksikliği ölçülebilir bir "modern toolchain" farkı olarak **düşük** önceliklidir; daha önemli olan deterministik, tekrarlanabilir ve hızlı bir derleme hattı.

### E19 · Eksik komutlar
`init`, `clean`, `watch`, `coverage`, `bench`, `publish`, `pack`, `remove`, `uninstall`, `install`, `doctor`, `why`, `completions`, `man`, `update`, `env`, `config`, `login` — hiçbiri yok (`src/main.rs:23-56`).

`vey help <cmd>` da yok (`src/main.rs:43-46` → genel yardım basıyor).
Exit code sözleşmesi yok: 0=başarı, 1=hata, 2=kullanım hatası ayrımı yapılmıyor (`src/main.rs:11-14`); panic'lerde `101` dışarı sızıyor.
Argümsız `vey` → exit 0 (`src/main.rs:17-20`).

---

## 9. Eklenecekler — bu dili olgunlaştırmak için

Aşağıdaki liste, "Vey olgun bir dil olsun" hedefi için **eklenmesi gerekenler**. Öncelik sırasıyla.

### 9-A · Dil çekirdeği

| # | Özellik | Söz dizimi önerisi | Neden |
|---|---|---|---|
| A-1 | **Async dönüş tipi** | `fn load() -> Promise<number>` (bugün reddediliyor) | A1'in kökü; en kritik eksik |
| A-2 | **Bileşen içinde yerel durum** | `component Counter() { let count = signal(0) ... }` | A6; bileşenleri yeniden kullanılabilir kılar |
| A-3 | **SVG + web component etiketleri** | `<svg>`, `<path>`, `<my-widget>` | A4; web dilinin temel eksikliği |
| A-4 | **Slot bildirimi + varsayılan içerik** | `component Card(title) { slot body { "varsayılan" } }` + `<Card>{ ... }</Card>` | A3; bileşen API'si |
| A-5 | **Spread prop** | `<Card {...props} />` | A7; sarmalayıcı bileşen kalıbı |
| A-6 | **Noktalı öznitelik adı** | `xlink:href`, `@click`, `:value` | A5; SVG + framework uyumu |
| A-7 | **Örtük dönüş (blok gövde)** | `fn(x) { x + 1 }` → `x + 1` | A2; JS/Swift beklentisi |
| A-8 | **Nitelikli öznitelikler** | `<div @click=... .class=... />` yerine: `class:list=[...]` | A5 |
| A-9 | **`for ... by` her türlü iteratörde** | `for x in list by 2` | A10 |
| A-10 | **Etiketli blok / `with`** | `with file { ... }` | kaynak yönetimi |

### 9-B · Tip sistemi

> Öncelik sırası **B24**'teki tabloya göre. (a) satırları = bugün en çok acı çektiren, (b) satırları = ileri özellikler.

#### (a) Acil — çıkarım kalitesi

| # | Özellik | Söz dizimi / hedef | Neden |
|---|---|---|---|
| B-1 | **Async fonksiyon dönüş tipi `Promise<T>`'ye sarılır** | `fn load() -> number` (await varsa) ⇒ `Promise<number>` | A1/B1'in kökü |
| B-2 | **Runtime helper'larında `Promise<T>`** | `sleep(): Promise<void>`, `http.get(url): Promise<Response>` | B1 |
| B-3 | **Jenerik monomorfizasyonu kaldır** | `fn parse<T>(text: string) -> T` gerçekten jenerik kalmalı; `parse<string>` **dikkate alınmalı** | B4 — yanlış tip kabul ediliyor |
| B-4 | **Bağlamsal tip (contextual typing)** | `map(names, fn(n) { n.length })` → `n: string` | B8 |
| B-5 | **Generic callback desteği** | `fn map<T, U>(list: [T], fn: (T) -> U) -> [U]` | B8 |
| B-6 | **`let`/`const` akış-tipli çıkarım** | `const x = findUser()` → `User \| null` (bugün `any`) | B9 |
| B-7 | **`any` → `unknown` geçişi** | annotation'sız `let` → `unknown`; `unknown` daraltmayı zorunlu kılsın | B10 — bugün neredeyse hiçbir akış denetlenmiyor |
| B-8 | **`?` operatörü hata tipini taşısın** | `Result<T,E>` → `?` → `E` de akışa girsin | B2 |
| B-9 | **Sözlük üye erişimi + anahtar tipi denetimi** | `d.foo` → value tipi; `d[true]` → hata | B11 |
| B-10 | **Union kapsayıcılığı** | `match x { 1 => .., "a" => .. }` → tükendi | B5 |
| B-11 | ~~**`is` ile `unknown`/`never`/`symbol`/`bigint`**~~ ✅ yapıldı | `is_builtin_kind` ↔ `is_builtin_type` hizası | B7 |
| B-12 | **`null` sıralanabilirliği tutarlılığı** | `null < 5` ya da `null == 5` — biri hata olmalı | B13 |
| B-13 | **Discriminated union daraltma** | `if s.kind == "circle" { s.r }` | B6 |

#### (b) İleri — modern tip özellikleri

| # | Özellik | Söz dizimi önerisi |
|---|---|---|
| B-14 | **`typeof`** | `type T = typeof value` |
| B-15 | **Mapped / conditional type** | `type Partial<T> = { [K in keyof T]?: T[K] }` |
| B-16 | **Index signature (yazma)** | `type Dict = { [key: string]: number }` (okuma zaten var) |
| B-17 | **`as const` / literal tipler** | `let mode = "dark" as const` |
| B-18 | **`satisfies`** | `let c = { port: 1 } satisfies Config` — kontrol et, tipi koru |
| B-19 | **Trait / interface uygulaması** | `impl Named for User` — nominal + yapısal birlikte |
| B-20 | **Generic `impl`** | `impl<T> Box<T> { fn get() -> T }` |
| B-21 | **Değişmez (immutable) tipler** | `type Frozen = readonly [number]` |
| B-22 | **`unknown` gerçekten üretilsin** | `never` tabanlı akış analizi, erişilemez kod tespiti |
| B-23 | **Higher-kinded types / associated types** | `impl<T> Iterable { type Item }` |
| B-24 | **Variance** | `Box<in T>` / `Box<out T>` — bugün `Var` her şeyi kabul ediyor |
| B-25 | **Template literal tipleri** | `` type Route = `/users/${string}` `` |
| B-26 | **`infer`, `keyof`, `Pick`, `Record`, `Partial`** | standart yardımcı tipler |
| B-27 | **Tip hatası zenginliği** | eksik kol listesi, parametre adı, "bu karşılaştırma her zaman yanlış" uyarısı |

### 9-C · Frontend

| # | Özellik | API önerisi | Öncelik |
|---|---|---|---|
| C-1 | ~~**Toplama (batching)**~~ ✅ `batch` + `untrack` eklendi | `state.batch(fn() { ... })` veya otomatik mikro-görev | 🔴 |
| C-2 | ~~**Eşitlik politikası**~~ ✅ yapıldı | `signal(v, { equals: (a, b) => a.id === b.id })`, `{ equals: "deep" }` | 🔴 |
| C-3 | ~~**Context**~~ ✅ var | `provide("theme", theme)` / `inject("theme")` | 🟡 |
| C-4 | **Memo** — ⚠️ mimari engel (bölüm 6, C4) | `memo(Component)` | 🟡 |
| C-5 | ~~**Hata sınırı**~~ ✅ var | `Boundary fallback=Fallback { ... }` | 🔴 |
| C-6 | **Async bileşen / Suspense** | `component Data() { let x = await load() ... }` + `<Suspense fallback=...>` | 🔴 |
| C-7 | ~~**Portal**~~ ✅ yapıldı | `portal(content, target, key?)` | 🟡 |
| C-8 | ~~**Geçiş yardımcısı**~~ ✅ yapıldı | `transition(200, { easing: "ease-out" })` → `{ enter, leave }` | 🟡 |
| C-9 | ~~**Form yardımcısı**~~ ✅ yapıldı | `form(values, { check })` → `{ values, errors, valid, dirty, set, submit, reset }` | 🟡 |
| C-10 | ~~**`onDestroy` düzeltmesi**~~ ✅ yapıldı | Node kaldırıldığında component destroy zinciri çalışıyor | 🔴 |
| C-11 | ~~**Head yönetimi**~~ ✅ var | `title { ... }`, `meta ...`, `link ...` head'e yazılıyor | 🟡 |
| C-12 | **SSR + hydration** | `vey build` → HTML + `data-vey` işaretleri; `hydrate("#app", App)` | 🔴 |
| C-13 | ~~**Odak ve scroll koruma**~~ ✅ yapıldı | sayfa yazılırken odak ve kaydırma yeri geri alınıyor | 🟡 |
| C-14 | **Listelerde sanallaştırma** | `virtual(for item in bigList)` | 🟢 |
| C-15 | **Olay delegasyonu otomatik** | `on:` direktifi veya `delegate("click", ".row", fn)` global | 🟢 |
| C-16 | ~~**Markup'ta yan etki ifadeleri çocuk olmamalı**~~ ✅ yapıldı | `onMount(...)` sayfaya yazılmıyor (P0-19) | 🔴 |
| C-17 | **İki yönlü diff** | İlk ağaç kurulmadan önce karşılaştırma (yalnızca değişen düğümleri kur) | 🟡 |
| C-18 | ~~**Tip'li durum**~~ ✅ yapıldı | `signal<T>(0)`, `store<T>(...)`, `Signal<T>` / `Store<T>` | 🟡 |

### 9-D · Backend

| # | Özellik | API önerisi |
|---|---|---|
| D-1 | **HTTPS / HTTP2** | `server(3000, { tls: { cert, key } })` |
| D-2 | **Güvenlik header'ları** | `security()` middleware (helmet benzeri) |
| D-3 | **CSRF koruması** | `csrf()` middleware |
| D-4 | **Rate limit** | `rateLimit({ perMinute: 60 })` |
| D-5 | **`res` genişletme** | `res.redirect`, `res.sendFile`, `res.download`, `res.type`, `res.vary`, `res.append`, `res.write` (streaming) |
| D-6 | **Statik dosya middleware** | `app.static("public", { maxAge, immutable, index })` + Range/206 + streaming |
| D-7 | **Şema doğrulama** | `fn handler(req: { body: User })` veya `validate(schema, body)` |
| D-8 | **Session deposu arayüzü** | `session({ store: redisStore })`, TTL, `regenerate()`, `flash` |
| D-9 | **KV / cache deposu** | `cache()` — TTL, namespace, kota |
| D-10 | **Timeout + iptal** | `server(3000, { requestTimeout: 30_000 })` + `AbortSignal` |
| D-11 | **Graceful shutdown** | `SIGTERM`/`SIGINT` → `app.stop(deadline)` + `closeAllConnections()` |
| D-12 | **İçerik doğrulama** | `app.post("/x", validate(User), fn(req){...})` |
| D-13 | **WebSocket tam RFC6455** | parçalama, binary, close kodu, ping keepalive, max payload, origin, auth |
| D-14 | **SSE geliştirme** | `id`, `retry`, heartbeat, `Last-Event-ID` resume |
| D-15 | **HTTP istemcisi genişletme** | `http.put/patch/delete/head`, gerçek `Response`, retry politikası, proxy |
| D-16 | **Config / env** | `.env` desteği, `vey.toml`, şema doğrulamalı env |
| D-17 | **Loglama** | seviye, request-id, JSON formatı, `req.log` |
| D-18 | **Async `fs`** | `await fs.read(...)`, `fs.write` atomik, `fs.stat/rename/copy` |

### 9-E · Araç zinciri

| # | Özellik |
|---|---|
| E-1 | LSP: definition, references, rename, documentSymbol, semanticTokens, formatting, codeAction, signatureHelp |
| E-2 | `vey test`: snapshot, coverage, mocking, fixture, timeout, paralellik, reporter, `describe/it`, `skip` |
| E-3 | `vey.toml` / `package.json` config |
| E-4 | `vey fmt`: genel property/idempotency testi, config, `.editorconfig` |
| E-5 | `vey lint`: kural seti genişletme + `// vey:ignore` kaçışı |
| E-6 | `notify` crate ile native dosya izleyici (polling yerine) |
| E-7 | Minify + asset hash + route bazlı code splitting |
| E-8 | WebSocket tabanlı HMR (CSS değişikliği dahil) |
| E-9 | `vey publish` (npm) |
| E-10 | CI/CD şablonları, Dockerfile |
| E-11 | Hata kodları + "related information" |
| E-12 | Kaynak haritasında `names` |
| E-13 | `vey docs`: API referansı, tür imzaları, örnek üretimi |
| E-14 | `vey bench` / performans ölçümü |

### 9-F · Dil kültürü, dokümantasyon ve ekip

| # | Özellik | Not |
|---|---|---|
| F-1 | ~~**README ve ROADMAP geri yazılmalı**~~ ✅ geri yazıldı | `README.md` ve `ROADMAP.md` yeniden yazıldı; `tests/readme.rs` README örneklerini ve her yardımcının belgede geçmesini denetliyor. |
| F-2 | **Dil referansı** | Söz dizimi + tip sistemi + runtime kütüphanesi, tek bir referans belgesinde (ve `vey docs` ile üretilebilir). |
| F-3 | **Sürüm politikası** | `// vey: 0.7` beyanının semantiği belirsiz; `vey migrate` daha yeni veya karşılaştırılamayan sürümü artık ezmiyor. |
| F-4 | **Örnek projeler** | `examples/` 10 dosya içeriyor ama hepsi küçük; gerçek bir uygulama örneği (uygulama + sunucu + test) yok. |
| F-5 | **Kural katkı rehberi** | Runtime'a yeni bir yardımcı eklendiğinde `src/builtins.rs` + `src/types/builtins.rs` + README + LSP listesinin **dördü** birden güncelleniyor — bu kural yazılı değil ve daha önce kaymış. |
| F-6 | **İki yardımcı listesi tek listeye** | `src/builtins.rs` (137 isim) ve `src/types/builtins.rs` (~100 metot imzası) ayrı ayrı tutuluyor; `src/cli/lsp.rs` de üçüncü bir kopya. |
| F-7 | **Yerelizasyi** | Tüm mesajlar İngilizce; i18n altyapısı yok. |
| F-8 | **Topluluk** | Katkı rehberi, davranış kuralları, sürüm duyurusu şablonu yok. |

---

## 10. Yol haritası

### Faz 0 — Doğruluğu onar (1–2 hafta) ✅ **tamamlandı**
P0-1 … P0-19. Yeni özellik yok, sadece yanlış davranışları düzelt. Her biri için regresyon testi.

**Çıktı:** "Vey'in yazdığı her program beklenen şekilde çalışıyor."

En kritik üçü:
1. **P0-13** (isim çözümü üretim sırasına bağlı → yanlış program sonucu)
2. **P0-14** (LSP panic) + **P0-1** (oturum çerezi kaybı) + **P0-2** (`bodyLimit` etkisiz) + **P0-3** (`EADDRINUSE` crash)
3. **P0-19** (markup'ta `onMount` metin düğümü olarak render ediliyor)

**Yapılanlar:**
- **P0-1** — `res.beforeFinish(fn)` eklendi (`src/runtime/shared.js`); `Set-Cookie` artık gövde yazılmadan önce yazılıyor (`src/runtime/server.js`).
- **P0-2** — Limit **gövde okunmadan** uygulanıyor: `content-length` kontrolü + `readBody` içinde kesme. `bodyLimit` artık bir "işaret" taşıyor ve sunucu onu görüyor.
- **P0-3** — `serverInstance.on("error")` eklendi; `app.onError(fn)` ve `app.lastError()` eklendi.
- **P0-4** — `sameText()` ile sabit-zamanlı karşılaştırma (`src/runtime/node.js`).
- **P0-5** — `trustProxy` seçeneği; başlıklar yalnızca proxy varsa okunuyor.
- **P0-6** — `exposeErrors` seçeneği; varsayılan olarak hata mesajı sızmıyor.
- **P0-7** — WS 64-bit frame uzunluğu **iki yarı** okunuyor; `maxFrame` limiti eklendi.
- **P0-8** — SSE `close`/`error` dinleniyor → `running` sayacı düşüyor, `stop()` donmuyor.
- **P0-9** — Oturum çerezi `Secure`/`Path` alıyor; `session.regenerate()` eklendi (session fixing).
- **P0-10** — `find()` ve `Map.get()` artık `null` döndürüyor.
- **P0-11** — `uuid()`/`random()` kriptografik kaynağı kullanıyor.
- **P0-12** — `compress` `Vary: Accept-Encoding` ekliyor, bayt tabanlı eşik kullanıyor, sıkıştırılmış türleri atlıyor; `res.transform` artık **birleşiyor**, `res.append` eklendi.
- **P0-13** — Kod üretici artık top-level isimleri **kod yazılmadan önce** biliyor (`Codegen::declare_top_level`).
- **P0-14** — JSON okuyucu `\u` kaçışını **harf harf** okuyor; artık panic olmuyor, vekil çiftler de okunuyor. Birim testi eklendi.
- **P0-15** — Build artık **manifest** (`.vey-manifest`) tutuyor; yalnızca kendi yazdığı dosyaları siliyor. `--force` ile klasör boşaltılabiliyor.
- **P0-16** — `vey lint` ve `vey fmt` bulgu varsa **exit 1** döndürüyor.
- **P0-17** — Site build'i derleyici uyarılarını basıyor.
- **P0-18** — `vey explain` ve LSP hover artık **import env**'ini kuruyor (`compiler::explain_source`).
- **P0-19** — `onMount`/`onDestroy` markup'ta yan etki olarak yazılıyor, çocuk listesine girmiyor; `toDom`/`html` fonksiyonları metne çevirmiyor.
- Ayrıca: **kırık route testi** düzeltildi (sahte DOM'un `parentNode` temizliği + test programında `return`).
- Eklenen regresyon testleri: `tests/runtime.rs` (+2), `tests/cli.rs` (+2), `tests/regressions.rs` (+3), `tests/dom.rs` (+1), `src/cli/json.rs` (+2 birim testi).


### Faz 1 — Tip güvenliğini tamamla (3–4 hafta) — 🔄 **devam ediyor**
**Önce çıkarım kalitesi (B24 tablosunun 1–8'i), sonra ileri özellikler.**

**Yapılanlar:**
1. ✅ **A-1 / B-1** — Async bir fonksiyonun dönüş tipi artık `Promise<T>`. `fn load() -> number` (await varsa) ⇒ `Promise<number>`; `Promise<number>` **elle de yazılabiliyor**. `await load()` artık `number` veriyor, `let y: string = await load()` **hata** veriyor. (`src/types/stmts.rs`, `src/types/checker.rs`)
2. ✅ **B-2 (kısmi)** — Runtime'ın async yardımcıları `Promise<T>` döndürüyor: `sleep` → `Promise<void>`, `http.get/post/fetch` ve `fetch` → `Promise<Response>`, `all`/`race` → `Promise<...>`, `clipboard.write` → `Promise<void>`. `Response` tipi tanımlandı. (`src/types/builtins.rs`)
3. ✅ **B-3** — Verilen tip argümanı artık **yok sayılmıyor**: `parse<string>("42")` → "type argument 'T' expects number, found string". Çözülmüş tip parametreleri kısıt olarak saklanıyor. (`src/types/stmts.rs`)
4. ✅ **B-5** — Union türü artık **tükenebilir**: `Color | null` match'i eksik varyantı **adıyla** söylüyor ("missing: Color.Blue"). `number | string` hâlâ bir ad ya da `_` istiyor (literal ile kapatılamaz).
5. ✅ **B-8** — **Bağlamsal tip** eklendi: bir callback, verildiği fonksiyonun tipinden parametrelerini alıyor. `each(items: [string], f: (string) -> void)` → `fn(item) { item.length }` artık `string` biliyor.
6. ✅ **B-11** — Sözlüğün üyesi artık değer tipini veriyor (`d.b` → `number`), anahtar tipi denetleniyor (`d[true]` → hata).
7. ✅ **B-21 (kısmi)** — `vey explain` ve LSP hover artık **import env**'ini kuruyor → çok modüllü dosyalarda `any` yerine gerçek tip. (`compiler::explain_source`)
8. ✅ **B-7 / B-15** — `is` artık runtime'ın bildiği **bütün** türlerle çalışıyor: `unknown`, `never`, `void`, `symbol`, `bigint`. İki ayrı liste (`is_builtin_kind` ↔ `is_builtin_type`) **tek listeye** bağlandı: `is_builtin_kind` artık `is_builtin_type`'a soruyor. `unknown` bir değer `is` ile **daraltılabiliyor** (`if v is string { return v }`), ve olmayacak bir kontrol (`number` üzerinde `is string`) **hata** veriyor. Runtime `is()` `any`/`unknown`/`never`/`void`/`symbol`/`bigint` kind'larını biliyor. (`src/types/exprs.rs`, `src/runtime/core.js`)
9. ✅ **B-9 (yarısı)** — Annotation'sız `const` artık **değerin tipini koruyor**: `const user = find()` → `User | null`, ve `user.age` hata veriyor. Nesne ve liste (`const obj = { a: 1 }`) **bilerek** dinamik kalıyor, çünkü alanlarına sonradan yazılabiliyor (mevcut test bunu istiyor). `let` `any` kalıyor — dil "dinamik" diyor, karar belgelendi. (`src/types/stmts.rs`)
10. ✅ **B-12** — `Object → Interface` artık **exact**: interface hedefte fazla alan sessizce kabul edilmiyor, alan **adıyla** söyleniyor ("'y' is not a field of Point"). Aynı kural çağrı argümanlarında da geçerli: parametre jenerik değilse `unify` başarısı `fits` kontrolünü artık atlamıyor. (`src/types/rules.rs`, `src/types/exprs.rs`, `src/types/helpers.rs`)
11. ✅ **B-18** — Bağımlı modülün tip hatası artık **yutulmuyor**: `type_environments` hatada durmuyor, bozuk modülü `unchecked` olarak işaretleyip kalanları denetliyor ve **bütün** hataları topluyor (`FileProblems.errors`). LSP her tanıyı **kendi dosyasının** URI'si altında yayınlıyor (`publish` artık grupluyor). (`src/compiler/mod.rs`, `src/cli/lsp.rs`)
12. ✅ **B-22 / B-23 (temizlik)** — `type_param_constraints` (yazılıp hiç okunmayan alan), `block_always_returns`/`stmt_always_returns` (ölü fonksiyonlar) ve iki kez atanan `env.resolved` kaldırıldı. `TypeChecker` artık **tek** yerde kuruluyor (`build_checker`), iki özdeş struct literal'i kalmadı. (`src/types/checker.rs`, `src/types/helpers.rs`)
13. ✅ **B-20 (kısmi)** — Union üyesinde alan yoksa **sessizce `any`** dönmüyor; üyenin kendi hatası söyleniyor (`user.age` → "type User has no field 'age'"). LSP tanısında geçersiz `hint` alanı kaldırıldı, ipucu mesajın içine yazılıyor ve `code` alanı eklendi.

**Kalan (öncelik sırasıyla):**
- B-16 — `Interface` tip argümanlarını taşımıyor (`Box<number>` ile `Box<string>` aynı görünüyor). Kullanıcıya görünen etkisi küçük (alanlar zaten yerine konuyor), değişiklik `Type` enum'una dokunuyor → sonraya bırakıldı.
- B-19 — İleri tip özellikleri (mapped/conditional type, trait, literal tipler).
- B-22 — `unknown` gerçekten üretilsin (bugün yalnızca yazılabiliyor).

**Çıktı:** "Async kod ve kütüphane çağrıları tip güvenli; `any` azaldı."

### Faz 2 — Frontend'i gerçekten reaktif yap (3–4 hafta) — 🔄 **devam ediyor**

**Yapılanlar:**
1. ✅ **C-1 (toplama)** — Sayfa yazımı toplanıyor: on değişiklik sayfayı **bir kez** yazıyor. Toplama, efektin aboneliğinde yapılıyor (`effect(fn, { defer: true })`), böylece bağımlılık takibi bozulmuyor. (`src/runtime/state.js`, `src/runtime/mount.js`)
2. ✅ **C-2 (eşitlik)** — Sinyalin eşitlik politikası var: `signal(value, { equals })`, `store(name, initial, { equals })`. Varsayılan `Object.is`; `{ equals: "deep" }` derin karşılaştırma. `deepEquals(a, b)` de yardımcı olarak eklendi.
3. ✅ **C-10 (`onDestroy`)** — `onMount`/`onDestroy` artık **bileşenin kendine** aittir: runtime bileşenin yerini (path) takip ediyor, `onDestroy` işi node kaldırıldığında (veya değiştirildiğinde) çalışıyor. `onMount` yalnızca **bir kez** çalışıyor.
4. ✅ **C-16 (markup yan etki ifadeleri)** — `onMount`/`onDestroy` markup'ta metin düğümü olarak basılmıyor (P0-19).
5. ✅ **A-2 (bileşen durumu)** — Bileşenin kendi `let`/`const` değerleri var ve sayfa yeniden yazıldığında **korunuyor** (her bileşenin kendi durumu var). Runtime bileşenin yerini bir "yol" ile takip ediyor.
6. ✅ **A-3 (SVG)** — SVG etiketleri yazılabiliyor (`svg`, `path`, `circle`, `g`, `defs`, `linearGradient`…). Runtime SVG düğümlerini `createElementNS` ile yapıyor. `use` gibi bir dil kelimesi de etiket olabiliyor.
7. ✅ **A-5 (spread prop)** — `Card ...props` ve `Card ...{ title: "x" }` yazılabiliyor.
8. ✅ **A-6 (noktalı öznitelik)** — `xlink:href`, `xml:lang`, `data-value` yazılabiliyor. `xlink:` işaretleri SVG'nin kendi uzayında yazılıyor.
9. ✅ **C-5 (hata sınırı)** — `Boundary` bileşeni, altındaki bileşenlerin hatasını yakalıyor ve `fallback` ile değiştiriyor.
10. ✅ **A-4 (slot)** — Bileşen **slot** bildirebiliyor: `slot name="body" { varsayılan }`. Çağıranın verdiği içerik slot'un yerini alıyor; çağıran bir şey vermezse slot'un kendi içeriği kalıyor. Slot'lu parametre artık zorunlu değil.
11. ✅ **Diff sıralama hatası** — `patchChildren` artık node'ları doğru sıraya koyuyor (eskiden değiştirilen node'lar yanlış sıraya gidiyordu ve sahte DOM'da kopyalanıyordu).
12. ✅ **Test kararlılığı** — `tests/dom.rs` ve `tests/dev.rs` kendi geçici klasörlerini **sayacısız** yapıyordu; paralel testler aynı klasörü paylaşıp birbirinin dosyalarını eziyordu. Sayaç eklendi. DOM testleri artık sabit süre beklemek yerine **yoklama** ile bekliyor.
13. ✅ **C-7 (portal)** — `portal(content, target, key?)`: sayfanın bir parçası **başka bir elementin** içine yazılıyor (diyalog, menü, tooltip). Portal kutusu adıyla saklanıyor, sayfa yeniden yazıldığında **aynı kutu** `patchChildren` ile güncelleniyor (alanların yazdığı kaybolmuyor), sayfa onu artık tutmuyorsa kutu **kaldırılıyor**. (`src/runtime/dom.js`, `src/runtime/mount.js`)
14. ✅ **C-8 (geçiş yardımcısı)** — `transition(duration, { easing, from, to, leave })` bir `enter` ve bir `leave` veriyor; ikisi de `enter=`/`leave=` özniteliklerine **doğrudan** verilebiliyor (`enter=fade.enter`). Değer önce `from`, sonra `to`/`leave` olarak yazılıyor; `leave` işi süre dolunca bitiyor, node o zaman kaldırılıyor. Süre 0 ise geçiş yapılmıyor. (`src/runtime/dom.js`)
15. ✅ **C-9 (form yardımcısı)** — `form(initial, { check })`: `values`, `errors`, `valid`, `dirty`, `set`, `replace`, `reset`, `check`, `submit` veriyor. `valid` ve `dirty` **computed** sinyaller, yani alan değişince kendiliğinden güncelleniyor; `submit` yalnızca form geçerliyken fonksiyonu çağırıyor (`Promise.resolve(null)` aksi halde). (`src/runtime/form.js`)
16. ✅ **Bileşen bir adda tutulabiliyor** — `let Box = Card` (veya `m.Card`) sonrası `Box("x")` etiket olarak yazılabiliyor; tip denetleyici adı tanıdığı için props'ları denetlemiyor, statik denetleyici de adı **kullanılmış** sayıyor. (`src/types/stmts.rs`, `src/check/mod.rs`)

**Önceden yapılmış olduğu doğrulanan kalemler** (bölüm 9-C'de ✅ işaretli, yeni testle kapsandı): context (`provide`/`inject`), head yönetimi (`title { ... }`, `meta ...`) ve hata sınırı (`Boundary`) — üçü birlikte `tests/dom.rs::the_head_the_shared_values_and_the_fallback_are_written` ile doğrulanıyor.

**Kalan:**
- C-4 (memo) — **mimari engel**: sayfa her yazılışta **gerçek DOM** kuruyor (`h()` `doc` varsa elementi hemen yapıyor), yani bir bileşenin alt ağacı yeniden kullanılamıyor. Denendi: memoize edilen düğümler yeni ebeveyne taşınıp ağacı bozuyor. Doğru çözüm iki fazlı (önce sanal ağaç, sonra diff) bir yazım; bu ayrı bir iş.
- Bu mimari engel **C-4, C-6 (async bileşen), C-12 (SSR → hydration) ve C-17 (iki yönlü diff) için ortaktır**: dördü de "önce bir ağaç kur, sonra onu karşılaştır" düzenini istiyor. Onları tek tek değil, **iki fazlı yazım** üzerinden kapatmak gerekir.
- C-13 (scroll/odak koruma) ✅ yapıldı; C-11 (head yönetimi) zaten vardı.

**Çıktı:** "Vey ile yazılan bir sayfa, React/Svelte/Solid seviyesinde bir sayfa."

### Faz 3 — Backend'i güvenli ve eksiksiz yap (2–3 hafta)
- D-1 (TLS), D-2 (güvenlik header), D-3 (CSRF), D-4 (rate limit).
- D-5 (`res` genişletme), D-6 (statik middleware + Range + streaming).
- D-7/D-12 (şema doğrulama), D-8 (session deposu), D-9 (cache).
- D-10/D-11 (timeout, graceful shutdown), D-13/D-14 (WS/SSE tam).
- D-15 (HTTP istemcisi), D-16 (config/env), D-18 (async fs).

**Çıktı:** "Vey sunucusu production'a çıkabilecek bir sunucu."

### Faz 4 — Dil ve araç zinciri (sürekli)
- 9-B(b) ileri tip özellikleri (mapped/conditional type, trait, discriminated union, literal tipler).
- E-1 … E-19 (araç zinciri).
- E-2 derleme hattı: minify + tree-shaking'i AST tabanlı yapma + çok yuvalı cache + doğru source map.
- Dil kültürü: `vey new` şablonları (bozuk Dockerfile düzeltilecek), örnekler, README/ROADMAP'in geri yazılması, sürüm politikası.

---

## 11. Test stratejisi

**Mevcut:** 5.348 satır Rust testi, `tests/readme.rs` README örneklerini doğruluyor (çok iyi bir karar — dokümantasyon dilden ayrışamıyor).

**Eksik olan katmanlar:**

1. **DOM testleri sahte DOM'a dayanıyor** ve sahte DOM gerçek DOM'dan farklı davranıyor (Bölüm 2). → `tests/fixtures/dom-fake.cjs` içinde `link()`/`removeChild`/`replaceChild` sonrası `parentNode`/`nextSibling` temizlenmeli, ya da gerçek bir DOM implementasyonu (`linkedom`, `happy-dom`) kullanılmalı.
2. **Reaktivite testleri yok**: batching, equality, `onDestroy`, `onMount` tekrarı, keyed diff, odak koruma, markup'ta yan etki ifadesi (P0-19).
3. **Backend'de P0 hatalarını yakalayacak test yok** (D23 listesi + P0-1, P0-2, P0-3, P0-8).
4. **Tip testleri**: `tests/types.rs` 732 satır — iyi ama şu senaryolar yok: `Promise` dönüş tipi, `Result` hata tipi, union kapsayıcılığı, generic monomorfizasyon, `unknown` daraltma, dict erişimi, `null` karşılaştırması.
5. **Fuzz/property test**: parser derinlik, string kaçışları, markup iç içe, `vey lsp` bozuk girdi (P0-14).
6. **Kıyaslama (differential) test**: aynı programın `vey build` çıktısı ile beklenen JS çıktısı; özellikle isim çözümlemesi (P0-13).
7. **Coverage ölçümü** yok → hangi satırlar test ediliyor bilinmiyor.
8. **CI yok**: `cargo test` + `cargo build --release` + `cargo clippy` + `cargo fmt --check` + node sürüm matrisi (18/20/22/24).
9. **Formatter property testi kapsamı sınırlı**: block-comment senaryosunda iki ardışık formatlama eşitliği var; tüm AST/syntax aileleri için yok (E9).
10. **Linter/fmt exit code testi yok**: (bugün **her zaman 0** — P0-16).

**Önerilen test kurgusu:**

```
tests/
  lang.rs          # dil davranışı (var)
  types.rs         # tip denetleyici (var) — async/Result/union eklenecek
  runtime.rs       # runtime yardımcıları (var)
  server.rs        # sunucu (var) — P0 regresyonları eklenecek
  dom.rs           # DOM (var) — fixture düzeltilecek
  cli.rs           # CLI (var) — exit code + config testleri eklenecek
  dev.rs           # dev sunucu (var)
  regressions.rs   # (var) — her P0 için bir regresyon testi buraya
  readme.rs        # README örnekleri (var) — çok iyi
  # yeni:
  snapshots/       # altın dosyalar
  properties.rs    # property/fuzz testleri
```

---

## 12. Kanıt indeksi

### Derleyici / dil
| Bulgu | Konum |
|---|---|
| Async dönüş tipi `Promise` ile sarılmıyor | `src/types/stmts.rs:822` |
| Tüm async helper'lar `any` | `src/types/builtins.rs:61` |
| `Promise`/`Result` tip varyantları | `src/types/core.rs:45,48` |
| Blok gövde örtük dönüş yapmıyor | `src/codegen/exprs.rs:346-365` |
| Async fonksiyon JS `async` oluyor | `src/codegen/mod.rs:261-262` |
| **İsim çözümü üretim sırasına bağlı** | `src/codegen/mod.rs:150-168` |
| Slot yalnızca çağrıda yakalanıyor | `src/parser/markup.rs:229,262` · `src/parser/mod.rs:482` |
| Checker slot'u parametre bekliyor | `src/types/stmts.rs:988-995` |
| HTML etiket listesi (SVG yok) | `src/parser/html.rs:3-102` |
| `is_element_start` kabul kuralları | `src/parser/markup.rs:89-112` |
| Öznitelik adında `-` dışında karakter yok | `src/parser/markup.rs:297-305` |
| Markup'ta `let` yok | `src/parser/markup.rs:34-58` |
| Markup'ta yan etki ifadesi çocuk listesine giriyor | `src/codegen/markup.rs:21` · `src/parser/markup.rs:34-58` |
| Parser derinlik sınırı | `src/parser/mod.rs:39` |
| `impl` metotları örneğe kopyalanıyor | `src/codegen/mod.rs:639-676` |
| `for ... by` sadece range | `src/codegen/mod.rs:485-495` |

### Tip sistemi
| Bulgu | Konum |
|---|---|
| Tip parametresi her şeyi kabul ediyor (variance yok) | `src/types/rules.rs:172-175` |
| `any` iki yönde de şeffaf | `src/types/rules.rs:141-145` |
| Jenerik monomorfizasyon | `src/types/stmts.rs:232-237` |
| Açık tip argümanı düşürülüyor | `src/types/exprs.rs:893-896` |
| Union `match` asla tükenmez sayılmıyor | `src/types/exprs.rs:368-380` |
| Tuple/Object/Range desenleri tükenmezliğe katılmıyor | `src/types/exprs.rs:294-299,381` |
| `?` hata tipini düşürüyor | `src/types/exprs.rs:427` |
| `catch` hata değeri `any` | `src/types/stmts.rs:471` |
| Bağlamsal tip yok | `src/types/exprs.rs:715-769` |
| Annotation'sız `let`/`const` → `any` | `src/types/stmts.rs:66,169,175` |
| `Dict` nokta erişimi → `any` | `src/types/exprs.rs:1563` |
| ~~Union üyesinde alan yoksa sessizce `any`~~ ✅ üyenin hatası söyleniyor | `src/types/exprs.rs::lookup_member` (`Type::Union`) |
| `unknown` interface'e atanamıyor | `src/types/rules.rs` — `(Unknown, Interface)` kolu yok |
| ~~`is_builtin_kind` ↔ `is_builtin_type` çelişkisi~~ ✅ tek liste | `src/types/exprs.rs::is_builtin_kind` |
| `null` sıralanabilir ama karşılaştırılamaz | `src/types/rules.rs:62` ↔ `:28-30` |
| Excess property kontrolü tutarsız | `src/types/rules.rs:199-218` |
| `as` tipi değiştiriyor (`satisfies` yok) | `src/types/exprs.rs:447-456` |
| `Interface` tip argümanlarını taşımıyor | `src/types/resolve.rs:388` |
| `for_explain` imports boş | `src/types/checker.rs:563` |
| ~~Bağımlı modülün hatası yutuluyor~~ ✅ `FileProblems.errors` + `type_environments` | `src/compiler/mod.rs` |
| `check/mod.rs` ↔ `types/*` mükerrerliği | `src/check/mod.rs` ↔ `src/types/*` |
| `type_param_constraints` yazılıyor, okunmuyor | `src/types/checker.rs:493,632,639` |
| `block_always_returns` / `stmt_always_returns` ölü | `src/types/helpers.rs:44,62` |
| `env.resolved` iki kez atanıyor | `src/types/checker.rs:100,102` |
| `TypeChecker` iki kere yapılandırılıyor | `src/types/checker.rs:66-88` ↔ `:557-579` |

### Frontend runtime
| Bulgu | Konum |
|---|---|
| Toplama yok | `src/runtime/state.js:12-19` |
| Sinyal eşitliği referans | `src/runtime/state.js:14` |
| Store'da eşitlik kontrolü yok | `src/runtime/state.js:63-68` |
| `computed` | `src/runtime/state.js:115-128` |
| Global store kayıt defteri | `src/runtime/state.js:135-148` |
| `onDestroy` yalnızca `mount().destroy()` | `src/runtime/events.js:64` · `src/runtime/mount.js:390` |
| `onMount` her render'da çalışıyor | `src/runtime/events.js:59-61` · `src/runtime/mount.js:314` |
| Node kaldırma sırasında destroy yok | `src/runtime/mount.js:186-214` |
| Keyed diff O(n²) | `src/runtime/mount.js:153-181` |
| Her render'da yeni ağaç kuruluyor | `src/runtime/mount.js:299-315` |
| `head`/`title`/`meta` özel işlemi yok | `src/runtime/dom.js:198-224` |
| `value` property olarak yazılıyor | `src/runtime/dom.js:159-167` |
| `VOID_TAGS` | `src/runtime/dom.js:11-14` |
| Async bileşen `await` edilmiyor | `src/codegen/mod.rs:319` |

### Backend runtime
| Bulgu | Konum |
|---|---|
| Oturum çerezi `res.send()` sonrası kayboluyor | `src/runtime/server.js:624-631` |
| `readBody` middleware öncesi | `src/runtime/server.js:595,602` |
| `bodyLimit` etkisiz | `src/runtime/middleware.js:83-94` |
| `server.on("error")` yok | `src/runtime/server.js:695-707` |
| `X-Forwarded-*` koşulsuz güven | `src/runtime/server.js:279-291` |
| Hata mesajı sızıyor | `src/runtime/server.js:336-338` |
| WS 64-bit frame kırpılıyor | `src/runtime/server.js:441-445` |
| SSE `close` dinlenmiyor | `src/runtime/server.js:493-519,617` |
| Oturum çerezi `Secure`/`TTL` yok | `src/runtime/session.js:128-137` |
| `use()` yolu düz prefix | `src/runtime/server.js:181-186` |
| Statik yalnızca 404 fallback'i | `src/runtime/server.js:371-373,611-613` |
| `OPTIONS` route'u ölü | `src/runtime/server.js:605-610` |
| `res` API'si dar | `src/runtime/shared.js:244-357` |
| Streaming yanıt yok | `src/runtime/shared.js:209-223` |
| `cookieHeader` `httpOnly` varsayılan true | `src/runtime/shared.js:360-371` |
| `matchRoute` kuralları | `src/runtime/shared.js:396-443` |
| `http.get/post` sadece | `src/runtime/http.js:302-334` |
| `fetch` timeout + signal çakışması | `src/runtime/http.js:249-259` |
| `crypto.verify` timing attack | `src/runtime/node.js:309-310` |
| `fs` senkron | `src/runtime/node.js:156-258` |
| `readStream` gerçek stream değil | `src/runtime/node.js:208-215` |
| `uuid`/`random` `Math.random` | `src/runtime/core.js:263-280` |
| `compress` `Vary` eklemiyor | `src/runtime/middleware.js:49-67` |
| Ölü `ctx.writeAnswer` | `src/runtime/shared.js:457` |

### Araç zinciri
| Bulgu | Konum |
|---|---|
| **LSP char-boundary panic** | `src/cli/json.rs:235` |
| LSP metot listesi | `src/cli/lsp.rs:99-162` |
| ~~LSP `hint` alanı geçersiz~~ ✅ ipucu mesajın içinde, `code` eklendi | `src/cli/lsp.rs::problem` |
| LSP `didClose` yok | `src/cli/lsp.rs:94-145` |
| LSP completion konumu yok sayıyor | `src/cli/lsp.rs:139-141,239-264` |
| LSP import env'si kurulmuyor | `src/types/checker.rs:563` |
| **`-o` hedef klasörü siliyor** | `src/cli/site.rs:94-98` |
| **Lint her zaman exit 0** | `src/cli/lint.rs:107` |
| **Fmt her zaman exit 0** | `src/cli/fmt.rs:77` |
| Lint bulguları stdout'da | `src/cli/lint.rs:68,139` |
| Lint `check` uyarılarını üretmiyor | `src/cli/lint.rs` ↔ `src/check/mod.rs:63-116` |
| ~~Formatter blok yorumları saymıyor~~ ✅ `tests/cli.rs::formatter_ignores_braces_inside_multiline_block_comments` | `src/cli/fmt.rs::braces` |
| Formatter boş satırları birleştiriyor | `src/cli/fmt.rs:145-152` |
| `dev` polling 300 ms | `src/cli/dev.rs:149-156` |
| ~~`test --watch` modülleri izlemiyor~~ ✅ başarılı ve bozuk import değişiklikleri test edildi | `src/cli/run.rs::run_tests` |
| Reload script enjeksiyonu | `src/cli/dev.rs:404-413` |
| ~~404 script yolu göreli~~ ✅ özel 404 bundle'ı inline; regresyon testi `tests/lang.rs::builds_a_deployable_site` | `src/cli/site.rs::site_files` |
| Site build uyarıları düşürülüyor | `src/cli/site.rs` (`compile_for_site`) |
| Tree-shaking yalnızca `export fn` | `src/compiler/loader.rs:252-259` |
| Tree-shaking lexer sezgisi | `src/compiler/loader.rs:266-294` |
| Tek yuvalı build cache | `src/compiler/cache.rs:291-304` |
| `project_root` iki kopyada | `src/compiler/mod.rs:300-313` ↔ `src/cli/entry.rs:111-126` |
| Kaynak haritası `names` boş | `src/sourcemap/mod.rs:158` |
| Kaynak haritası mutlak yol | `src/compiler/mod.rs:96` |
| Harita inline `<script>` içinde | `src/cli/output.rs:24-35,37-40` |
| UTF-16 sütun hatası | `src/lexer/mod.rs:182-192` ↔ `src/sourcemap/mod.rs:70-73` |
| Paylaşılan argüman parser'ı | `src/cli/args.rs:43-98` |
| Config dosyası yok | `grep "vey.toml" src/` → 0 |
| Env var desteği yok | `grep "env::var" src/` → 0 |
| `vey migrate` codemod yok | `src/cli/migrate.rs:111-136` |
| ~~`vey add` JSON'u string ile düzenliyor~~ ✅ `tests/cli.rs::add_writes_a_package_name_that_already_appears_in_a_string_value` | `src/cli/add.rs::add_dependency` |
| ~~`vey add` npm hatasını yutuyor~~ ✅ `tests/cli.rs::add_fails_when_npm_cannot_install_the_package` | `src/cli/add.rs::cmd_add` |
| **Bozuk Dockerfile** | `src/cli/scaffold.rs:421-436` |
| CI şablonu test dosyası üretmiyor | `src/cli/scaffold.rs:456` |
| Test koşucusu | `src/cli/run.rs` · `src/runtime/core.js:181-253` |
| Bundle tek dosya, 187 KB | `src/runtime.rs:10-41` · `src/cli/output.rs:13-14` |

---

## 13. Sonuç

Vey'in **çekirdek taşıyıcıları sağlam**: derleyici mimarisi, modül sistemi, kaynak haritaları, önbellek, paralel üretim, tree-shaking, iyi hata mesajları, README'yi test eden testler, 16 komutlu CLI, olgun bir Node runtime'ı ve şaşırtıcı derecede yetkin bir sunucu. Rust tarafında **hiç bağımlılık yok** — her şey elle yazılmış ve bu, hem övgü hem de bakım riski.

Eksik olan şey **derinlik** değil, **genişlik ve tutarlılık**:

- **Tip tarafında** async hiç yok (`Promise` var ama fonksiyon dönüş tipi olarak kullanılamıyor), kütüphane `any` denizine dökülmüş (126 `any` kullanımı), annotation'sız her `let` `any`, callback parametreleri `any`, `unknown` pratikte hiç üretilmiyor, generic fonksiyonlar monomorfize olup **açıkça yazılan tip argümanını yok sayıyor**.
- **Frontend tarafında** reaktivite iskelet düzeyinde; `onDestroy` çalışmıyor, `onMount` her render'da tekrar çalışıyor, toplama yok, eşitlik yok, markup'ta yan etki ifadesi metin düğümü olarak basılıyor, SVG yazılamıyor, bileşen kendi durumunu tutamıyor, SSR-hydration köprüsü yok.
- **Backend tarafında** 19 P0'dan 9'u gerçek güvenlik/doğruluk hatası (oturum çerezi kaybı, `bodyLimit` etkisiz, `EADDRINUSE` crash, timing attack, IP spoofing, hata sızıntısı, WS frame kırpma, SSE donması, session fixing) ve "production hazır" özellikler (TLS, CSRF, rate limit, şema doğrulama, streaming, graceful shutdown) eksik.
- **Araç zincirinde** LSP üç özellikle sınırlı ve bir panic riski taşıyor, config dosyası yok, `vey build -o` kullanıcı dosyalarını silebiliyor, lint/fmt CI'da kullanılamaz, `vey new`'in Dockerfile'ı bozuk, "hello world" bundle'ı 187 KB.

**En kritik iki eksik — ve bunlar birbirini besliyor:**

1. **Tip tarafı** bugün kullanıcı kodunun büyük kısmını `any` sayıyor. Bu yüzden "async tip güvenliği yok" bir özellik eksiği değil, **bir güvenlik eksiği**: derleyici yanlış programı sessizce kabul ediyor.
2. **Frontend** bu boşluğu doldurmak zorunda kalıyor — kullanıcı "hangi tip ne?" diye soramayacağı için `any` alanlarla çalışmak zorunda, bu da tip denetleyicinin işini gereksizleştiriyor.

Bu yüzden **Faz 0 ve Faz 1 birlikte** en yüksek getirili iş: Faz 0 yanlış davranışları siliyor (19 P0), Faz 1 ise tip sisteminin gerçekten işe yaramasını sağlıyor. Bundan sonra gelen Faz 2 (frontend) ve Faz 3 (backend) doğru bir temel üzerine oturur.

**Pratik başlangıç önerisi (tek kişi, 1 hafta):**
1. P0-13 (isim çözümlemesi) — 1 gün, yanlış program sonucunu siliyor.
2. P0-19 (markup yan etki ifadeleri) — 1 gün, sayfaya fonksiyon kaynak kodu basılmasını siliyor.
3. P0-14 (LSP panic) — yarım gün.
4. P0-1 + P0-2 + P0-3 (sunucu) — 1 gün.
5. P0-15 + P0-16 + P0-17 (CLI) — yarım gün.
6. Async dönüş tipi (`Promise<T>`) — 2–3 gün; tip sistemine en büyük tek katkı.

Bu altı madde, dili "deneyebilir bir şey" olmaktan "güvenilir bir şey" olmaya taşır.

---

---

## 14. Bağımsız yeniden denetim (bu tur) — ileri seviye frontend ve backend

> Bu bölüm, mevcut kod tabanının (`src/`, `tests/`) yeniden satır satır okunmasıyla üretilmiştir. Amaç: mevcut raporu güncellemek değil, **0.7.0 sonrasında kalan ve ileri seviye bir dil için gerekli** eksikleri tek yerde toplamaktır. Her madde için kanıt (dosya:satır) verilmiştir. Durum etiketleri: 🔴 eksik / 🟡 kısmi / ✅ var.

### 14.0 Yönetici özeti (bu tur)

- **Test durumu:** `cargo test` → **266 test, hepsi geçiyor** (lang 62, types 43, runtime 39, dom 68, server 15, cli 23, dev 4, regressions 5, readme 3). Derleme uyarıları yok.
- **Frontend reaktivitesi** önemli ölçüde ilerledi (`signal`/`store`/`computed`/`effect`/`form`/`transition`/`portal`/`Boundary`/`provide`-`inject` var), ancak **halen eksik olanlar**: toplama (batching) yok, memo yok, async bileşen/Suspense yok, SSR→hydration köprüsü yok, router'da layout/guard/nested route yok, keyed diff O(n²), `ref` tipi yok, olay tip güvenliği yok.
- **Backend** olgun: middleware pipeline, WS, SSE, session, static, multipart, body limit var. Kalan: HTTPS/HTTP2, streaming yanıt, takılabilir session store, şema doğrulama, async fs/crypto/zlib, rate limit/CSRF/security middleware'leri **bildirilmiş ama implementasyon derinliği sınırlı**.
- **Tip sistemi** async dönüş tipi ve `Promise<T>` hâlâ fonksiyon dönüş tipi olarak kullanılamıyor (Faz 1'de "devam ediyor"). `unknown`/`never`/`symbol`/`bigint` pratikte yok. `any` hâlâ iki yönde şeffaf.
- **Araç zinciri** LSP hâlâ temel (tanım/referans/rename eksik), config dosyası kısmen var, formatter idempotency sorunu, lint 3 kural.
- **Yeni bulgu (bu tur):** `src/runtime/dom.js` içinde SVG ve HTML dışı etiketler için **tip/etiket doğrulaması** compiler tarafında var (`check`), ama runtime tarafında **bilinmeyen etiketler sessizce `document.createElement` ile oluşturuluyor** — tarayıcıda sessiz hata. Ayrıca `mount.js` keyed diff **aynı key'li düğümleri yeniden kullanmıyor** (performans). `server.js` içinde `readBody` **synchronously buffer'a alıyor** ve büyük gövdelerde bellek şişmesi riski var. `session.js` **sadece bellek içi** store; yatay ölçekleme yok.

### 14.1 Frontend — ileri seviye için eklenecekler

#### F-A. Reaktivite çekirdeği (core reactivity)
1. ~~**Batching / toplama**~~ ✅ — `batch(fn)` birden çok `set` çağrısını tek effect çalışmasına topluyor; `untrack(fn)` okunan değeri mevcut effect'e bağımlılık olarak eklemiyor. Regresyon testi: `tests/runtime.rs::batch_runs_an_effect_once_and_untrack_does_not_add_a_dependency`.
2. **Memo** — `memo(fn)` = cached computed with equality. `computed` var ama cache invalidation şeffaf değil. Kanıt: `state.js` computed.
3. **Equality politikası** — `{ equals: "deep" | fn }` var (C-2 kapandı), ama **deep equality varsayılan değil**; nesne signal'larında her set render tetikler. Eklenecek: varsayılan shallow/deep seçeneği.
4. **Effect cleanup** — `effect(fn)` cleanup fonksiyonu döndürebilsin (React useEffect tarzı). Şu an yok. Kanıt: `state.js`.
5. **Async effect / resource** — `resource(fn)` ile async veri yükleme + loading/error state. Yok.
6. **Context provider scope** — `provide`/`inject` var ama **tip güvenli değil** ve **nested provider override** test edilmemiş. Kanıt: `dom.js` provide/inject.
7. ~~**Store patch**~~ ✅ — `store.set("a.b", x)` iç içe nesneyi yapısal kopyalayarak güncelliyor ve kardeş alanları koruyor. Regresyon testi: `tests/runtime.rs::a_store_can_update_a_nested_path_without_losing_siblings`.

#### F-B. Bileşen modeli
8. **Async bileşen / Suspense** — `async component` ve `<Suspense fallback=...>` yok. Kanıt: README'de yok, runtime'da yok.
9. **Error boundary gelişmiş** — `Boundary` var ama **fallback prop tipi** ve **reset** yok. Kanıt: `dom.js` Boundary.
10. **Slot gelişmiş** — `slot` var ama **named slots with props** (scoped slots) yok.
11. **Portal gelişmiş** — `portal` var ama **target selector değişimi** ve **SSR'da portal** yok.
12. **Component lazy loading** — `lazy(() => import(...))` yok.
13. **HOC / higher-order components** — Vey'de yok; `let Box = Card` var (ad ile bileşen) ama genel HOC kalıbı yok.

#### F-C. Rendering / diff
14. 🟡 **Keyed diff performansı** — key eşleştirme Map tabanlı O(n) oldu; LIS ve iki fazlı sanal ağaç hâlâ eksik. Kanıt: `mount.js` diff.
15. **Scroll/odak koruma** — C-13 kapandı (odak+scroll hatırlanıyor). Eksik: **liste yeniden sıralamada animasyon** (FLIP).
16. **SSR → hydration** — `html()` var ama **hydration** (mevcut DOM'u devralma) yok; tarayıcıda yeniden render ediliyor. Kanıt: `mount.js`, `dom.js`.
17. **Head yönetimi** — `title()` var; `head` etiketi var. Eksik: **dinamik meta/link yönetimi** (react-helmet tarzı) ve **SSR head**.
18. **CSS-in-JS / scoped styles** — `style { }` bloğu var; scoped (bileşene özel) CSS yok. Eklenecek: `style scoped`.

#### F-D. Router
19. **Layout / nested route** — `route` var; **layout** (ortak kabuk) yok. Kanıt: `router.js`.
20. **Route guard** — `beforeEach` / auth guard yok.
21. **Nested route params** — yok.
22. **History mode** — hash vs browser history seçeneği yok (sadece browser history).
23. **Prefetch / code splitting** — yok.

#### F-E. Form
24. **Form validasyon şeması** — `form` + `check` var; **schema-based validation** (zod tarzı) yok.
25. **Async validation** — yok.
26. **Field-level validation debounce** — yok.
27. **Form submission state** (submitting/isSubmitting) — yok.

#### F-G. Olaylar ve yaşam döngüsü
28. **onDestroy güvenilirliği** — C-9 kapandı. Eksik: **async leave** (promise ile bekleme) test edilmiş mi? `leave=fn(node){ return sleep(300) }` var. Kanıt: mount.js leave handling.
29. **onMount tek sefer** — C-10 kapandı.
30. **Event tip güvenliği** — olay adları string; bilinmeyen olay adı kontrol edilmiyor. Kanıt: `events.js` on().
31. **Keyboard/focus trap** — erişilebilirlik (a11y) yardımcıları yok.

#### F-H. Erişilebilirlik ve i18n
32. **ARIA attribute kontrolü** — yok.
33. **i18n runtime** — `intl` var (format), ama **çeviri sistemi** (t() fonksiyonu, locale switching) yok.
34. **RTL desteği** — yok.

### 14.2 Backend — ileri seviye için eklenecekler

#### B-A. HTTP sunucu çekirdeği
1. **HTTPS / HTTP2** — `server()` sadece `node:http`. Eklenecek: `server(port, { tls: { cert, key } })`. Kanıt: `server.js` httpModule.
2. **Streaming yanıt** — `res.write`/`res.end` streaming yok; tüm yanıt bellekte. Eklenecek: `response.stream(fn)`.
3. **Request timeout uygulaması** — `opts.timeout` var ve uygulanıyor (server.js:772), ama **varsayılan 0** (kapalı) ve **AbortController entegrasyonu** yok; uzun süren handler'lar manuel olarak kesilemiyor. Kanıt: server.js:146,772.
4. **Keep-alive / connection pooling** — varsayılan; ayarlanabilir değil.
5. **HTTP/2 server push** — yok.

#### B-B. Middleware ve güvenlik
6. **Middleware path param / wildcard** — `use("/admin", fn)` var ama **wildcard** (`/admin/*`) yok. Kanıt: `fitsPath`.
7. **Error middleware 4-arity** — global state; Express tarzı `(err, req, res, next)` yok. Kanıt: D7.
8. **CSRF / rateLimit / security** — bildirilmiş ve var (middleware.js), ama **implementasyon derinliği** (token rotation, store-based rate limit) sınırlı. Kanıt: `middleware.js`.
9. **CORS gelişmiş** — credentials, origin callback, preflight cache yok.
10. **Compression** — gzip var; brotli yok.
11. **Helmet tarzı güvenlik başlıkları** — `security()` var; daha fazla başlık (CSP nonce, HSTS preload) yok.

#### B-C. Session ve state
12. **Takılabilir session store** — sadece bellek; Redis/DB store interface yok. Kanıt: `session.js`.
13. **Session cookie Secure/HttpOnly/SameSite** — P0-9 kapandı; **SameSite=None** (cross-site) senaryosu test edilmemiş.
14. **Distributed cache / KV** — yok.
15. **Server-side rendering cache** — yok.

#### B-D. WebSocket ve SSE
16. **WS heartbeat/ping-pong** — yok (bağlantı ölü tespiti).
17. **WS binary frames** — sadece text.
18. **WS per-message deflate** — yok.
19. **SSE reconnect / Last-Event-ID** — yok.
20. **SSE broadcast helper** — yok.

#### B-E. Veri ve dosya sistemi
21. **Async fs** — `fs.read` vb. senkron; async versiyon yok. Kanıt: `node.js`.
22. **fs.watch** — dosya izleme yok (dev server polling kullanıyor).
23. **Stream read/write** — `fs.readStream`/`writeStream` var (README'de), implementasyon kontrol edilmeli.
24. **Database client** — yok (npm ile kullanılabilir).
25. **Query builder / ORM** — yok.

#### B-F. Crypto ve zlib
26. **Async crypto** — senkron; büyük veride event loop bloklanır.
27. **zlib streaming** — yok.
28. **JWT helper** — yok (crypto ile yapılabilir ama helper yok).
29. **Password hashing** (bcrypt/argon2) — yok.

#### B-G. HTTP istemcisi
30. **http.put/patch/delete/head** — README'de yok; sadece get/post. Kanıt: builtins.rs / node.js.
31. **Retry / timeout / abort** — `http.fetch` var; retry politikası yok.
32. **Interceptors** — yok.

#### B-H. Process ve env
33. **env() validation** — yok.
34. **process.spawn streaming** — var; **stdio pipe** kontrolü sınırlı.
35. **Worker threads** — yok.

### 14.3 Tip sistemi — ileri seviye için eklenecekler

1. **Async dönüş tipi** — `Promise<T>` fonksiyon dönüş tipi olarak kullanılamıyor (Faz 1). Kanıt: A1.
2. **`unknown` / `never` / `symbol` / `bigint`** — pratikte yok (B-15).
3. **`satisfies`** — yok (B-14).
4. **Discriminated union narrowing** — yok (B-6).
5. **Variance / in-out** — yok (B-3).
6. **Conditional types** — yok (B-19).
7. **Mapped types** — yok (B-19).
8. **Template literal types** — yok (B-19).
9. **Overload** — yok.
10. **`any` iki yönde şeffaf** — B-10; `unknown` ile değiştirilmeli.
11. **Callback contextual typing** — yok (B-8).
12. **Generic variance** — yok.
13. **Type guards (user-defined)** — `is` ile narrowing var ama **custom type guard fonksiyonu** (`fn isUser(x): x is User`) yok.
14. **Exhaustiveness checking** — enum match'te var; union'da yok (B-5).
15. **Nominal typing** — yok (structural only).
16. **Branded types** — yok.

### 14.4 Araç zinciri — ileri seviye için eklenecekler

1. **LSP: go-to-definition, references, rename, hover type** — temel var; rename/definition eksik (E1).
2. **LSP: code actions / quick fix** — yok.
3. **LSP: inlay hints** — yok.
4. **Config dosyası** — `vey.toml` var; **tam destek** (tüm seçenekler) yok (E7).
5. **Formatter idempotency** — bozuk (E9).
6. **Linter kuralları** — 3 kural; daha fazlası (unused import, dead code, complexity) yok (E10).
7. **Test runner gelişmiş** — watch, coverage, parallel, snapshot yok (E8).
8. **Dev server** — polling yerine fs.watch, HMR (hot module replacement) yok (E4/E5).
9. **Build: minify, tree-shake, code split** — minify var; code split yok.
10. **Deploy** — stub (E14).
11. **Docs generation** — doc comment → HTML var; **API doc site** yok.
12. **Migrate** — codemod yok (E13).
13. **Telemetry / crash reporting** — yok (E18).
14. **Package manager entegrasyonu** — `vey add` var; **lockfile, registry** yok.

### 14.5 Öncelik matrisi (bu tur)

| Öncelik | Madde | Etki | Efor |
|---|---|---|---|
| P0 | Async dönüş tipi (Promise<T>) | Çok yüksek | 3 gün |
| P0 | Batching (batch/untrack) | Yüksek | 2 gün |
| P0 | SSR → hydration | Yüksek | 5 gün |
| P1 | HTTPS + streaming yanıt | Yüksek | 3 gün |
| P1 | Memo + effect cleanup | Orta | 2 gün |
| P1 | LSP go-to-def/rename | Orta | 4 gün |
| P1 | Async fs/crypto | Orta | 3 gün |
| P2 | Router layout/guard | Orta | 4 gün |
| P2 | Form validation schema | Orta | 3 gün |
| P2 | i18n runtime | Düşük | 5 gün |
| P3 | HTTP/2, WS binary, SSE reconnect | Düşük | 6 gün |

### 14.6 Kanıt indeksi (bu tur)

- `src/runtime/state.js` — signal/store/computed/effect implementasyonu; batching yok, cleanup yok.
- `src/runtime/mount.js` — keyed diff, onMount/onDestroy kuyruğu, scroll/odak koruma (C-13).
- `src/runtime/dom.js` — provide/inject, portal, Boundary, transition, form binding.
- `src/runtime/events.js` — on/delegate/onMount/onDestroy/ref.
- `src/runtime/router.js` — route/navigate; layout/guard yok.
- `src/runtime/server.js` — readBody, findRoute, fitsPath, allowedMethods; HTTPS/streaming yok.
- `src/runtime/session.js` — bellek içi store.
- `src/runtime/middleware.js` — logger/cors/compress/errorHandler/bodyLimit/security/rateLimit/csrf.
- `src/runtime/node.js` — fs/path/crypto/zlib/process/env (çoğu senkron).
- `src/runtime/http.js` — fetch; put/patch/delete yok.
- `src/types/builtins.rs` — runtime helper imzaları; Promise<T> async dönüş tipi eksik.
- `src/types/checker.rs`, `src/types/exprs.rs` — tip çıkarımı; `any` şeffaflığı.
- `src/cli/lsp.rs` — LSP implementasyonu; go-to-def/rename eksik.
- `src/cli/fmt.rs`, `src/cli/lint.rs` — formatter/linter.
- `src/cli/dev.rs` — polling watcher.

### 14.7 Sonuç (bu tur)

Vey 0.7.0 itibarıyla **"deneyebilir" bir temelden "güvenilir" bir temele** geçti: 19 P0 kapatıldı, 266 test geçiyor, frontend reaktivitesi temel seviyede çalışıyor, backend production'a yakın. Bu turun tespit ettiği eksikler **derinlik ve genişlik** eksikleri: batching/memo/Suspense/hydration (frontend), HTTPS/streaming/async IO/session store (backend), async tip güvenliği ve modern tip özellikleri (tip sistemi), LSP/formatter/linter/test runner (araç zinciri).

**Önerilen sıradaki adım:** Faz 1'i bitir (async dönüş tipi + `unknown`/`never` + contextual typing), çünkü tip sistemi güçlenmeden frontend/backend'deki gelişmiş özellikler tip güvenli kullanılamaz. Ardından Faz 2 (batching + hydration) ve Faz 3 (HTTPS + streaming + async IO).
