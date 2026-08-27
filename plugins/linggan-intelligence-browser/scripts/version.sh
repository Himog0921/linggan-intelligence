#!/bin/bash
# 灵感爆爆爆 - 版本管理脚本
# 用法: ./scripts/version.sh [patch|minor|major] [描述信息]
# 示例: ./scripts/version.sh patch "修复评论采集bug"
#       ./scripts/version.sh minor "新增批量采集功能"

set -e

cd "$(dirname "$0")/.."

TYPE=${1:-patch}
MSG=${2:-"版本更新"}

# 读取当前版本
CURRENT=$(node -p "require('./package.json').version")
echo "当前版本: v$CURRENT"

# 计算新版本
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"
case $TYPE in
  major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
  minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
  patch) PATCH=$((PATCH + 1)) ;;
  *) echo "用法: $0 [patch|minor|major] [描述]"; exit 1 ;;
esac
NEW="$MAJOR.$MINOR.$PATCH"
echo "新版本:   v$NEW"

# 同步版本号到 package.json、package-lock.json 和 manifest.json
node -e "
const fs = require('fs');
const files = ['package.json', 'package-lock.json', 'manifest.json'];
for (const f of files) {
  const data = JSON.parse(fs.readFileSync(f, 'utf8'));
  data.version = '$NEW';
  if (f === 'package-lock.json' && data.packages && data.packages['']) {
    data.packages[''].version = '$NEW';
  }
  fs.writeFileSync(f, JSON.stringify(data, null, 2) + '\n');
  console.log('  ✓ ' + f + ' -> v$NEW');
}
"

# 构建
echo "构建中..."
npm run build --silent
echo "  ✓ 构建完成"

# 打包 dist 为 zip（方便分发）
ZIP_NAME="linggan-boom-v${NEW}.zip"
mkdir -p releases
rm -f "releases/$ZIP_NAME"
cd dist && zip -r -q "../releases/$ZIP_NAME" . && cd ..
echo "  ✓ 打包: releases/$ZIP_NAME"

node scripts/verify-release-package.mjs --version "$NEW" --zip "releases/$ZIP_NAME"

# Git 提交 + 打 tag
git add package.json package-lock.json manifest.json
git commit -m "release: v$NEW — $MSG"
git tag -a "v$NEW" -m "v$NEW: $MSG"

echo ""
echo "✅ 发布完成: v$NEW"
echo "   提交已创建，tag 已打好"
echo "   安装包: releases/$ZIP_NAME"
echo ""
echo "如需推送: git push && git push --tags"
