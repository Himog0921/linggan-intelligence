# Paddle OCR 本机运行时

> 状态: 权威当前
> 最后核对: 2026-10-26
> 适用范围: `linggan-media-worker` 的本地 PaddleOCR 依赖、启动前检查与安全边界
> 事实来源: OCR-CONTENT-LAYERING-001、`paddle_ocr.py` bridge、`prepare-paddle-ocr.sh`
> 冲突时以谁为准: 用户最新授权、AGENTS.md、当前 worker 源码与真实运行回执

## 目的与边界

当前保留本机 PaddleOCR v4 日常路径、已有本地 OCR-VL 路径，以及单独受控的 DeepSeek-v4-flash 图片 OCR 接口。安装步骤不会执行数据库 migration、重排历史作业、重启 worker、访问平台或调用视觉模型。

运行 `scripts/runtime/prepare-paddle-ocr.sh --install` 会在 `LINGGAN_SUPPORT_DIR/paddle-ocr/venv`（默认 Application Support）安装并校验固定版本：Paddle 3.3.1、PaddleOCR 3.7.0、Pillow 10.x–11.x。脚本只接受 Python 3.9–3.13，优先使用 Homebrew 的 3.13/3.12；可用 `LINGGAN_PADDLE_PYTHON_COMMAND` 指定兼容解释器。首次安装会下载 Python wheels 与两份 PP-OCRv4 mobile 模型到同一 Application Support runtime；`--check` 只验证已安装解释器、版本和模型文件，不联网。这是本机依赖安装，不是平台采集。

## Worker 配置

- 默认 Python：`$LINGGAN_SUPPORT_DIR/paddle-ocr/venv/bin/python`。
- 可仅为受控诊断覆写 `LINGGAN_PADDLE_OCR_PYTHON`。
- bridge 路径默认为 worker 包内 `media_worker_support/paddle_ocr.py`；可用 `LINGGAN_PADDLE_OCR_SCRIPT` 指向同版本审计副本。
- worker 只有在 Python 可执行且 bridge 文件存在时声明 `image_ocr` / `video_frame_ocr` 能力；否则不认领这些作业，不以 Tesseract 后备。

## 发布后的有界步骤

1. 在新 runtime checkout 执行 `scripts/runtime/prepare-paddle-ocr.sh --install`，再执行 `--check`；确认固定解释器、三个版本和两个本地模型都已就绪。
2. 历次机器 OCR 结果由本次获准的清理迁移实际删除；保留当前本地安装、模型文件、平台原文和原图。
3. 清理历史结果不会自动重排或重识别；已有 worker 的关闭状态继续保留。后续实际识别需按相应范围运行，云模型外发仍须单独授权。
4. 启动媒体 worker，确认它声明 OCR 能力且新的 Paddle job 为 `local-v4`（整理规则 `rules-v3`），本地 OCR-VL 为 `local-vl-v1`；抽查 raw、layout、分层与“封面 OCR”标题来源。

以上每一步都需要相应的部署/数据库/重排授权；源码合并本身不触发任何一步。

## 指定任务的一次本地复测

`linggan-media-worker --jobs-file /absolute/path/jobs.json` 只认领文件中列出的既有 job UUID（非空 JSON 数组），沿用当前本地处理器、并发限制和读本发布事务。它不投影全库队列，不启动常驻轮询，每个列出的任务至多尝试一次；失败或尚不可领取时以非零退出，后续重试由操作员明确选择。数据库连接阶段仍使用原有连接等待逻辑。

该入口用于已经明确授权并冻结材料范围的本地复测，不创建任务、不更改云端配置或自动外发。图片任务仍为 `local-v4`，ASR 为 `local-v3`；封面和正文槽可指向同一原件，报告分别统计槽位任务数与不同原图数。
