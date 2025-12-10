use llm_router::cli::model::{run, BaseOpts, ModelCommand, OutputFormat};
use tokio::task;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn list_models_table_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/models/available"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "models": [{
                "name": "hf/test/model.gguf",
                "size_gb": 1.0,
                "description": "test",
                "required_memory_gb": 2.0,
                "tags": ["gguf"]
            }],
            "source": "hf"
        })))
        .mount(&server)
        .await;

    let cmd = ModelCommand::List {
        base: BaseOpts {
            router: server.uri(),
        },
        search: None,
        limit: 20,
        offset: 0,
        format: OutputFormat::Table,
    };

    // run blocking CLI in a blocking task
    let result = task::spawn_blocking(move || run(cmd)).await.unwrap();
    assert!(result.is_ok());
}

#[tokio::test]
async fn add_model_returns_error_on_400() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/models/register"))
        .respond_with(ResponseTemplate::new(400).set_body_string("duplicate"))
        .mount(&server)
        .await;

    let cmd = ModelCommand::Add {
        base: BaseOpts {
            router: server.uri(),
        },
        repo: "org/repo".into(),
        file: "model.gguf".into(),
    };

    let err = task::spawn_blocking(move || run(cmd))
        .await
        .unwrap()
        .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("HTTP 400"), "unexpected error: {msg}");
}

#[tokio::test]
async fn download_model_requires_node_when_specific() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/models/download"))
        .respond_with(ResponseTemplate::new(400).set_body_string("node_ids required"))
        .mount(&server)
        .await;

    let cmd = ModelCommand::Download {
        base: BaseOpts {
            router: server.uri(),
        },
        name: "hf/test/model.gguf".into(),
        all: false,
        node: None,
    };

    let err = task::spawn_blocking(move || run(cmd))
        .await
        .unwrap()
        .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("HTTP 400"), "unexpected error: {msg}");
}
