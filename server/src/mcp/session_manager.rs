//! A session manager that rebuilds sessions the process does not know.

// MCP clients such as `mcp-remote` keep sending a session id after the server
// restarts or reaps the session, and never re-initialize when they get a 404.
// `ReattachingSessionManager` answers such a request by building a fresh
// session under the same id, so the client carries on without reconnecting.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use futures::Stream;
use rmcp::model::{
    ClientCapabilities, ClientJsonRpcMessage, ClientRequest, Implementation,
    InitializeRequestParams, NumberOrString, ProtocolVersion, Request, ServerJsonRpcMessage,
};
use rmcp::transport::streamable_http_server::session::local::{
    create_local_session, LocalSessionHandle, LocalSessionManager, LocalSessionManagerError,
    SessionConfig, SessionError,
};
use rmcp::transport::streamable_http_server::session::{
    ServerSseMessage, SessionId, SessionManager,
};
use rmcp::transport::{TransportAdapterIdentity, WorkerTransport};
use rmcp::{serve_server, RoleServer, Service};
use uuid::Uuid;

/// Caps how many deleted session ids are remembered at once.
const TOMBSTONE_CAP: usize = 1024;

/// How long a deleted id stays refused when sessions never expire.
const TOMBSTONE_TTL_WITHOUT_KEEP_ALIVE: Duration = Duration::from_secs(60 * 60 * 24 * 7);

/// How long a rebuild waits for the new session to answer `initialize`.
const REBUILD_INITIALIZE_TIMEOUT: Duration = Duration::from_secs(10);

/// Names the client a rebuilt session reports in place of the real one.
const REBUILD_CLIENT_NAME: &str = "fewd-session-rebuild";

/// Serves MCP sessions from memory and rebuilds any session whose id it does
/// not hold.
///
/// A request carrying an unknown id gets a new session under that id, as if
/// the client had just initialized, provided the id is a canonical
/// lowercase hyphenated UUID and the client has not recently deleted it.
/// Every other id gets a 404 as usual. A rebuild that fails to start a
/// session is an error, which rmcp answers with a 500.
///
/// A rebuilt session reports placeholder client info and declares no client
/// capabilities, so handlers see a reattached client as supporting neither
/// sampling nor elicitation.
///
/// A deleted id stays refused for the session keep-alive (7 days when
/// sessions never expire) or until 1024 newer deletions displace it. The
/// record lives only in memory, so the id can be rebuilt after a restart.
///
/// Callers must authenticate requests before they reach this manager,
/// because any well-formed id reaches a working session.
pub(crate) struct ReattachingSessionManager<S> {
    shared: Arc<Shared<S>>,
}

struct Shared<S> {
    local: LocalSessionManager,
    factory: Box<dyn Fn() -> Result<S, io::Error> + Send + Sync>,
    rebuild_lock: tokio::sync::Mutex<()>,
    tombstones: Mutex<Tombstones>,
    // The generation of each rebuilt session currently in `local.sessions`.
    // A rebuilt session's exit task removes its map entry only while the
    // generation still matches, so a late exit cannot close a newer session.
    generations: Mutex<HashMap<SessionId, u64>>,
    next_generation: AtomicU64,
    #[cfg(test)]
    rebuilds: std::sync::atomic::AtomicUsize,
}

impl<S> ReattachingSessionManager<S>
where
    S: Service<RoleServer> + Send + 'static,
{
    /// Creates a manager whose sessions use `config` and whose rebuilt
    /// sessions are served by handlers from `factory`.
    pub(crate) fn new(
        config: SessionConfig,
        factory: impl Fn() -> Result<S, io::Error> + Send + Sync + 'static,
    ) -> Self {
        let tombstone_ttl = config
            .keep_alive
            .unwrap_or(TOMBSTONE_TTL_WITHOUT_KEEP_ALIVE);
        let mut local = LocalSessionManager::default();
        local.session_config = config;
        Self {
            shared: Arc::new(Shared {
                local,
                factory: Box::new(factory),
                rebuild_lock: tokio::sync::Mutex::new(()),
                tombstones: Mutex::new(Tombstones::new(tombstone_ttl, TOMBSTONE_CAP)),
                generations: Mutex::new(HashMap::new()),
                next_generation: AtomicU64::new(0),
                #[cfg(test)]
                rebuilds: std::sync::atomic::AtomicUsize::new(0),
            }),
        }
    }
}

