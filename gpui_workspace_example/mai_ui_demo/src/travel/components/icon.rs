use gpui_kit::{component::Icon, *};

pub(in crate::travel) fn icon(name: &str, color: u32) -> Icon {
  let path = match name {
    "home" => "<path d='m3 10 9-7 9 7v10H3Z M9 20v-7h6v7'/>",
    "map" => "<path d='m3 5 6-2 6 2 6-2v16l-6 2-6-2-6 2Z M9 3v16 M15 5v16'/>",
    "heart" => {
      "<path d='M20.8 4.6a5.5 5.5 0 0 0-7.8 0L12 5.7l-1.1-1.1a5.5 5.5 0 0 0-7.8 7.8L12 \
       21l8.8-8.6a5.5 5.5 0 0 0 0-7.8Z'/>"
    }
    "user" => "<circle cx='12' cy='8' r='4'/><path d='M4 21v-2a8 8 0 0 1 16 0v2'/>",
    "bell" => "<path d='M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9 M10 21h4'/>",
    "check" => "<path d='m5 12 4 4L19 6'/>",
    "arrow" => "<path d='M4 12h16 m-6-6 6 6-6 6'/>",
    "sun" => {
      "<circle cx='12' cy='12' r='4'/><path d='M12 1v3 M12 20v3 M1 12h3 M20 12h3 M4 4l2 2 M18 18l2 \
       2'/>"
    }
    "bike" => {
      "<circle cx='5' cy='16' r='4'/><circle cx='19' cy='16' r='4'/><path d='m5 16 5-9 5 9H5 M9 \
       7h5 M15 4h3l1 12'/>"
    }
    _ => "<path d='M3 21h18 M5 21V10h14v11 M3 10l9-7 9 7 M9 21v-6h6v6'/>",
  };
  Icon::default()
    .data(
      format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' \
         stroke='currentColor' stroke-width='1.6' stroke-linecap='round' \
         stroke-linejoin='round'>{path}</svg>"
      )
      .as_bytes(),
    )
    .size_5()
    .text_color(rgb(color))
}
