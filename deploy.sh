#!/bin/bash
set -e

TARGET="aarch64-unknown-linux-gnu"
HOST="root@192.168.1.42"
BINARY="joyboard"

echo "==> 编译 ($TARGET)..."
cargo build --target "$TARGET" --release --features web

echo "==> 剥离符号..."
aarch64-linux-gnu-strip "target/$TARGET/release/$BINARY"

echo "==> 部署二进制到 $HOST:/usr/local/bin/"
sshpass -p root scp -o StrictHostKeyChecking=no \
    "target/$TARGET/release/$BINARY" \
    "$HOST:/usr/local/bin/"

echo "==> 部署 web 资源到 $HOST:/usr/local/share/joyboard/web/"
sshpass -p root ssh -o StrictHostKeyChecking=no "$HOST" "mkdir -p /usr/local/share/joyboard/web/"
sshpass -p root scp -o StrictHostKeyChecking=no \
    web/index.html \
    "$HOST:/usr/local/share/joyboard/web/index.html"

echo "==> 完成! 二进制 $(ls -lh target/$TARGET/release/$BINARY | awk '{print $5}')"
echo ""
echo "使用方式:"
echo "  ssh root@192.168.1.42 joyboard evtest /dev/input/event1    # 调试模式"
echo "  ssh root@192.168.1.42 joyboard                              # 后台 daemon"
echo "  ssh root@192.168.1.42 joyboard tui                          # 终端 UI（新窗口）"
echo "  ssh root@192.168.1.42 joyboard serve                        # 配置面板"
echo ""
echo "浮窗 UI (overlay) 需要在设备上本地编译:"
echo "  ssh root@192.168.1.42"
echo "  cd /tmp && mkdir -p joyboard-overlay && cd joyboard-overlay"
echo "  # 复制本机源码到设备, 或使用:"
echo "  sudo apt install libgtk-3-dev"
echo "  # 编译: cargo build --release --features overlay"
echo "  # 运行: ./target/release/joyboard overlay"
