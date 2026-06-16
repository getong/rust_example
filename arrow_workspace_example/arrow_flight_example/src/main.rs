use std::{net::SocketAddr, sync::Arc};

use anyhow::{Context, Result, anyhow};
use arrow_array::{Float64Array, Int32Array, RecordBatch, StringArray};
use arrow_cast::pretty::pretty_format_batches;
use arrow_flight::{
  Action, ActionType, Criteria, Empty, FlightClient, FlightData, FlightDescriptor, FlightEndpoint,
  FlightInfo, HandshakeRequest, HandshakeResponse, PollInfo, PutResult, SchemaResult, Ticket,
  encode::FlightDataEncoderBuilder,
  flight_service_server::{FlightService, FlightServiceServer},
};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use futures::{StreamExt, TryStreamExt, stream::BoxStream};
use tokio::{net::TcpListener, sync::oneshot};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{
  Request, Response, Status, Streaming,
  transport::{Channel, Server},
};

const SALES_TICKET: &str = "sales-demo";

#[derive(Clone)]
struct SalesFlightService {
  batches: Arc<Vec<RecordBatch>>,
  schema: SchemaRef,
  endpoint_uri: String,
}

#[tonic::async_trait]
impl FlightService for SalesFlightService {
  type HandshakeStream = BoxStream<'static, Result<HandshakeResponse, Status>>;
  type ListFlightsStream = BoxStream<'static, Result<FlightInfo, Status>>;
  type DoGetStream = BoxStream<'static, Result<FlightData, Status>>;
  type DoPutStream = BoxStream<'static, Result<PutResult, Status>>;
  type DoActionStream = BoxStream<'static, Result<arrow_flight::Result, Status>>;
  type ListActionsStream = BoxStream<'static, Result<ActionType, Status>>;
  type DoExchangeStream = BoxStream<'static, Result<FlightData, Status>>;

  async fn handshake(
    &self,
    _request: Request<Streaming<HandshakeRequest>>,
  ) -> Result<Response<Self::HandshakeStream>, Status> {
    Err(Status::unimplemented(
      "handshake is not needed in this demo",
    ))
  }

  async fn list_flights(
    &self,
    _request: Request<Criteria>,
  ) -> Result<Response<Self::ListFlightsStream>, Status> {
    let info = self.flight_info()?;
    Ok(Response::new(futures::stream::iter([Ok(info)]).boxed()))
  }

  async fn get_flight_info(
    &self,
    request: Request<FlightDescriptor>,
  ) -> Result<Response<FlightInfo>, Status> {
    let descriptor = request.into_inner();
    if descriptor != demo_descriptor() {
      return Err(Status::not_found("unknown FlightDescriptor"));
    }

    Ok(Response::new(self.flight_info()?))
  }

  async fn poll_flight_info(
    &self,
    _request: Request<FlightDescriptor>,
  ) -> Result<Response<PollInfo>, Status> {
    Err(Status::unimplemented(
      "poll_flight_info is not needed in this demo",
    ))
  }

  async fn get_schema(
    &self,
    _request: Request<FlightDescriptor>,
  ) -> Result<Response<SchemaResult>, Status> {
    Err(Status::unimplemented(
      "get_schema is not needed in this demo",
    ))
  }

  async fn do_get(&self, request: Request<Ticket>) -> Result<Response<Self::DoGetStream>, Status> {
    let ticket = request.into_inner();
    if ticket.ticket.as_ref() != SALES_TICKET.as_bytes() {
      return Err(Status::not_found("unknown Ticket"));
    }

    let batches = self.batches.as_ref().clone();
    let batch_stream = futures::stream::iter(batches.into_iter().map(Ok));
    let flight_stream = FlightDataEncoderBuilder::new()
      .with_schema(Arc::clone(&self.schema))
      .build(batch_stream)
      .map_err(Status::from)
      .boxed();

    Ok(Response::new(flight_stream))
  }

  async fn do_put(
    &self,
    _request: Request<Streaming<FlightData>>,
  ) -> Result<Response<Self::DoPutStream>, Status> {
    Err(Status::unimplemented("do_put is not needed in this demo"))
  }

  async fn do_action(
    &self,
    _request: Request<Action>,
  ) -> Result<Response<Self::DoActionStream>, Status> {
    Err(Status::unimplemented(
      "do_action is not needed in this demo",
    ))
  }

  async fn list_actions(
    &self,
    _request: Request<Empty>,
  ) -> Result<Response<Self::ListActionsStream>, Status> {
    Err(Status::unimplemented(
      "list_actions is not needed in this demo",
    ))
  }

  async fn do_exchange(
    &self,
    _request: Request<Streaming<FlightData>>,
  ) -> Result<Response<Self::DoExchangeStream>, Status> {
    Err(Status::unimplemented(
      "do_exchange is not needed in this demo",
    ))
  }
}