impl<S> Shared<S>
where
    S: Service<RoleServer> + Send + 'static,
{
    fn is_tombstoned(&self, id: &SessionId) -> bool {
        lock(&self.tombstones).contains(id, Instant::now())
    }

    /// Builds a session under `id`. Returns `Ok(false)` when the id is
    /// tombstoned, and an error when the new session fails to start.
    // The session id is a routing handle, not a credential. Every request
    // passes bearer auth before it reaches rmcp, sessions are not bound to a
    // person, and any authenticated caller can already open unlimited
    // sessions, so attaching to an arbitrary UUID grants nothing new.
    //
    // A rebuilt worker numbers its request-wise streams from 0 again. For
    // about a minute after a rebuild, a client resuming a stream from before
    // it with `Last-Event-ID` can be replayed a duplicate or misrouted
    // response from its own session. That cost is accepted.
    async fn rebuild(self: Arc<Self>, id: SessionId) -> Result<bool, LocalSessionManagerError> {
        let _guard = self.rebuild_lock.lock().await;
        if self.local.sessions.read().await.contains_key(&id) {
            return Ok(true);
        }
        if self.is_tombstoned(&id) {
            return Ok(false);
        }

        // A failure below is an error, not `Ok(false)`: a 404 tells the client
        // the session is gone for good, which `mcp-remote` never recovers from.
        let service = match (self.factory)() {
            Ok(service) => service,
            Err(err) => {
                tracing::warn!(session_id = %id, ?err, "MCP session rebuild: handler factory failed");
                return Err(SessionError::Io(err).into());
            }
        };
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let (handle, worker) = create_local_session(id.clone(), self.local.session_config.clone());
        // This mirrors the task rmcp's `StreamableHttpService` spawns for a
        // session it creates, except that the exit path is `forget`, which
        // never takes `rebuild_lock`. A failed rebuild reaches it while this
        // function still holds the lock.
        tokio::spawn({
            let shared = Arc::clone(&self);
            let id = id.clone();
            async move {
                let transport = WorkerTransport::spawn(worker);
                match serve_server::<S, _, _, TransportAdapterIdentity>(service, transport).await {
                    Ok(running) => {
                        let _ = running.waiting().await;
                    }
                    Err(err) => {
                        tracing::warn!(session_id = %id, %err, "MCP session rebuild: serve failed");
                    }
                }
                shared.forget(&id, generation).await;
            }
        });

        // The worker quits unless `initialize` is the first event it sees, so
        // the handle joins the map only after initialization succeeds.
        let initialized = tokio::time::timeout(
            REBUILD_INITIALIZE_TIMEOUT,
            handle.initialize(rebuild_initialize()),
        )
        .await;
        // A handler that rejects `initialize` replies with a JSON-RPC error and
        // its service stops, so only a result counts as a started session.
        match initialized {
            Ok(Ok(ServerJsonRpcMessage::Response(_))) => {}
            Ok(Ok(reply)) => {
                tracing::warn!(session_id = %id, ?reply, "MCP session rebuild: initialize refused");
                let refused = io::Error::other("session rebuild initialize refused");
                return Err(SessionError::Io(refused).into());
            }
            Ok(Err(err)) => {
                tracing::warn!(session_id = %id, %err, "MCP session rebuild: initialize failed");
                return Err(err.into());
            }
            Err(_elapsed) => {
                tracing::warn!(session_id = %id, "MCP session rebuild: initialize timed out");
                let timeout = io::Error::new(io::ErrorKind::TimedOut, "session rebuild timed out");
                return Err(SessionError::Io(timeout).into());
            }
        }

        {
            let mut sessions = self.local.sessions.write().await;
            sessions.insert(id.clone(), handle);
            lock(&self.generations).insert(id.clone(), generation);
        }
        #[cfg(test)]
        self.rebuilds.fetch_add(1, Ordering::Relaxed);

        // A DELETE that ran before the insert tombstoned the id without
        // finding a handle, so the new session must go.
        if self.is_tombstoned(&id) {
            let removed = {
                let mut sessions = self.local.sessions.write().await;
                lock(&self.generations).remove(&id);
                sessions.remove(&id)
            };
            if let Some(handle) = removed {
                let _ = handle.close().await;
            }
            return Ok(false);
        }
        Ok(true)
    }

    // Removes a rebuilt session after its worker exits, unless a newer
    // session has replaced it. It never tombstones: an exit is not a DELETE.
    async fn forget(&self, id: &SessionId, generation: u64) {
        let removed = {
            let mut sessions = self.local.sessions.write().await;
            let mut generations = lock(&self.generations);
            if generations.get(id) != Some(&generation) {
                return;
            }
            generations.remove(id);
            sessions.remove(id)
        };
        if let Some(handle) = removed {
            let _ = handle.close().await;
        }
    }

    // Removes `id` from the map and tombstones it in one critical section, so
    // a concurrent rebuild either inserts first and its handle is returned
    // here, or inserts afterwards and sees the tombstone. Only canonical ids
    // are ever rebuilt, so only they are tombstoned; a DELETE of any other id
    // cannot push a real tombstone out at the cap. When the handle turns out
    // to belong to a reaped worker, the caller lifts the tombstone, and a
    // request for the id in between gets a 404. That window is microseconds
    // once per keep-alive expiry, so it is accepted.
    async fn take_and_tombstone(&self, id: &SessionId) -> Option<LocalSessionHandle> {
        let mut sessions = self.local.sessions.write().await;
        if is_canonical_uuid(id) {
            lock(&self.tombstones).insert(id.clone(), Instant::now());
        }
        lock(&self.generations).remove(id);
        sessions.remove(id)
    }
}

