mkdir -p "$HOME/.local/opt/eder" \
         "$HOME/.local/share/applications" \
         "$HOME/.local/share/icons/hicolor/scalable/apps"

cp target/appimage/eder-*.AppImage "$HOME/.local/opt/eder/eder.AppImage"
chmod +x "$HOME/.local/opt/eder/eder.AppImage"
cp packaging/appimage/eder.svg "$HOME/.local/share/icons/hicolor/scalable/apps/eder.svg"

cat > "$HOME/.local/share/applications/eder.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Eder — PDF Library
Comment=Browse and read a local PDF library
Exec=$HOME/.local/opt/eder/eder.AppImage
Icon=$HOME/.local/share/icons/hicolor/scalable/apps/eder.svg
Terminal=false
Categories=Office;Viewer;
EOF