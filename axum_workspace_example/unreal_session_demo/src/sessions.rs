use axum::{
  Extension, Json,
  extract::{Path, Query, State},
  http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
  AppState,
  auth::{self, AuthenticatedPlayer, GAME_AUDIENCE, Player},
  error::ApiError,
};

const TICKET_LIFETIME: u64 = 120;

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionState {
  Waiting,
  InProgress,
  Ended,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
  name: String,
  map: String,
  mode: String,
  max_players: usize,
  advertised: bool,
  allow_join_in_progress: bool,
}

impl Settings {
  fn validate(&self) -> Result<(), ApiError> {
    if [&self.name, &self.map, &self.mode]
      .iter()
      .any(|value| value.trim().is_empty() || value.len() > 128)
    {
      return Err(ApiError::bad_request(
        "name, map and mode must contain 1..128 bytes of nonblank text",
      ));
    }
    if !(1..=16).contains(&self.max_players) {
      return Err(ApiError::bad_request(
        "max_players must be between 1 and 16 (including the host)",
      ));
    }
    Ok(())
  }
}

#[derive(Clone, Serialize)]
struct Member {
  player_id: String,
  #[serde(skip_serializing)]
  membership_id: Uuid,
}

#[derive(Clone, Serialize)]
pub(crate) struct Session {
  id: Uuid,
  host_id: String,
  state: SessionState,
  settings: Settings,
  members: Vec<Member>,
}

impl Session {
  fn require_host(&self, player: &str) -> Result<(), ApiError> {
    if self.host_id != player {
      return Err(ApiError::forbidden(
        "only the host can perform this operation",
      ));
    }
    Ok(())
  }

  fn accepts_join(&self) -> bool {
    self.state == SessionState::Waiting
      || (self.state == SessionState::InProgress && self.settings.allow_join_in_progress)
  }

  fn admission(&self, state: &AppState, member: &Member) -> Result<JoinResponse, ApiError> {
    Ok(JoinResponse {
      session: self.clone(),
      connect_string: state.game_server_addr.to_string(),
      travel_url_example: format!("{}?Ticket=<join_ticket>", state.game_server_addr),
      join_ticket: auth::issue(
        state,
        &member.player_id,
        GAME_AUDIENCE,
        TICKET_LIFETIME,
        Some((self.id, member.membership_id)),
      )?,
      expires_in: TICKET_LIFETIME,
    })
  }
}