impl SalesFlightService {
  fn new(batches: Vec<RecordBatch>, endpoint_uri: String) -> Result<Self> {
    let schema = batches
      .first()
      .map(RecordBatch::schema)
      .ok_or_else(|| anyhow!("demo needs at least one RecordBatch"))?;

    Ok(Self {
      batches: Arc::new(batches),
      schema,
      endpoint_uri,
    })
  }

  fn flight_info(&self) -> Result<FlightInfo, Status> {
    FlightInfo::new()
      .try_with_schema(self.schema.as_ref())
      .map_err(|error| Status::internal(error.to_string()))
      .map(|info| {
        info
          .with_descriptor(demo_descriptor())
          .with_endpoint(
            FlightEndpoint::new()
              .with_ticket(Ticket::new(SALES_TICKET))
              .with_location(self.endpoint_uri.clone()),
          )
          .with_total_records(total_rows(&self.batches))
          .with_total_bytes(-1)
          .with_ordered(true)
          .with_app_metadata("dataset=sales")
      })
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  let batches = demo_batches()?;
  let (addr, shutdown_tx, server_task) = spawn_flight_server(batches).await?;

  let mut client = connect_client(addr).await?;
  let descriptor = demo_descriptor();
  let flight_info = client
    .get_flight_info(descriptor)
    .await
    .context("get FlightInfo from server")?;

  println!("FlightInfo discovery");
  println!("  endpoint count: {}", flight_info.endpoint.len());
  println!("  total records: {}", flight_info.total_records);
  println!("  ordered stream: {}", flight_info.ordered);

  let ticket = flight_info
    .endpoint
    .first()
    .and_then(|endpoint| endpoint.ticket.clone())
    .ok_or_else(|| anyhow!("FlightInfo did not contain an endpoint ticket"))?;

  let received_batches: Vec<RecordBatch> = client
    .do_get(ticket)
    .await
    .context("open do_get stream")?
    .try_collect()
    .await
    .context("decode FlightData stream into RecordBatches")?;

  println!();
  println!("RecordBatches received through Arrow Flight");
  println!("{}", pretty_format_batches(&received_batches)?);

  shutdown_tx
    .send(())
    .map_err(|_| anyhow!("Flight server shutdown receiver was dropped"))?;
  server_task.await.context("join Flight server task")??;

  Ok(())
}

async fn spawn_flight_server(
  batches: Vec<RecordBatch>,
) -> Result<(
  SocketAddr,
  oneshot::Sender<()>,
  tokio::task::JoinHandle<Result<()>>,
)> {
  let listener = TcpListener::bind("127.0.0.1:0")
    .await
    .context("bind local Flight server")?;
  let addr = listener.local_addr().context("read local server address")?;
  let endpoint_uri = format!("grpc://{addr}");
  let service = SalesFlightService::new(batches, endpoint_uri)?;
  let (shutdown_tx, shutdown_rx) = oneshot::channel();

  let server_task = tokio::spawn(async move {
    Server::builder()
      .add_service(FlightServiceServer::new(service))
      .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
        let _ = shutdown_rx.await;
      })
      .await
      .context("run Flight server")
  });

  Ok((addr, shutdown_tx, server_task))
}

async fn connect_client(addr: SocketAddr) -> Result<FlightClient> {
  let channel = Channel::from_shared(format!("http://{addr}"))
    .context("build Flight client channel")?
    .connect()
    .await
    .context("connect Flight client")?;

  Ok(FlightClient::new(channel))
}

fn demo_descriptor() -> FlightDescriptor {
  FlightDescriptor::new_cmd("sales-demo")
}

fn demo_batches() -> Result<Vec<RecordBatch>> {
  let schema = Arc::new(Schema::new(vec![
    Field::new("order_id", DataType::Int32, false),
    Field::new("region", DataType::Utf8, false),
    Field::new("amount", DataType::Float64, false),
  ]));

  let first = RecordBatch::try_new(
    Arc::clone(&schema),
    vec![
      Arc::new(Int32Array::from(vec![1001, 1002, 1003])),
      Arc::new(StringArray::from(vec!["east", "west", "east"])),
      Arc::new(Float64Array::from(vec![125.50, 320.00, 88.25])),
    ],
  )?;

  let second = RecordBatch::try_new(
    schema,
    vec![
      Arc::new(Int32Array::from(vec![1004, 1005])),
      Arc::new(StringArray::from(vec!["south", "north"])),
      Arc::new(Float64Array::from(vec![410.75, 205.00])),
    ],
  )?;

  Ok(vec![first, second])
}

fn total_rows(batches: &[RecordBatch]) -> i64 {
  batches.iter().map(RecordBatch::num_rows).sum::<usize>() as i64
}
