#!/bin/bash
# Build script for creating a macOS application bundle

set -e

APP_NAME="MosaicTerm"
BUNDLE_ID="com.mosaicterm.app"
VERSION="0.1.0"
BINARY_NAME="mosaicterm"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check for icon source (argument, icon.png, or existing AppIcon.icns)
SOURCE_IMAGE=""
if [ $# -ge 1 ]; then
    SOURCE_IMAGE="$1"
    if [ ! -f "$SOURCE_IMAGE" ]; then
        echo -e "${YELLOW}Warning: Source image '$SOURCE_IMAGE' not found. Will look for icon.png${NC}"
        SOURCE_IMAGE=""
    fi
fi

# If no argument provided, check for icon.png in project root
if [ -z "$SOURCE_IMAGE" ] && [ -f "icon.png" ]; then
    SOURCE_IMAGE="icon.png"
fi

echo -e "${BLUE}Building MosaicTerm for macOS...${NC}"

# Build the release binary
cargo build --release

# Create app bundle structure
APP_DIR="target/release/${APP_NAME}.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"

echo -e "${BLUE}Creating app bundle structure...${NC}"
mkdir -p "${MACOS_DIR}"
mkdir -p "${RESOURCES_DIR}"

# Copy binary
echo -e "${BLUE}Copying binary...${NC}"
cp "target/release/${BINARY_NAME}" "${MACOS_DIR}/${APP_NAME}"

# Create Info.plist
echo -e "${BLUE}Creating Info.plist...${NC}"
cat > "${CONTENTS_DIR}/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleExecutable</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.13</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSRequiresAquaSystemAppearance</key>
    <false/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>LSUIElement</key>
    <false/>
    <key>LSEnvironment</key>
    <dict>
        <key>TERM</key>
        <string>xterm-256color</string>
        <key>LANG</key>
        <string>en_US.UTF-8</string>
        <key>LC_ALL</key>
        <string>en_US.UTF-8</string>
    </dict>
</dict>
</plist>
EOF

# Create PkgInfo
echo -e "${BLUE}Creating PkgInfo...${NC}"
echo -n "APPL????" > "${CONTENTS_DIR}/PkgInfo"

# Handle icon creation/copying
if [ -n "$SOURCE_IMAGE" ]; then
    # Create icon from provided source image
    echo -e "${BLUE}Creating app icon from $SOURCE_IMAGE...${NC}"
    
    ICONSET="/tmp/MosaicTerm.iconset"
    rm -rf "$ICONSET"
    mkdir -p "$ICONSET"
    
    # Generate all required icon sizes using sips (silently)
    sips -z 16 16     "$SOURCE_IMAGE" --out "${ICONSET}/icon_16x16.png" >/dev/null 2>&1
    sips -z 32 32     "$SOURCE_IMAGE" --out "${ICONSET}/icon_16x16@2x.png" >/dev/null 2>&1
    sips -z 32 32     "$SOURCE_IMAGE" --out "${ICONSET}/icon_32x32.png" >/dev/null 2>&1
    sips -z 64 64     "$SOURCE_IMAGE" --out "${ICONSET}/icon_32x32@2x.png" >/dev/null 2>&1
    sips -z 128 128   "$SOURCE_IMAGE" --out "${ICONSET}/icon_128x128.png" >/dev/null 2>&1
    sips -z 256 256   "$SOURCE_IMAGE" --out "${ICONSET}/icon_128x128@2x.png" >/dev/null 2>&1
    sips -z 256 256   "$SOURCE_IMAGE" --out "${ICONSET}/icon_256x256.png" >/dev/null 2>&1
    sips -z 512 512   "$SOURCE_IMAGE" --out "${ICONSET}/icon_256x256@2x.png" >/dev/null 2>&1
    sips -z 512 512   "$SOURCE_IMAGE" --out "${ICONSET}/icon_512x512.png" >/dev/null 2>&1
    sips -z 1024 1024 "$SOURCE_IMAGE" --out "${ICONSET}/icon_512x512@2x.png" >/dev/null 2>&1
    
    # Verify all required sizes were created
    REQUIRED_ICONS=(
        "icon_16x16.png"
        "icon_16x16@2x.png"
        "icon_32x32.png"
        "icon_32x32@2x.png"
        "icon_128x128.png"
        "icon_128x128@2x.png"
        "icon_256x256.png"
        "icon_256x256@2x.png"
        "icon_512x512.png"
        "icon_512x512@2x.png"
    )
    
    ALL_PRESENT=true
    for icon in "${REQUIRED_ICONS[@]}"; do
        if [ ! -f "${ICONSET}/${icon}" ]; then
            echo -e "${YELLOW}Warning: Missing ${icon}${NC}"
            ALL_PRESENT=false
        fi
    done
    
    # Convert to icns only if all icons are present
    if [ "$ALL_PRESENT" = true ]; then
        if iconutil -c icns "$ICONSET" -o "${RESOURCES_DIR}/AppIcon.icns"; then
            echo -e "${GREEN}✓ App icon created successfully${NC}"
        else
            echo -e "${YELLOW}Warning: iconutil failed to create .icns file${NC}"
        fi
    else
        echo -e "${YELLOW}Warning: Could not create all icon sizes, skipping icon creation${NC}"
    fi
    
    # Clean up
    rm -rf "$ICONSET"
    
elif [ -f "resources/AppIcon.icns" ]; then
    echo -e "${BLUE}Copying app icon from resources/...${NC}"
    cp "resources/AppIcon.icns" "${RESOURCES_DIR}/AppIcon.icns"
    echo -e "${GREEN}✓ App icon copied${NC}"
    
elif [ -f "AppIcon.icns" ]; then
    echo -e "${BLUE}Copying app icon from project root...${NC}"
    cp "AppIcon.icns" "${RESOURCES_DIR}/AppIcon.icns"
    echo -e "${GREEN}✓ App icon copied${NC}"
    
else
    echo -e "${YELLOW}No app icon found. App will use the default macOS icon.${NC}"
    echo -e "${BLUE}To add a custom icon:${NC}"
    echo "  1. Save a PNG as 'icon.png' in the project root"
    echo "  2. Or run: ./build-macos-app.sh your-icon.png"
    echo "  3. Or place AppIcon.icns in the project root"
fi

# Make the binary executable
chmod +x "${MACOS_DIR}/${APP_NAME}"

# Refresh macOS icon cache by touching the app bundle
touch "${APP_DIR}"

# Reset Launch Services database to ensure icon shows up
# This forces macOS to re-read the app's Info.plist and icon
if command -v /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister &> /dev/null; then
    /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "${APP_DIR}" 2>/dev/null || true
fi

echo ""
echo -e "${GREEN}✓ App bundle created at: ${APP_DIR}${NC}"
echo ""
echo -e "${BLUE}To run the app:${NC}"
echo "  open ${APP_DIR}"
echo ""
echo -e "${BLUE}To install to /Applications:${NC}"
echo "  cp -r ${APP_DIR} /Applications/"
echo ""
echo -e "${BLUE}For production distribution:${NC}"
echo "  1. Sign: codesign --force --deep --sign - ${APP_DIR}"
echo "  2. Create DMG: hdiutil create -volname MosaicTerm -srcfolder ${APP_DIR} -ov -format UDZO MosaicTerm.dmg"
echo "  3. Notarize (requires Apple Developer account)"
echo ""
if [ -z "$SOURCE_IMAGE" ] && [ ! -f "AppIcon.icns" ] && [ ! -f "resources/AppIcon.icns" ]; then
    echo -e "${YELLOW}Tip: To add a custom icon, run:${NC}"
    echo "  ./build-macos-app.sh your-icon.png"
fi

