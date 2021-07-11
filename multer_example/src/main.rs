use bytes::Bytes;
use futures::stream::Stream;
// Import multer types.
use futures::stream;
use multer::Multipart;
use std::convert::Infallible;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Generate a byte stream and the boundary from somewhere e.g. server request body.
    let (stream, boundary) = get_byte_stream_from_somewhere().await;

    // Create a `Multipart` instance from that byte stream and the boundary.
    let mut multipart = Multipart::new(stream, boundary);

    // Iterate over the fields, use `next_field()` to get the next field.
    while let Some(mut field) = multipart.next_field().await? {
        // Get field name.
        let name = field.name();
        // Get the field's filename if provided in "Content-Disposition" header.
        let file_name = field.file_name();

        println!("Name: {:?}, File Name: {:?}", name, file_name);

        // Process the field data chunks e.g. store them in a file.
        while let Some(chunk) = field.chunk().await? {
            // Do something with field chunk.
            println!("Chunk: {:?}", chunk);
        }
    }

    Ok(())
}

// Generate a byte stream and the boundary from somewhere e.g. server request body.
async fn get_byte_stream_from_somewhere(
) -> (impl Stream<Item = Result<Bytes, Infallible>>, &'static str) {
    let data = "--X-BOUNDARY\r\nContent-Disposition: form-data; name=\"my_text_field\"\r\n\r\nabcd\r\n--X-BOUNDARY--\r\n";
    let stream = stream::once(async move { Result::<Bytes, Infallible>::Ok(Bytes::from(data)) });

    (stream, "X-BOUNDARY")
}
