// 認証モジュール

/// 初回起動時の管理者アカウント作成
pub mod bootstrap;
/// メールID形式の検証
pub mod email;
/// JWT生成・検証（jsonwebtoken）
pub mod jwt;
/// 認証ミドルウェア（JWT, APIキー, ノードトークン）
pub mod middleware;
/// パスワードハッシュ化・検証（bcrypt）
pub mod password;

mod cookie;

pub use cookie::{
    build_csrf_cookie, build_jwt_cookie, clear_csrf_cookie, clear_jwt_cookie,
    generate_random_token, DASHBOARD_CSRF_COOKIE, DASHBOARD_JWT_COOKIE,
};
