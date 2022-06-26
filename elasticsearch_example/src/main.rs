use elasticsearch::{http::Method, Elasticsearch, SearchParts};
use http::HeaderMap;
use serde_json::{json, Value};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Elasticsearch::default();
    let body = b"{\"query\":{\"match_all\":{}}}";
    let response = client
        .send(
            Method::Post,
            SearchParts::Index(&["tweets"]).url().as_ref(),
            HeaderMap::new(),
            Option::<&Value>::None,
            Some(body.as_ref()),
            None,
        )
        .await?;
    println!("response: {:?}", response);
    // Ok(())

    let client = Elasticsearch::default();

    // make a search API call
    let search_response = client
        .search(SearchParts::None)
        .body(json!({
            "query": {
                "match_all": {}
            }
        }))
        .allow_no_indices(true)
        .send()
        .await?;

    // get the HTTP response status code
    let status_code = search_response.status_code();
    println!("status_code: {}", status_code);

    // read the response body. Consumes search_response
    let response_body = search_response.json::<Value>().await?;
    println!("response_body: {}", response_body);

    // read fields from the response body
    let took = response_body["took"].as_i64().unwrap();
    println!("took: {}", took);

    Ok(())
}
