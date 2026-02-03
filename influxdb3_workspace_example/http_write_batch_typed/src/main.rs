use chrono::Utc;
use influx3_lp::Influx3Lp;
use influxdb3::InfluxDbClientBuilder;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() {
  let influxdb_client = InfluxDbClientBuilder::new()
    .with_server_endpoint("http://localhost:8181")
    .with_token("apiv3_0Z8nqz5wAp5g1kfcZeVKNWwYgv88ia7skv3-E8nQnbeYC6ESjLGX4elUseq_fL4hmZh0tSkeIE33lwk_gXp22g")
    .database("weather")
    .build()
    .unwrap();

  let weather_data_point_1 = WeatherDataPoint {
    timestamp: Utc::now().timestamp_nanos_opt().unwrap(),
    city: "Paris".to_string(),
    district: "second".to_string(),
    sensor_id: "XKCD722".to_string(),
    temperature: 21.12,
    hygrometry: 46.0,
  };

  sleep(Duration::from_millis(200)).await;

  let weather_data_point_2 = WeatherDataPoint {
    timestamp: Utc::now().timestamp_nanos_opt().unwrap(),
    city: "Paris".to_string(),
    district: "second".to_string(),
    sensor_id: "XKCD722".to_string(),
    temperature: 21.26,
    hygrometry: 45.0,
  };

  let batch = Vec::from([weather_data_point_1, weather_data_point_2]);

  match influxdb_client.write_batch_typed(&batch).await {
    Ok(cluster_uuid_opt) => {
      println!("Writing successful : cluster_uuid = {:?}", cluster_uuid_opt);
    }
    Err(error_detail) => {
      println!("Failure : {:?}", error_detail);
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
