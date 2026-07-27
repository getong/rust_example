mod app;
mod features;

fn main() {
  if std::env::args().any(|argument| argument == "--blitz-preview") {
    let html = blitz_preview_html();
    blitz::launch_static_html(&html);
  } else {
    dioxus::launch(app::App);
  }
}

fn blitz_preview_html() -> String {
  format!(
    r#"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8">
    <title>Focus Board</title>
    <style>{}</style>
  </head>
  <body>
    <div id="main">
      <div class="app-shell">
        <header class="top-bar">
          <div class="brand-mark">F</div>
          <div>
            <p class="brand-name">Focus Board</p>
            <p class="brand-date">本地任务空间</p>
          </div>
          <span class="storage-status">已连接</span>
        </header>

        <main class="page-content">
          <section class="page">
            <div class="page-heading">
              <div>
                <p class="eyebrow">进度</p>
                <h1>概览</h1>
              </div>
            </div>

            <div class="progress-panel">
              <div class="progress-copy">
                <span class="progress-value">67%</span>
                <span class="progress-label">总体完成率</span>
              </div>
              <div class="progress-track">
                <div class="progress-fill" style="width: 67%"></div>
              </div>
            </div>

            <div class="metric-grid">
              <div class="metric ink"><strong>6</strong><span>任务总数</span></div>
              <div class="metric coral"><strong>2</strong><span>正在进行</span></div>
              <div class="metric green"><strong>4</strong><span>已经完成</span></div>
            </div>

            <div class="section-heading">
              <h2>最近任务</h2>
              <span>4 条</span>
            </div>
            <div class="compact-list">
              <div class="compact-row">
                <span class="status-dot"></span>
                <span class="compact-title">整理本周工作计划</span>
                <span class="compact-status">进行中</span>
              </div>
              <div class="compact-row">
                <span class="status-dot done"></span>
                <span class="compact-title done">完成项目架构设计</span>
                <span class="compact-status">完成</span>
              </div>
              <div class="compact-row">
                <span class="status-dot"></span>
                <span class="compact-title">检查 Blitz 原生渲染效果</span>
                <span class="compact-status">进行中</span>
              </div>
              <div class="compact-row">
                <span class="status-dot done"></span>
                <span class="compact-title done">配置本地数据存储</span>
                <span class="compact-status">完成</span>
              </div>
            </div>
          </section>
        </main>

        <nav class="bottom-nav" aria-label="主导航">
          <span class="nav-item"><span class="nav-symbol">✓</span><span class="nav-label">任务</span></span>
          <span class="nav-item active"><span class="nav-symbol">▦</span><span class="nav-label">概览</span></span>
          <span class="nav-item"><span class="nav-symbol">⚙</span><span class="nav-label">设置</span></span>
        </nav>
      </div>
    </div>
  </body>
</html>"#,
    app::MAIN_CSS
  )
}

#[cfg(test)]
mod tests {
  use super::blitz_preview_html;

  #[test]
  fn blitz_preview_embeds_the_application_stylesheet() {
    let html = blitz_preview_html();

    assert!(html.contains(".app-shell"));
    assert!(html.contains("class=\"progress-panel\""));
  }
}
