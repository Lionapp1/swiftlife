# SwiftLife

SwiftLife artık tamamen **native C++ + GTK3 + WebKitGTK 4.1** tabanlı, yalnızca Linux için geliştirilen masaüstü web tarayıcısıdır.

## Mimari

```text
SwiftLife UI (GTK3)
        │
        ▼
Browser Core (C++)
        │
        ▼
WebKitGTK 4.1
        │
        ▼
WebKit / JavaScriptCore + GTK rendering stack
```

Rust/Wry/Winit katmanı kaldırıldı. Tarayıcı penceresi, adres çubuğu, gezinme ve WebView artık doğrudan C++ tarafında yönetiliyor.

## Özellikler

- Native GTK arayüz
- WebKitGTK gerçek web motoru
- Geri / ileri / yenile / ana sayfa
- URL veya Google araması
- Kalıcı profil, cache ve oturum
- İndirme klasörü yönetimi
- HTTPS/HTTP gezinme kontrolü
- `target="_blank"` isteklerini mevcut görünümde açma
- Donanım hızlandırmalı WebKit rendering altyapısı
- Linux masaüstü entegrasyonu
- Release CMake derlemesi

## Gereksinimler

Ubuntu/Debian:

```bash
sudo apt update
sudo apt install -y build-essential cmake pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev
```

## Derleme

```bash
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --parallel
./build/swiftlife
```

## Profil

SwiftLife kullanıcı verilerini `~/.local/share/SwiftLife/` altında tutar. WebKitGTK profil ve cache'i ayrı olarak saklanır; böylece tarayıcı oturumu yeniden başlatıldığında korunur.

## Lisans

MIT
