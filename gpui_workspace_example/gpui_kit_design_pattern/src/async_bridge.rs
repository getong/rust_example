//! 异步桥接：后台只产出信号，主线程转发给纯模型的自监听器。
//! 使用确定性的模拟下载，无网络依赖；这里演示跨任务，不涉及跨进程 IPC。
use std::time::Duration;

use gpui_kit::{
  component::{Disableable, button::*},
  *,
};

enum LoaderSignal {
  ChunkDownloaded(Vec<u8>),
  DownloadFailed,
}

#[derive(Debug, PartialEq)]
enum LoadingStatus {
  Idle,
  Loading,
  Success(usize),
  Failed,
}

struct ImageLoader {
  status: LoadingStatus,
  _listener: Subscription,
  task: Option<Task<()>>,
}

impl EventEmitter<LoaderSignal> for ImageLoader {}

impl ImageLoader {
  fn new(cx: &mut Context<Self>) -> Self {
    let listener = cx.subscribe_self(|this, signal: &LoaderSignal, cx| {
      this.status = match signal {
        LoaderSignal::ChunkDownloaded(bytes) => LoadingStatus::Success(bytes.len()),
        LoaderSignal::DownloadFailed => LoadingStatus::Failed,
      };
      cx.notify();
    });
    Self {
      status: LoadingStatus::Idle,
      _listener: listener,
      task: None,
    }
  }

  fn start_download(&mut self, fail: bool, cx: &mut Context<Self>) {
    // 单次只运行一个任务，防止旧结果覆盖新请求。
    if self.status == LoadingStatus::Loading {
      return;
    }
    self.status = LoadingStatus::Loading;
    cx.notify();

    let timer = cx.background_executor().timer(Duration::from_secs(1));
    let download = cx.background_executor().spawn(async move {
      timer.await;
      if fail {
        LoaderSignal::DownloadFailed
      } else {
        LoaderSignal::ChunkDownloaded(vec![0_u8; 1024])
      }
    });

    // Context::spawn 在主线程执行。后台任务既不持有 Entity，也不修改 UI。
    // 保存 Task：模型销毁时取消等待；WeakEntity 避免让已关闭视图继续存活。
    self.task = Some(cx.spawn(async move |loader, cx| {
      let signal = download.await;
      // 实体被释放时无需再投递结果。
      let _ = loader.update(cx, |_, cx| cx.emit(signal));
    }));
  }
}

pub(crate) struct AsyncBridgeDemo {
  loader: Entity<ImageLoader>,
  _observer: Subscription,
}

impl AsyncBridgeDemo {
  pub(crate) fn new(cx: &mut Context<Self>) -> Self {
    let loader = cx.new(ImageLoader::new);
    let observer = cx.observe(&loader, |_, _, cx| cx.notify());
    Self {
      loader,
      _observer: observer,
    }
  }
}

impl Render for AsyncBridgeDemo {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let status = &self.loader.read(cx).status;
    let loading = *status == LoadingStatus::Loading;
    let label = match status {
      LoadingStatus::Idle => "Idle — choose an outcome".to_owned(),
      LoadingStatus::Loading => "Loading in background…".to_owned(),
      LoadingStatus::Success(bytes) => format!("Success: {bytes} bytes received"),
      LoadingStatus::Failed => "Download failed — retry with either button".to_owned(),
    };
    div().flex().flex_col().gap_3().child(label).child(
      div()
        .flex()
        .gap_3()
        .child(
          Button::new("load-success")
            .label("Simulate success")
            .disabled(loading)
            .on_click(cx.listener(|this, _, _, cx| {
              this
                .loader
                .update(cx, |loader, cx| loader.start_download(false, cx));
            })),
        )
        .child(
          Button::new("load-failure")
            .label("Simulate failure")
            .disabled(loading)
            .on_click(cx.listener(|this, _, _, cx| {
              this
                .loader
                .update(cx, |loader, cx| loader.start_download(true, cx));
            })),
        ),
    )
  }
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use gpui_kit::{AppContext, TestAppContext};

  use super::{ImageLoader, LoadingStatus};

  #[gpui_kit::test]
  fn background_result_is_delivered_and_failure_can_be_retried(cx: &mut TestAppContext) {
    let loader = cx.new(ImageLoader::new);
    loader.read_with(cx, |loader, _| {
      assert_eq!(loader.status, LoadingStatus::Idle)
    });
    for fail in [true, false] {
      loader.update(cx, |loader, cx| loader.start_download(fail, cx));
      // 运行中发起相反的请求必须被忽略。
      loader.update(cx, |loader, cx| loader.start_download(!fail, cx));
      cx.run_until_parked();
      loader.read_with(cx, |loader, _| {
        assert_eq!(loader.status, LoadingStatus::Loading)
      });
      cx.executor().advance_clock(Duration::from_secs(1));
      cx.run_until_parked();
      loader.read_with(cx, |loader, _| {
        assert_eq!(
          loader.status,
          if fail {
            LoadingStatus::Failed
          } else {
            LoadingStatus::Success(1024)
          }
        );
      });
    }
  }

  #[gpui_kit::test]
  fn dropping_loader_during_download_does_not_retain_it(cx: &mut TestAppContext) {
    let loader = cx.new(ImageLoader::new);
    let weak = loader.downgrade();
    loader.update(cx, |loader, cx| loader.start_download(false, cx));
    cx.run_until_parked();
    drop(loader);
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(weak.upgrade().is_none());
  }
}