#[derive(Serialize)]
pub(crate) struct JoinResponse {
  session: Session,
  connect_string: String,
  travel_url_example: String,
  join_ticket: String,
  expires_in: u64,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Search {
  map: Option<String>,
  mode: Option<String>,
}

pub(crate) async fn create(
  State(state): State<AppState>,
  Extension(Player(player)): AuthenticatedPlayer,
  Json(settings): Json<Settings>,
) -> Result<(StatusCode, Json<JoinResponse>), ApiError> {
  settings.validate()?;
  let member = Member {
    player_id: player.clone(),
    membership_id: Uuid::new_v4(),
  };
  let session = Session {
    id: Uuid::new_v4(),
    host_id: player,
    state: SessionState::Waiting,
    settings,
    members: vec![member.clone()],
  };
  let admission = session.admission(&state, &member)?;
  state.sessions.write().await.insert(session.id, session);
  Ok((StatusCode::CREATED, Json(admission)))
}

pub(crate) async fn find(
  State(state): State<AppState>,
  Query(search): Query<Search>,
) -> Json<Vec<Session>> {
  let sessions = state.sessions.read().await;
  let mut found: Vec<_> = sessions
    .values()
    .filter(|session| {
      session.settings.advertised
        && session.accepts_join()
        && session.members.len() < session.settings.max_players
        && search
          .map
          .as_ref()
          .is_none_or(|map| *map == session.settings.map)
        && search
          .mode
          .as_ref()
          .is_none_or(|mode| *mode == session.settings.mode)
    })
    .cloned()
    .collect();
  found.sort_by_key(|session| session.id);
  Json(found)
}

pub(crate) async fn get(
  State(state): State<AppState>,
  Extension(Player(player)): AuthenticatedPlayer,
  Path(id): Path<Uuid>,
) -> Result<Json<Session>, ApiError> {
  let sessions = state.sessions.read().await;
  let session = sessions.get(&id).ok_or_else(ApiError::not_found)?;
  if !session.settings.advertised
    && !session
      .members
      .iter()
      .any(|member| member.player_id == player)
  {
    return Err(ApiError::not_found());
  }
  Ok(Json(session.clone()))
}

pub(crate) async fn update(
  State(state): State<AppState>,
  Extension(Player(player)): AuthenticatedPlayer,
  Path(id): Path<Uuid>,
  Json(settings): Json<Settings>,
) -> Result<Json<Session>, ApiError> {
  settings.validate()?;
  let mut sessions = state.sessions.write().await;
  let session = sessions.get_mut(&id).ok_or_else(ApiError::not_found)?;
  session.require_host(&player)?;
  if session.state != SessionState::Waiting {
    return Err(ApiError::conflict("settings can only change while waiting"));
  }
  if settings.max_players < session.members.len() {
    return Err(ApiError::conflict(
      "capacity cannot be smaller than the current member count",
    ));
  }
  session.settings = settings;
  Ok(Json(session.clone()))
}

pub(crate) async fn join(
  State(state): State<AppState>,
  Extension(Player(player)): AuthenticatedPlayer,
  Path(id): Path<Uuid>,
) -> Result<Json<JoinResponse>, ApiError> {
  let mut sessions = state.sessions.write().await;
  let session = sessions.get_mut(&id).ok_or_else(ApiError::not_found)?;
  if session.state == SessionState::Ended {
    return Err(ApiError::conflict("session has ended"));
  }
  if let Some(member) = session
    .members
    .iter()
    .find(|member| member.player_id == player)
  {
    return Ok(Json(session.admission(&state, member)?));
  }
  if !session.accepts_join() {
    return Err(ApiError::conflict(
      "joining an in-progress session is disabled",
    ));
  }
  if session.members.len() >= session.settings.max_players {
    return Err(ApiError::conflict("session is full"));
  }
  let member = Member {
    player_id: player,
    membership_id: Uuid::new_v4(),
  };
  session.members.push(member.clone());
  match session.admission(&state, &member) {
    Ok(admission) => Ok(Json(admission)),
    Err(error) => {
      session.members.pop();
      Err(error)
    }
  }
}

pub(crate) async fn leave(
  State(state): State<AppState>,
  Extension(Player(player)): AuthenticatedPlayer,
  Path(id): Path<Uuid>,
) -> Result<Json<Session>, ApiError> {
  let mut sessions = state.sessions.write().await;
  let session = sessions.get_mut(&id).ok_or_else(ApiError::not_found)?;
  if session.host_id == player {
    return Err(ApiError::conflict(
      "the host must destroy the session instead of leaving",
    ));
  }
  session.members.retain(|member| member.player_id != player);
  Ok(Json(session.clone()))
}

async fn transition(
  state: AppState,
  player: String,
  id: Uuid,
  expected: SessionState,
  next: SessionState,
) -> Result<Json<Session>, ApiError> {
  let mut sessions = state.sessions.write().await;
  let session = sessions.get_mut(&id).ok_or_else(ApiError::not_found)?;
  session.require_host(&player)?;
  if session.state != expected {
    return Err(ApiError::conflict("invalid session state transition"));
  }
  session.state = next;
  Ok(Json(session.clone()))
}

pub(crate) async fn start(
  State(state): State<AppState>,
  Extension(Player(player)): AuthenticatedPlayer,
  Path(id): Path<Uuid>,
) -> Result<Json<Session>, ApiError> {
  transition(
    state,
    player,
    id,
    SessionState::Waiting,
    SessionState::InProgress,
  )
  .await
}

pub(crate) async fn end(
  State(state): State<AppState>,
  Extension(Player(player)): AuthenticatedPlayer,
  Path(id): Path<Uuid>,
) -> Result<Json<Session>, ApiError> {
  transition(
    state,
    player,
    id,
    SessionState::InProgress,
    SessionState::Ended,
  )
  .await
}

pub(crate) async fn destroy(
  State(state): State<AppState>,
  Extension(Player(player)): AuthenticatedPlayer,
  Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
  let mut sessions = state.sessions.write().await;
  sessions
    .get(&id)
    .ok_or_else(ApiError::not_found)?
    .require_host(&player)?;
  sessions.remove(&id);
  Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConnectRequest {
  ticket: String,
}

#[derive(Serialize)]
pub(crate) struct ConnectResponse {
  admitted: bool,
  player_id: String,
  session_id: Uuid,
  session_state: SessionState,
  message: &'static str,
}

pub(crate) async fn connect(
  State(state): State<AppState>,
  Json(request): Json<ConnectRequest>,
) -> Result<Json<ConnectResponse>, ApiError> {
  let claims = auth::verify(&state, &request.ticket, GAME_AUDIENCE)?;
  let id = claims.session_id.ok_or_else(ApiError::unauthorized)?;
  let membership_id = claims.membership_id.ok_or_else(ApiError::unauthorized)?;
  let sessions = state.sessions.read().await;
  let session = sessions.get(&id).ok_or_else(ApiError::not_found)?;
  if session.state == SessionState::Ended {
    return Err(ApiError::conflict("session has ended"));
  }
  if !session
    .members
    .iter()
    .any(|member| member.player_id == claims.sub && member.membership_id == membership_id)
  {
    return Err(ApiError::forbidden(
      "membership revoked; join again to obtain a new ticket",
    ));
  }
  Ok(Json(ConnectResponse {
    admitted: true,
    player_id: claims.sub,
    session_id: id,
    session_state: session.state,
    message: "admission check only; no Unreal network connection was created",
  }))
}
