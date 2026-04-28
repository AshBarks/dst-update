# dst-update

《饥荒联机版》(Don't Starve Together) 版本更新查询工具。

通过 RSS 源或官方更新页面获取最新版本信息，提取 build_number、release_id、发布日期等结构化数据。

## 安装

先安装 Rust：

```bash
curl -fsSL https://sh.rustup.rs | sh
```

然后编译项目：

```bash
cargo build --release
```

然后切换到 `target/release` 目录，或者将其中的 `dst-update` 复制到可执行目录中。

## 使用方法

不带子命令时默认使用 `html` 数据源：

```bash
dst-update
# 输出: 529998 2022-11-05 Test https://forums.kleientertainment.com/...
```

### html — 从更新页面获取

```bash
dst-update html
dst-update html --test
dst-update html --release
dst-update html --build-only
```

| 参数 | 说明 |
|------|------|
| `--test` | 仅查询 Test 或 Unknown 状态的最新版本 |
| `--release` | 仅查询 Release 或 Unknown 状态的最新版本 |
| `--build-only` | 仅输出 build_number |

`--test` 与 `--release` 互斥，不可同时使用。

### rss — 从 RSS 源获取

```bash
dst-update rss
dst-update rss --detail
dst-update rss --build-only
```

| 参数 | 说明 |
|------|------|
| `--detail` | 额外输出更新描述 |
| `--build-only` | 仅输出 build_number |

RSS 模式自动过滤非 PC 平台的更新。

```bash
dst-update rss
# 输出: 530002 2022-11-07T20:00:00+00:00 https://forums.kleientertainment.com/...
```

## 输出格式

| 数据源 | 默认格式 | `--build-only` |
|--------|---------|----------------|
| html | `{build_number} {date} {status} {link}` | `{build_number}` |
| rss | `{build_number} {rfc3339_datetime} {link}` | `{build_number}` |

## 项目结构

```
src/
├── main.rs    # CLI 入口 (clap)
├── lib.rs     # 库接口与 re-export
├── model.rs   # 数据模型
├── rss.rs     # RSS 获取与解析
├── html.rs    # 更新页面获取与解析
├── http.rs    # HTTP 客户端 (含重试)
└── text.rs    # HTML 文本清洗
```

## 许可证

Apache License 2.0 或 MIT License
