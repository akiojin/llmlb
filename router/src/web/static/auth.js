// 認証ユーティリティ関数

/**
 * JWTトークン付きでfetchを実行するラッパー関数
 * @param {string} url - リクエストURL
 * @param {RequestInit} options - fetchオプション
 * @returns {Promise<Response>} - fetchレスポンス
 */
async function authenticatedFetch(url, options = {}) {
  const token = localStorage.getItem('jwt_token');

  const headers = {
    ...options.headers,
  };

  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const response = await fetch(url, {
    ...options,
    headers,
  });

  // 401の場合はログインページにリダイレクト
  if (response.status === 401) {
    localStorage.removeItem('jwt_token');
    window.location.href = '/dashboard/login.html';
    return response;
  }

  return response;
}

// グローバルに公開
window.authenticatedFetch = authenticatedFetch;