impl<S> SessionManager for ReattachingSessionManager<S>
where
    S: Service<RoleServer> + Send + 'static,
{
    type Error = LocalSessionManagerError;
    type Transport = <LocalSessionManager as SessionManager>::Transport;

    async fn create_session(&self) -> Result<(SessionId, Self::Transport), Self::Error> {
        self.shared.local.create_session().await
    }

    async fn initialize_session(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> Result<ServerJsonRpcMessage, Self::Error> {
        self.shared.local.initialize_session(id, message).await
    }

    async fn has_session(&self, id: &SessionId) -> Result<bool, Self::Error> {
        if self.shared.local.has_session(id).await? {
            return Ok(true);
        }
        if !is_canonical_uuid(id) || self.shared.is_tombstoned(id) {
            return Ok(false);
        }
        Arc::clone(&self.shared).rebuild(id.clone()).await
    }

    // rmcp calls this for a client DELETE and, for sessions it created, once
    // the worker exits. A handle that fails to close with
    // `SessionServiceTerminated` belongs to a dead worker, which is the exit
    // case, so the provisional tombstone is lifted. A DELETE that lands on an
    // already-dead worker is lifted too, so that id can be rebuilt. If it is
    // rebuilt before rmcp's pending exit task runs, that task's close ends the
    // new session and tombstones the id. Both need a DELETE to race a worker's
    // death, so they are accepted.
    async fn close_session(&self, id: &SessionId) -> Result<(), Self::Error> {
        let Some(handle) = self.shared.take_and_tombstone(id).await else {
            return Ok(());
        };
        match handle.close().await {
            Ok(()) => Ok(()),
            Err(SessionError::SessionServiceTerminated) => {
                lock(&self.shared.tombstones).remove(id);
                Ok(())
            }
            Err(err) => Err(err.into()),
        }
    }

    async fn create_stream(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> Result<impl Stream<Item = ServerSseMessage> + Send + Sync + 'static, Self::Error> {
        self.shared.local.create_stream(id, message).await
    }

    async fn accept_message(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> Result<(), Self::Error> {
        self.shared.local.accept_message(id, message).await
    }

    async fn create_standalone_stream(
        &self,
        id: &SessionId,
    ) -> Result<impl Stream<Item = ServerSseMessage> + Send + Sync + 'static, Self::Error> {
        self.shared.local.create_standalone_stream(id).await
    }

    async fn resume(
        &self,
        id: &SessionId,
        last_event_id: String,
    ) -> Result<impl Stream<Item = ServerSseMessage> + Send + Sync + 'static, Self::Error> {
        self.shared.local.resume(id, last_event_id).await
    }
}

/// Remembers session ids that clients deleted, each until it ages past `ttl`.
struct Tombstones {
    entries: HashMap<SessionId, Instant>,
    ttl: Duration,
    cap: usize,
}

