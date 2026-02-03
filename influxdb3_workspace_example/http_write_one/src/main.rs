use chrono::NaiveDateTime;
use influxdb3::{DataPointBuilder, FieldDataType, InfluxDbClientBuilder};

#[tokio::main]
async fn main() {
  let influxdb_client = InfluxDbClientBuilder::new()
    .with_server_endpoint("http://localhost:8181")
    .with_token("apiv3_fVk554m9Nlx7uJ18t_n0n8xxgtbCP7Ud0RwaTmm5dPxWNf62HRuyvRo9cnL1uwrTDLeG22zxK7QdmZLFP-klPw")
    .database("weather")
    .build()
    .unwrap();

  let data_point = DataPointBuilder::new()
    .table("France")
    .with_tag("city", "Paris")
    .with_tag("district", "second")
    .with_tag("sensor_id", "XKCD722")
    .with_field("temperature", FieldDataType::Float(19.78))
    .with_field("hygrometry", FieldDataType::Float(51.0))
    .datetime(
      NaiveDateTime::parse_from_str("2025-12-29T21:10:59.126", "%Y-%m-%dT%H:%M:%S%.3f")
        .unwrap()
        .and_utc(),
    )
    .build()
    .unwrap();

  match influxdb_client.write_one(data_point).await {
    Ok(cluster_uuid_opt) => {
      println!("Writing successful : cluster_uuid = {:?}", cluster_uuid_opt);
    }
    Err(error_detail) => {
      println!("Failure : {:?}", error_detail);
    }
  }
}
