#!/usr/bin/env node
/**
 * 代码监视审核器
 * 监控 src 目录变化，自动触发代码质量审核和构建验证
 */

const fs = require('fs');
const path = require('path');
const { execSync, spawn } = require('child_process');

const SRC_DIR = path.join(__dirname, '..', 'src');
const DEBOUNCE_MS = 1000; // 防抖延迟

let debounceTimer = null;
let lastChangeTime = 0;

// 颜色输出
const colors = {
  reset: '\x1b[0m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m',
};

function log(color, prefix, message) {
  console.log(`${colors[color]}[${prefix}]${colors.reset} ${message}`);
}

// 代码质量检查规则
const QUALITY_RULES = [
  {
    name: '硬编码选择器检查',
    pattern: /querySelector\s*\(\s*['"`][^'"`]+['"`]\s*\)/g,
    check: (content, file) => {
      // 检查是否有未通过 SELECTORS 常量的硬编码选择器
      const lines = content.split('\n');
      const issues = [];
      lines.forEach((line, idx) => {
        if (line.includes('querySelector') && !line.includes('SELECTORS') && !line.includes('// ok:')) {
          // 检查是否是合理的例外（如通配符、简单标签）
          const match = line.match(/querySelector\s*\(\s*['"`]([^'"`]+)['"`]\s*\)/);
          if (match && match[1].length > 10 && !match[1].startsWith('.')) {
            issues.push(`L${idx + 1}: 可能的硬编码选择器 "${match[1].substring(0, 30)}..."`);
          }
        }
      });
      return issues;
    }
  },
  {
    name: 'TODO/FIXME 检查',
    pattern: /(TODO|FIXME|XXX|HACK):/gi,
    check: (content, file) => {
      const matches = content.match(/(TODO|FIXME|XXX|HACK):/gi);
      return matches ? [`发现 ${matches.length} 个待办标记`] : [];
    }
  },
  {
    name: 'console.log 检查',
    pattern: /console\.(log|warn|error)\s*\(/g,
    check: (content, file) => {
      // 排除有意义的错误日志
      const lines = content.split('\n');
      const issues = [];
      lines.forEach((line, idx) => {
        if (line.includes('console.log') && !line.includes('// debug:') && !line.includes('DEBUG')) {
          issues.push(`L${idx + 1}: console.log 可能需要移除或改为条件日志`);
        }
      });
      return issues.slice(0, 3); // 最多显示3个
    }
  },
  {
    name: '错误处理检查',
    pattern: /async\s+\w+\s*\([^)]*\)\s*\{[^}]*\bawait\b[^}]*\}/g,
    check: (content, file) => {
      // 检查 async 函数是否有 try-catch
      const asyncFuncs = content.match(/async\s+\w+\s*\([^)]*\)\s*\{/g) || [];
      const tryCatches = (content.match(/try\s*\{/g) || []).length;
      const issues = [];
      if (asyncFuncs.length > 0 && tryCatches === 0 && content.includes('await')) {
        issues.push(`有 ${asyncFuncs.length} 个 async 函数但缺少 try-catch`);
      }
      return issues;
    }
  },
  {
    name: '魔法数字检查',
    check: (content, file) => {
      const lines = content.split('\n');
      const issues = [];
      lines.forEach((line, idx) => {
        // 检查明显的魔法数字（超时、延迟等）
        const match = line.match(/setTimeout\s*\(\s*[^,]+,\s*(\d{4,})\s*\)/);
        if (match) {
          issues.push(`L${idx + 1}: setTimeout ${match[1]}ms 建议提取为常量`);
        }
      });
      return issues.slice(0, 2);
    }
  }
];

// 审核单个文件
function reviewFile(filePath) {
  const relativePath = path.relative(path.join(__dirname, '..'), filePath);
  const results = {
    file: relativePath,
    passed: true,
    issues: []
  };

  try {
    const content = fs.readFileSync(filePath, 'utf-8');

    QUALITY_RULES.forEach(rule => {
      const issues = rule.check(content, filePath);
      if (issues.length > 0) {
        results.passed = false;
        results.issues.push({
          rule: rule.name,
          details: issues
        });
      }
    });
  } catch (err) {
    results.issues.push({
      rule: '文件读取错误',
      details: [err.message]
    });
    results.passed = false;
  }

  return results;
}

// 构建验证
function verifyBuild() {
  log('cyan', '构建', '正在验证构建...');
  try {
    execSync('npm run build', {
      cwd: path.join(__dirname, '..'),
      stdio: 'pipe',
      timeout: 60000
    });
    log('green', '构建', '构建成功 ✓');
    return { passed: true };
  } catch (err) {
    log('red', '构建', '构建失败 ✗');
    const output = err.stdout?.toString() || err.stderr?.toString() || err.message;
    return {
      passed: false,
      error: output.split('\n').slice(-10).join('\n')
    };
  }
}

