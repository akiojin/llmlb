//! Model management CLI

use clap::{Parser, Subcommand, ValueEnum};
use reqwest::blocking::Client;
use serde_json::json;

#[derive(Parser, Debug)]
/// 共通オプション
/// 共通オプション
pub struct BaseOpts {
    /// Router base URL (e.g., http://127.0.0.1:8080)
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    /// Router base URL (e.g., http://127.0.0.1:8080)
    pub router: String,
}

#[derive(Subcommand, Debug)]
/// モデル管理コマンド
/// モデル管理コマンド
pub enum ModelCommand {
    /// List HF GGUF catalog
    /// HF GGUFカタログを表示
    List {
        #[command(flatten)]
        /// 共通オプション
        base: BaseOpts,
        #[arg(long)]
        /// 検索クエリ
        search: Option<String>,
        #[arg(long, default_value_t = 20)]
        /// 取得件数
        limit: u32,
        #[arg(long, default_value_t = 0)]
        /// オフセット
        offset: u32,
        #[arg(long, value_enum, default_value = "table")]
        /// 出力形式
        format: OutputFormat,
    },
    /// Register HF GGUF to supported models
    /// HF GGUFを対応モデルに登録
    Add {
        #[command(flatten)]
        /// 共通オプション
        base: BaseOpts,
        /// Hugging Face repo (e.g., TheBloke/Llama-2-7B-GGUF)
        /// HFリポジトリ名
        repo: String,
        /// GGUF filename inside the repo
        /// GGUFファイル名
        #[arg(long, short = 'f')]
        file: String,
    },
    /// Trigger download to nodes
    /// 登録モデルをノードにダウンロード指示
    Download {
        #[command(flatten)]
        /// 共通オプション
        base: BaseOpts,
        /// model name (registered ID)
        /// 登録モデルID
        name: String,
        /// 全ノードに配布
        /// 特定ノードID
        #[arg(long, group = "target")]
        all: bool,
        /// 全ノードに配布
        /// 特定ノードID
        #[arg(long, group = "target")]
        node: Option<String>,
    },
}

#[derive(Clone, Debug, ValueEnum)]
/// 出力フォーマット
/// 出力フォーマット
pub enum OutputFormat {
    /// JSON形式
    Json,
    /// テーブル形式
    Table,
}

/// モデルコマンドのエントリポイント
/// モデルコマンドのエントリポイント
pub fn run(cmd: ModelCommand) -> anyhow::Result<()> {
    match cmd {
        ModelCommand::List {
            base,
            search,
            limit,
            offset,
            format,
        } => list_models(base, search, limit, offset, format),
        ModelCommand::Add { base, repo, file } => add_model(base, repo, file),
        ModelCommand::Download {
            base,
            name,
            all,
            node,
        } => download_model(base, name, all, node),
    }
}

fn list_models(
    base: BaseOpts,
    search: Option<String>,
    limit: u32,
    offset: u32,
    format: OutputFormat,
) -> anyhow::Result<()> {
    let client = Client::new();
    let url = format!(
        "{}/api/models/available?source=hf&limit={}&offset={}&search={}",
        base.router,
        limit,
        offset,
        search.unwrap_or_default()
    );
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }
    let v: serde_json::Value = resp.json()?;
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        OutputFormat::Table => {
            if let Some(models) = v.get("models").and_then(|m| m.as_array()) {
                println!("{:<60} {:>8} {:<20}", "name", "GB", "source");
                for m in models {
                    let name = m.get("name").and_then(|x| x.as_str()).unwrap_or("-");
                    let gb = m
                        .get("size_gb")
                        .and_then(|x| x.as_f64())
                        .map(|x| format!("{:.1}", x))
                        .unwrap_or("-".into());
                    let source = v.get("source").and_then(|x| x.as_str()).unwrap_or("hf");
                    println!("{:<60} {:>8} {:<20}", name, gb, source);
                }
            } else {
                println!("no models");
            }
        }
    }
    Ok(())
}

fn add_model(base: BaseOpts, repo: String, file: String) -> anyhow::Result<()> {
    let client = Client::new();
    let url = format!("{}/api/models/register", base.router);
    let body = json!({
        "repo": repo,
        "filename": file
    });
    let resp = client.post(url).json(&body).send()?;
    if !resp.status().is_success() {
        let status = resp.status();
        let txt = resp.text().unwrap_or_default();
        anyhow::bail!("HTTP {} {}", status, txt);
    }
    let v: serde_json::Value = resp.json()?;
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

fn download_model(
    base: BaseOpts,
    name: String,
    all: bool,
    node: Option<String>,
) -> anyhow::Result<()> {
    let target = if all { "all" } else { "specific" };
    let node_ids: Vec<String> = node.into_iter().collect();
    let client = Client::new();
    let url = format!("{}/api/models/download", base.router);
    let body = json!({
        "model_name": name,
        "target": target,
        "node_ids": node_ids,
    });
    let resp = client.post(url).json(&body).send()?;
    if !resp.status().is_success() {
        let status = resp.status();
        let txt = resp.text().unwrap_or_default();
        anyhow::bail!("HTTP {} {}", status, txt);
    }
    let v: serde_json::Value = resp.json()?;
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}