impl Tombstones {
    fn new(ttl: Duration, cap: usize) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
            cap,
        }
    }

    /// Records `id` as deleted at `now`, dropping expired entries and, at the
    /// cap, the oldest entry.
    fn insert(&mut self, id: SessionId, now: Instant) {
        let ttl = self.ttl;
        self.entries
            .retain(|_, deleted_at| now.saturating_duration_since(*deleted_at) < ttl);
        if !self.entries.contains_key(&id) && self.entries.len() >= self.cap {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, deleted_at)| **deleted_at)
                .map(|(oldest, _)| oldest.clone());
            if let Some(oldest) = oldest {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(id, now);
    }

    fn contains(&self, id: &SessionId, now: Instant) -> bool {
        self.entries
            .get(id)
            .is_some_and(|deleted_at| now.saturating_duration_since(*deleted_at) < self.ttl)
    }

    fn remove(&mut self, id: &SessionId) {
        self.entries.remove(id);
    }
}

/// Reports whether `id` is a UUID in the lowercase hyphenated form rmcp
/// issues, so braced, URN, simple, and uppercase spellings are rejected.
fn is_canonical_uuid(id: &str) -> bool {
    // Of the spellings `Uuid::try_parse` accepts, only the hyphenated one is
    // 36 bytes long, so the length and case checks avoid formatting a string.
    id.len() == 36 && !id.bytes().any(|b| b.is_ascii_uppercase()) && Uuid::try_parse(id).is_ok()
}

/// Builds the `initialize` request a rebuilt session is started with.
fn rebuild_initialize() -> ClientJsonRpcMessage {
    let params = InitializeRequestParams::new(
        ClientCapabilities::default(),
        Implementation::new(REBUILD_CLIENT_NAME, env!("CARGO_PKG_VERSION")),
    )
    .with_protocol_version(ProtocolVersion::LATEST);
    ClientJsonRpcMessage::request(
        ClientRequest::InitializeRequest(Request::new(params)),
        NumberOrString::Number(0),
    )
}

