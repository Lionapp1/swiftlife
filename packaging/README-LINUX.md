# SwiftLife — Linux

## Tek dosya ile çalıştırma

GitHub Actions, `main` dalındaki her başarılı değişiklikte x86_64 Linux için `SwiftLife-x86_64.AppImage` üretir.

AppImage'ı indirdikten sonra:

1. Dosyaya sağ tık → **Özellikler** → **Çalıştırılabilir** seçeneğini etkinleştirin.
2. Dosyaya çift tıklayın.
3. SwiftLife doğrudan açılır; kurulum gerekmez.

## Kaynak koddan çalıştırma

Ubuntu/Debian:

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev
cargo run --release
```

Arch/Manjaro:

```bash
sudo pacman -S webkit2gtk-4.1
cargo run --release
```

Fedora:

```bash
sudo dnf install gtk3-devel webkit2gtk4.1-devel
cargo run --release
```

### Not

SwiftLife Linux'ta Wry + WebKitGTK kullanır. Child WebView mimarisi nedeniyle X11 oturumları hedeflenir; Wayland üzerinde çalıştırmak dağıtıma ve WebKit/GTK yapılandırmasına bağlı olabilir.
