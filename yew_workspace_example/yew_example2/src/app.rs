use std::rc::Rc;

use image::{imageops, RgbaImage};
use wasm_bindgen::{prelude::*, Clamped, JsCast};
use yew::{html, Component, Context, Html};

pub struct App;

pub enum Msg {
  LoadFile(Vec<web_sys::File>),
  Shrink,
}

impl Component for App {
  type Message = Msg;
  type Properties = ();
  fn create(_: &Context<Self>) -> Self {
    Self
  }

  fn update(&mut self, _: &Context<Self>, msg: Self::Message) -> bool {
    match msg {
      Msg::LoadFile(files) => {
        let Some(file) = files.first() else { return false };
        let file_url = web_sys::Url::create_object_url_with_blob(&file).unwrap();
        let document = web_sys::window().unwrap().document().unwrap();
        let image = Rc::new(
          document
            .create_element("img")
            .unwrap()
            .dyn_into::<web_sys::HtmlImageElement>()
            .unwrap(),
        );
        image.set_src(&file_url);

        let image_clone = image.clone();

        let callback = Closure::wrap(Box::new(move || {
          let canvas = document.get_element_by_id("preview").unwrap();
          let canvas: web_sys::HtmlCanvasElement = canvas
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .map_err(|_| ())
            .unwrap();

          let context = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();
          canvas.set_width(image_clone.natural_width());
          canvas.set_height(image_clone.natural_height());
          context
            .draw_image_with_html_image_element(&image_clone, 0.0, 0.0)
            .unwrap();
        }) as Box<dyn Fn()>);
        // .as_ref().unchecked_ref() can extract the &Function from the &JsValue
        image.set_onload(Some(callback.as_ref().unchecked_ref()));
        // Do not drop the Closure so the JS callback won't be invalidated after the
        // function exits
        callback.forget();
      }
      Msg::Shrink => {
        // DO nothing yet
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document.get_element_by_id("preview").unwrap();
        let canvas: web_sys::HtmlCanvasElement = canvas
          .dyn_into::<web_sys::HtmlCanvasElement>()
          .map_err(|_| ())
          .unwrap();

        let context = canvas
          .get_context("2d")
          .unwrap()
          .unwrap()
          .dyn_into::<web_sys::CanvasRenderingContext2d>()
          .unwrap();
        let width: u32 = canvas.width();
        let height: u32 = canvas.height();
        let image_buffer = context
          .get_image_data(0.0, 0.0, width.into(), height.into())
          .unwrap()
          .data();
        let image: RgbaImage =
          image::ImageBuffer::from_vec(width, height, image_buffer.to_vec()).unwrap();
        let output_image =
          imageops::resize(&image, width / 2, height / 2, imageops::FilterType::Nearest);
        let output_image_data = web_sys::ImageData::new_with_u8_clamped_array(
          Clamped(&mut output_image.into_vec()),
          width / 2,
        )
        .unwrap();
        context.clear_rect(0.0, 0.0, width.into(), height.into());
        canvas.set_width(width / 2);
        canvas.set_height(height / 2);
        context
          .put_image_data(&output_image_data, 0.0, 0.0)
          .unwrap();
      }
    }
    true
  }

  fn view(&self, ctx: &Context<Self>) -> Html {
    html! {
        <div>
            <input type="file" name="image-upload" id="image-upload" onchange={ctx.link().callback(|event: yew::Event| {
                let files = event.target()
                    .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
                    .and_then(|input| input.files())
                    .map(|files| (0..files.length()).filter_map(|index| files.get(index)).collect())
                    .unwrap_or_default();
                Msg::LoadFile(files)
            })} />
            <br />
            <button id="shrink" onclick={ctx.link().callback(|_| Msg::Shrink)}>{ "Shrink" }</button>
            <br / >
            <canvas id="preview"></canvas>
        </div>
    }
  }
}