// 功能完整性检查（基于 TEST_CHECKLIST.md）
function checkFeatureCompleteness(changedFiles) {
  const checklistPath = path.join(__dirname, '..', 'docs', 'product', 'TEST_CHECKLIST.md');
  const suggestions = [];

  try {
    const checklist = fs.readFileSync(checklistPath, 'utf-8');

    // 根据修改的文件类型，提示相关的验收项
    changedFiles.forEach(file => {
      const relativePath = path.relative(path.join(__dirname, '..'), file);

      if (relativePath.includes('/content/')) {
        if (!suggestions.includes('笔记采集')) suggestions.push('笔记采集 (验收清单 #1)');
        if (!suggestions.includes('评论采集')) suggestions.push('评论采集 (验收清单 #2)');
        if (!suggestions.includes('批量')) suggestions.push('批量采集 (验收清单 #4-5)');
      }
      if (relativePath.includes('/dashboard/')) {
        suggestions.push('Dashboard (验收清单 #6)');
      }
      if (relativePath.includes('antiDetect')) {
        suggestions.push('反检测 (验收清单 #8)');
      }
      if (relativePath.includes('/platforms/')) {
        suggestions.push('抖音回归 (验收清单 #10-11)');
      }
    });
  } catch (err) {
    // checklist 不存在，跳过
  }

  return suggestions;
}

// 执行完整审核
function runReview(changedFiles) {
  console.log('\n' + '='.repeat(60));
  log('blue', '审核', `开始审核 ${changedFiles.length} 个文件`);
  console.log('='.repeat(60) + '\n');

  const startTime = Date.now();
  const results = {
    files: [],
    build: null,
    suggestions: [],
    summary: {
      total: changedFiles.length,
      passed: 0,
      failed: 0
    }
  };

  // 1. 代码质量审核
  changedFiles.forEach(file => {
    if (file.endsWith('.js') || file.endsWith('.ts')) {
      const result = reviewFile(file);
      results.files.push(result);
      if (result.passed) {
        results.summary.passed++;
        log('green', '质量', `${result.file} ✓`);
      } else {
        results.summary.failed++;
        log('yellow', '质量', `${result.file} 有问题:`);
        result.issues.forEach(issue => {
          console.log(`    - ${issue.rule}: ${issue.details.join(', ')}`);
        });
      }
    }
  });

  // 2. 构建验证
  results.build = verifyBuild();

  // 3. 功能完整性建议
  results.suggestions = checkFeatureCompleteness(changedFiles);
  if (results.suggestions.length > 0) {
    console.log('\n' + '-'.repeat(60));
    log('cyan', '验收', '建议验收以下功能:');
    results.suggestions.forEach(s => console.log(`  • ${s}`));
  }

  // 4. 总结
  const duration = ((Date.now() - startTime) / 1000).toFixed(1);
  console.log('\n' + '=''.repeat(60));
  if (results.summary.failed === 0 && results.build.passed) {
    log('green', '结果', `审核通过 (${duration}s)`);
  } else {
    log('yellow', '结果', `审核完成，有 ${results.summary.failed} 个文件需关注 (${duration}s)`);
  }
  console.log('='.repeat(60) + '\n');

  return results;
}

// 收集所有 JS/TS 文件
function collectSourceFiles(dir, files = []) {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  entries.forEach(entry => {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      collectSourceFiles(fullPath, files);
    } else if (entry.name.endsWith('.js') || entry.name.endsWith('.ts')) {
      files.push(fullPath);
    }
  });
  return files;
}

// 监视器主循环
function startWatcher() {
  console.log('\n' + '🔍 灵感爆爆爆 - 代码监视审核器');
  console.log('='.repeat(60));
  console.log(`监视目录: ${SRC_DIR}`);
  console.log(`审核内容: 代码质量 + 构建验证 + 功能完整性`);
  console.log('按 Ctrl+C 停止监视\n');

  // 初始审核
  const allFiles = collectSourceFiles(SRC_DIR);
  runReview(allFiles);

  // 开始监视
  const watcher = fs.watch(SRC_DIR, { recursive: true }, (eventType, filename) => {
    if (!filename || !filename.match(/\.(js|ts|css|json)$/)) return;

    const now = Date.now();
    if (now - lastChangeTime < DEBOUNCE_MS) return;
    lastChangeTime = now;

    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      const changedFile = path.join(SRC_DIR, filename);
      if (fs.existsSync(changedFile)) {
        log('blue', '变化', `检测到文件变化: ${filename}`);
        runReview([changedFile]);
      }
    }, DEBOUNCE_MS);
  });

  watcher.on('error', (err) => {
    log('red', '错误', `监视器错误: ${err.message}`);
  });

  // 优雅退出
  process.on('SIGINT', () => {
    console.log('\n');
    log('cyan', '停止', '监视器已停止');
    watcher.close();
    process.exit(0);
  });
}

// 启动
startWatcher();
