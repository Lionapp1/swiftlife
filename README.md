# SwiftLife

SwiftLife, Rust ve yerel WebView motorları üzerine kurulmuş hızlı ve modern bir masaüstü web tarayıcısıdır.

## Özellikler

- Baştan sona Türkçe, sade ve modern arayüz
- Geri / ileri / yenile / ana sayfa kontrolleri
- Adres çubuğu: URL veya doğrudan arama
- Gerçek web sayfalarını yerel WebView içinde açma
- `target="_blank"` ve `window.open()` isteklerini mevcut görünümde açma
- Sistem sağ tık menüsü ve sayfa etkileşimleri
- Panoya erişim, medya otomatik oynatma ve tarayıcı yakınlaştırma kısayolları
- Pencere boyutuna göre otomatik WebView yerleşimi
- Hafif Rust ana süreç; Chromium/Electron paketlemesi yok
- Geliştirici araçları desteği (debug derlemelerinde)

## Teknoloji

- Rust 2021
- Wry 0.56
- Winit 0.30
- Windows WebView2 / macOS WKWebView / Linux WebKitGTK

## Çalıştırma

```bash
cargo run
```

Üretim derlemesi:

```bash
cargo build --release
```

### Linux notu

Wry'nin çocuk WebView yaklaşımı Linux'ta X11 ile desteklenir. Wayland için WebKitGTK/GTK tabanlı entegrasyon gerekir.

## Lisans

MIT

<!-- runtime optimization trigger -->