// None of these mutexes is held across an await, so a poisoned one only means
// another thread panicked mid-update of a plain map, which is still usable.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::body::{to_bytes, Body};
    use axum::http::{Request as HttpRequest, StatusCode};
    use migration::MigratorTrait;
    use rmcp::transport::streamable_http_server::tower::{
        StreamableHttpServerConfig, StreamableHttpService,
    };
    use sea_orm::Database;
    use tower::ServiceExt;

    use crate::mcp::handler::FewdMcp;

    type Manager = ReattachingSessionManager<FewdMcp>;
    type Http = StreamableHttpService<FewdMcp, Manager>;

    // These tests drive the manager without the bearer middleware, so they
    // call `tools/list`, which needs no authenticated person.
    // Concurrent calls on one session need distinct JSON-RPC ids, because rmcp
    // routes each response to the stream that registered its request id.
    fn tools_list(rpc_id: u64) -> String {
        format!(r#"{{"jsonrpc":"2.0","method":"tools/list","id":{rpc_id}}}"#)
    }

    async fn fewd_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite connects");
        migration::Migrator::up(&db, None)
            .await
            .expect("migrations run on empty DB");
        db
    }

    fn manager(db: &sea_orm::DatabaseConnection, config: SessionConfig) -> Arc<Manager> {
        let db = db.clone();
        Arc::new(ReattachingSessionManager::new(config, move || {
            Ok(FewdMcp::new(db.clone()))
        }))
    }

    fn http(db: &sea_orm::DatabaseConnection, manager: &Arc<Manager>) -> Http {
        let db = db.clone();
        StreamableHttpService::new(
            move || Ok(FewdMcp::new(db.clone())),
            Arc::clone(manager),
            StreamableHttpServerConfig::default(),
        )
    }

    fn post(session_id: Option<&str>, body: &str) -> HttpRequest<Body> {
        let mut builder = HttpRequest::builder()
            .method("POST")
            .uri("/")
            .header("host", "localhost")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream");
        if let Some(id) = session_id {
            builder = builder.header("mcp-session-id", id);
        }
        builder.body(Body::from(body.to_owned())).unwrap()
    }

    // Initializes through rmcp's own create-session path and returns the id.
    async fn initialize(service: &Http) -> String {
        let init = r#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"session-manager-test","version":"0"}}}"#;
        let response = service.clone().oneshot(post(None, init)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let id = response.headers()["mcp-session-id"]
            .to_str()
            .unwrap()
            .to_owned();
        to_bytes(Body::new(response.into_body()), 64 * 1024)
            .await
            .unwrap();
        let ack = service
            .clone()
            .oneshot(post(
                Some(&id),
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(ack.status(), StatusCode::ACCEPTED);
        id
    }

    async fn assert_tools_list_succeeds(service: &Http, id: &str, rpc_id: u64) {
        let response = service
            .clone()
            .oneshot(post(Some(id), &tools_list(rpc_id)))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(Body::new(response.into_body()), 1024 * 1024)
            .await
            .unwrap();
        let body = String::from_utf8_lossy(&body);
        assert!(body.contains("\"result\""), "expected a result: {body}");
        assert!(!body.contains("\"error\""), "expected no error: {body}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_first_use_rebuilds_once() {
        let db = fewd_db().await;
        let first = manager(&db, SessionConfig::default());
        let id = initialize(&http(&db, &first)).await;

        let second = manager(&db, SessionConfig::default());
        let service = http(&db, &second);
        let calls: Vec<_> = (0..8)
            .map(|rpc_id| {
                let service = service.clone();
                let id = id.clone();
                tokio::spawn(async move { assert_tools_list_succeeds(&service, &id, rpc_id).await })
            })
            .collect();
        for call in calls {
            call.await.expect("tools/list task completes");
        }

        assert_eq!(second.shared.rebuilds.load(Ordering::Relaxed), 1);
    }

    // This drives the path rmcp owns: `initialize` without a session header
    // creates the session in `StreamableHttpService`, whose exit task calls
    // `close_session` once keep-alive reaps the worker. If that call left a
    // tombstone or the dead handle, the wait below would time out. The
    // tombstone TTL follows keep-alive, so the test lengthens it; otherwise a
    // wrong tombstone would expire before anything observed it.
    #[tokio::test]
    async fn keep_alive_expiry_does_not_tombstone() {
        let db = fewd_db().await;
        let mut config = SessionConfig::default();
        config.keep_alive = Some(Duration::from_millis(100));
        let manager = manager(&db, config);
        lock(&manager.shared.tombstones).ttl = Duration::from_secs(60);
        let service = http(&db, &manager);
        let id = initialize(&service).await;
        let session_id: SessionId = id.clone().into();

        let deadline = Instant::now() + Duration::from_secs(5);
        while manager.shared.local.has_session(&session_id).await.unwrap()
            || manager.shared.is_tombstoned(&session_id)
        {
            assert!(
                Instant::now() < deadline,
                "the reaped session left its handle or a tombstone behind"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        assert_tools_list_succeeds(&service, &id, 2).await;
        assert_eq!(manager.shared.rebuilds.load(Ordering::Relaxed), 1);
    }

    // A rebuilt session gets no exit task from rmcp, so its keep-alive reap
    // must leave the map through `forget`. A handle left behind would answer
    // every later request for the id with a 500 instead of a rebuild.
    #[tokio::test]
    async fn reaped_rebuilt_session_is_rebuilt_again() {
        let db = fewd_db().await;
        let mut config = SessionConfig::default();
        config.keep_alive = Some(Duration::from_millis(100));
        let manager = manager(&db, config);
        let service = http(&db, &manager);
        let id: SessionId = Uuid::new_v4().to_string().into();

        assert!(manager.has_session(&id).await.unwrap());
        let deadline = Instant::now() + Duration::from_secs(5);
        while manager.shared.local.has_session(&id).await.unwrap() {
            assert!(
                Instant::now() < deadline,
                "the reaped rebuilt session left its handle behind"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(lock(&manager.shared.generations).is_empty());
        assert!(!manager.shared.is_tombstoned(&id));

        assert_tools_list_succeeds(&service, &id, 2).await;
        assert_eq!(manager.shared.rebuilds.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn stale_forget_leaves_newer_rebuild_alive() {
        let db = fewd_db().await;
        let manager = manager(&db, SessionConfig::default());
        let service = http(&db, &manager);
        let id: SessionId = Uuid::new_v4().to_string().into();

        assert!(manager.has_session(&id).await.unwrap());
        let stale_generation = lock(&manager.shared.generations)[&id];

        // A DELETE that meets a dead worker removes the entry without
        // leaving a tombstone, and a later request rebuilds the id.
        let stale = manager.shared.take_and_tombstone(&id).await.unwrap();
        lock(&manager.shared.tombstones).remove(&id);
        let _ = stale.close().await;
        assert!(manager.has_session(&id).await.unwrap());

        manager.shared.forget(&id, stale_generation).await;

        assert!(manager.shared.local.has_session(&id).await.unwrap());
        assert_tools_list_succeeds(&service, &id, 2).await;
    }

    // A 404 would tell `mcp-remote` the session is gone for good, so a rebuild
    // that cannot start a session must surface as rmcp's 500 instead.
    #[tokio::test]
    async fn failed_rebuild_answers_500_not_404() {
        let db = fewd_db().await;
        let manager: Arc<Manager> = Arc::new(ReattachingSessionManager::new(
            SessionConfig::default(),
            || Err(io::Error::other("handler unavailable")),
        ));
        let service = http(&db, &manager);
        let id = Uuid::new_v4().to_string();

        let response = service
            .oneshot(post(Some(&id), &tools_list(2)))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(manager.shared.rebuilds.load(Ordering::Relaxed), 0);
    }

    struct RefusingHandler;

    impl rmcp::ServerHandler for RefusingHandler {
        async fn initialize(
            &self,
            _request: InitializeRequestParams,
            _context: rmcp::service::RequestContext<RoleServer>,
        ) -> Result<rmcp::model::InitializeResult, rmcp::ErrorData> {
            Err(rmcp::ErrorData::internal_error("initialize refused", None))
        }
    }

    // A handler that rejects `initialize` leaves no running service, so a
    // registered handle would answer every later request for the id with a
    // 500 and never be rebuilt.
    #[tokio::test]
    async fn refused_initialize_fails_rebuild() {
        let manager =
            ReattachingSessionManager::new(SessionConfig::default(), || Ok(RefusingHandler));
        let id: SessionId = Uuid::new_v4().to_string().into();

        assert!(manager.has_session(&id).await.is_err());
        assert!(!manager.shared.local.has_session(&id).await.unwrap());
        assert!(lock(&manager.shared.generations).is_empty());
    }

    #[tokio::test]
    async fn deleted_rebuilt_session_is_not_rebuilt() {
        let db = fewd_db().await;
        let manager = manager(&db, SessionConfig::default());
        let id: SessionId = Uuid::new_v4().to_string().into();

        assert!(manager.has_session(&id).await.unwrap());
        manager.close_session(&id).await.unwrap();

        assert!(!manager.has_session(&id).await.unwrap());
        assert_eq!(manager.shared.rebuilds.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn tombstones_expire_after_ttl() {
        let start = Instant::now();
        let mut tombstones = Tombstones::new(Duration::from_secs(10), 8);
        let id: SessionId = "a".into();
        tombstones.insert(id.clone(), start);

        assert!(tombstones.contains(&id, start + Duration::from_secs(9)));
        assert!(!tombstones.contains(&id, start + Duration::from_secs(10)));

        tombstones.insert("b".into(), start + Duration::from_secs(11));
        assert!(
            !tombstones.entries.contains_key(&id),
            "expired entry pruned"
        );
    }

    #[test]
    fn tombstones_evict_oldest_at_cap() {
        let start = Instant::now();
        let mut tombstones = Tombstones::new(Duration::from_secs(60), 2);
        tombstones.insert("a".into(), start);
        tombstones.insert("b".into(), start + Duration::from_secs(1));
        tombstones.insert("c".into(), start + Duration::from_secs(2));

        let now = start + Duration::from_secs(3);
        assert_eq!(tombstones.entries.len(), 2);
        assert!(!tombstones.contains(&"a".into(), now));
        assert!(tombstones.contains(&"b".into(), now));
        assert!(tombstones.contains(&"c".into(), now));
    }

    #[test]
    fn session_id_must_be_canonical_uuid() {
        let id = Uuid::new_v4();
        assert!(is_canonical_uuid(&id.hyphenated().to_string()));
        assert!(!is_canonical_uuid(
            &id.hyphenated().to_string().to_uppercase()
        ));
        assert!(!is_canonical_uuid(&id.simple().to_string()));
        assert!(!is_canonical_uuid(&id.braced().to_string()));
        assert!(!is_canonical_uuid(&id.urn().to_string()));
        assert!(!is_canonical_uuid("not-a-uuid"));
        assert!(!is_canonical_uuid(""));
    }
}
