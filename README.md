# dst-update

《饥荒联机版》(Don't Starve Together) 版本更新查询与导出工具。

通过 Klei 官方论坛的更新页面或 RSS 源获取最新版本信息，提取 build_number、release_id、发布日期等结构化数据，并支持将全量更新详情导出为 JSON。

## 安装

### 从源码编译

先安装 Rust：

```bash
curl -fsSL https://sh.rustup.rs | sh
```

然后编译项目：

```bash
cargo build --release
```

编译产物位于 `target/release/dst-update`，可复制到任意可执行目录中使用。

### 从 Release 下载

项目通过 GitHub Actions 自动构建以下平台的二进制文件：

| 平台 | 文件 |
|------|------|
| Linux x86_64 | `dst-update-linux-x86_64` |
| macOS x86_64 | `dst-update-macos-x86_64` |
| macOS Apple Silicon | `dst-update-macos-aarch64` |
| Windows x86_64 | `dst-update-windows-x86_64.exe` |

## 使用方法

### 全局选项

| 选项 | 说明 |
|------|------|
| `-v` | 增加日志详细程度（可多次使用：`-v` = info，`-vv` = debug） |
| `--build-only` | 仅输出 build_number（仅默认模式生效） |
| `--test` | 仅查询 Test 或 Unknown 状态（仅默认模式生效） |
| `--release` | 仅查询 Release 或 Unknown 状态（仅默认模式生效） |

`--test` 与 `--release` 互斥，不可同时使用。

不带子命令时默认使用 `html` 数据源：

```bash
dst-update
# 输出: 728321 2026-05-12 Release https://forums.kleientertainment.com/game-updates/dst/728321-r2733/

dst-update --build-only
# 输出: 728321

dst-update --test
# 输出: 725604 2026-04-22 Test https://forums.kleientertainment.com/game-updates/dst/725604-r2728/
```

### html — 从更新页面获取

从 Klei 官方论坛的更新列表页面抓取最新版本信息。

```bash
dst-update html
dst-update html --test
dst-update html --release
dst-update html --build-only
```

| 参数 | 说明 |
|------|------|
| `--build-only` | 仅输出 build_number |
| `--test` | 仅查询 Test 或 Unknown 状态的最新版本 |
| `--release` | 仅查询 Release 或 Unknown 状态的最新版本 |

### rss — 从 RSS 源获取

从 Klei 官方论坛的 RSS Feed 抓取最新版本信息。默认仅返回 PC 平台更新。

```bash
dst-update rss
# 输出: 728321 2026-05-12T18:00:00+00:00 https://forums.kleientertainment.com/game-updates/dst/728321-r2733/

dst-update rss --detail
# 额外输出更新描述文本

dst-update rss --build-only
# 输出: 728321
```

| 参数 | 说明 |
|------|------|
| `--detail` | 额外输出更新描述（HTML 已清洗为纯文本） |
| `--build-only` | 仅输出 build_number |

### export — 导出全量更新详情

抓取所有更新条目的详情页，导出为 JSON 文件。支持增量更新：若目标文件已存在，将跳过已抓取的条目，仅抓取新增部分。

```bash
dst-update export
# 默认输出到 dst_updates.json

dst-update export --output updates.json --pretty
# 输出格式化的 JSON

dst-update export --max-pages 5 --concurrency 3 --interval 500
# 限制翻页数、并发数和请求间隔
```

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `--output` | `dst_updates.json` | 输出文件路径 |
| `--max-pages` | 无限制（自动翻到空页为止） | 最大抓取页数 |
| `--pretty` | `false` | 格式化 JSON 输出 |
| `--concurrency` | `5` | 并发请求数 |
| `--interval` | `1000`（毫秒） | 并发请求之间的间隔 |

导出的 JSON 格式：

```json
{
  "total": 75,
  "fetched_at": "2026-05-14T16:10:13+00:00",
  "updates": [
    {
      "build_number": 728321,
      "link": "https://forums.kleientertainment.com/game-updates/dst/728321-r2733/",
      "release_date": "2026-05-12",
      "is_hotfix": true,
      "status": "Release",
      "is_current": true,
      "row_id": 2733,
      "release_id": 2733,
      "title": "[Game Update] - 728321",
      "content_text": "Small hotfix to address some concerns..."
    }
  ]
}
```

## 输出格式汇总

| 子命令 | 默认格式 | `--build-only` |
|--------|---------|----------------|
| html | `{build_number} {date} {status} {link}` | `{build_number}` |
| rss | `{build_number} {rfc3339_datetime} {link}` | `{build_number}` |
| export | JSON 文件 | — |

## 特性

- **双数据源**：支持从 HTML 更新页面或 RSS Feed 获取数据
- **状态过滤**：按 Test / Release 状态筛选更新
- **增量导出**：export 命令自动检测已抓取条目，仅抓取新增更新
- **并发抓取**：可配置并发数和请求间隔，平衡速度与服务器压力
- **自动重试**：HTTP 请求失败时指数退避重试（最多 3 次，初始 1s，最大 30s）
- **HTML 清洗**：将富文本更新描述转为干净的纯文本
- **跨平台**：支持 Linux、macOS、Windows

## 项目结构

```
src/
├── main.rs    # CLI 入口与子命令分发 (clap)
├── lib.rs     # 库接口与 re-export
├── model.rs   # 数据模型与错误类型
├── html.rs    # 更新页面抓取、解析与详情导出
├── rss.rs     # RSS 获取与解析
├── http.rs    # HTTP 客户端（含同步/异步与指数退避重试）
└── text.rs    # HTML 文本清洗
```

## 许可证

Apache License 2.0 或 MIT License
