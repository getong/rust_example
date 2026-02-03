use influxdb3::InfluxDbClientBuilder;
use serde::Deserialize;

#[tokio::main]
async fn main() {
  let influxdb_client = InfluxDbClientBuilder::new()
    .with_server_endpoint("http://localhost:8181")
    .with_token("apiv3_fVk554m9Nlx7uJ18t_n0n8xxgtbCP7Ud0RwaTmm5dPxWNf62HRuyvRo9cnL1uwrTDLeG22zxK7QdmZLFP-klPw")
    .database("weather")
    .build()
    .unwrap();

  match influxdb_client
    .query_sql("SELECT time,temperature,hygrometry FROM 'France'")
    .await
  {
    Ok(response_value) => match serde_json::from_value::<Vec<DataPoint>>(response_value) {
      Ok(data_points) => {
        println!("Response : {:#?}", data_points);
      }
      Err(error_details) => {
        println!("Failure to parse content : {:?}", error_details);
      }
    },
    Err(error_detail) => {
      println!("Failure : {:?}", error_detail);
    }
  }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DataPoint {
  time: String,
  temperature: f64,
  hygrometry: f64,
}
