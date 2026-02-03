use chrono::Utc;
use influx3_lp::Influx3Lp;
use influxdb3::InfluxDbClientBuilder;

#[tokio::main]
async fn main() {
  let influxdb_client = InfluxDbClientBuilder::new()
    .with_server_endpoint("http://localhost:8181")
    .with_token("apiv3_fVk554m9Nlx7uJ18t_n0n8xxgtbCP7Ud0RwaTmm5dPxWNf62HRuyvRo9cnL1uwrTDLeG22zxK7QdmZLFP-klPw")
    .database("weather")
    .build()
    .unwrap();

  let weather_data_point = WeatherDataPoint {
    timestamp: Utc::now().timestamp_nanos_opt().unwrap(),
    city: "Paris".to_string(),
    district: "second".to_string(),
    sensor_id: "XKCD722".to_string(),
    temperature: 20.01,
    hygrometry: 46.0,
  };

  match influxdb_client.health().await {
    Ok(()) => {
      println!("Client is up and running !");

      match influxdb_client.write_one_typed(weather_data_point).await {
        Ok(cluster_uuid_opt) => {
          println!("Writing successful : cluster_uuid = {:?}", cluster_uuid_opt);
        }
        Err(error_detail) => {
          println!("Failure : {:?}", error_detail);
        }
      }
    }
    Err(error_detail) => {
      println!("Something is wrong : {:?}", error_detail);
    }
  }
}

#[derive(Influx3Lp)]
#[influx3_lp(table_name = "France")]
struct WeatherDataPoint {
  #[influx3_lp(timestamp)]
  pub timestamp: i64,
  #[influx3_lp(tag)]
  pub city: String,
  #[influx3_lp(tag)]
  pub district: String,
  #[influx3_lp(tag)]
  pub sensor_id: String,
  pub temperature: f64,
  pub hygrometry: f64,
}
