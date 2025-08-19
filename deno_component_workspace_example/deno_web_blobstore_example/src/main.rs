use deno_web::BlobStore;
use url::Url;

#[tokio::main]
async fn main() {
  // println!("Hello, world!");
  let blob_store = BlobStore::default();

  let url = Url::parse("https://npmjs.com/").unwrap();
  match blob_store.get_object_url(url) {
    Some(blob) => {
      println!("the blob type is {:?}", blob.media_type);
    }
    _ => {
      println!("not work")
    }
  }
}
