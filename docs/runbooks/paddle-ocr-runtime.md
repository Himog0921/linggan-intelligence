# Paddle OCR 本机运行时

> 状态: 权威当前
> 最后核对: 2026-09-20
> 适用范围: `linggan-media-worker` 的本地 PaddleOCR 依赖、启动前检查与安全边界
> 事实来源: OCR-CONTENT-LAYERING-001、`paddle_ocr.py` bridge、`prepare-paddle-ocr.sh`
> 冲突时以谁为准: 用户最新授权、AGENTS.md、当前 worker 源码与真实运行回执

## 目的与边界

图片识字只由本机 PaddleOCR v4 执行。安装步骤不会执行数据库 migration、重排历史作业、重启 worker、访问平台或调用视觉模型。

运行 `scripts/runtime/prepare-paddle-ocr.sh` 会在 `LINGGAN_SUPPORT_DIR/paddle-ocr/venv`（默认 Application Support）安装并校验固定版本：Paddle 3.3.1、PaddleOCR 3.7.0、Pillow 10.x–11.x。首次运行会下载 Python wheels 与 Paddle 官方模型；这是本机依赖安装，不是平台采集。

## Worker 配置

- 默认 Python：`$LINGGAN_SUPPORT_DIR/paddle-ocr/venv/bin/python`。
- 可仅为受控诊断覆写 `LINGGAN_PADDLE_OCR_PYTHON`。
- bridge 路径默认为 worker 包内 `media_worker_support/paddle_ocr.py`；可用 `LINGGAN_PADDLE_OCR_SCRIPT` 指向同版本审计副本。
- worker 只有在 Python 可执行且 bridge 文件存在时声明 `image_ocr` / `video_frame_ocr` 能力；否则不认领这些作业，不以 Tesseract 后备。

## 发布后的有界步骤

1. 在新 runtime checkout 准备 Paddle 环境，并核对脚本打印的三个版本。
2. 应用已批准的 `0091_ocr_content_layering`；它只退役历史 Tesseract OCR 的可读性，不删除原始媒体或 Evidence。
3. 先执行 `linggan-media-requeue --kinds image_ocr,video_frame_ocr --dry-run`，核对数量，再由有授权的操作者去掉 `--dry-run`。
4. 启动媒体 worker，确认它声明 OCR 能力且新的 job 为 `local-v3`；抽查 raw、layout、分层与“封面 OCR”标题来源。

以上每一步都需要相应的部署/数据库/重排授权；源码合并本身不触发任何一步。
