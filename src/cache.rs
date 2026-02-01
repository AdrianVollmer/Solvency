use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;

use crate::config::AuthMode;
use crate::db::queries::{accounts, categories, settings as db_settings, tags};
use crate::db::DbPool;
use crate::error::AppResult;
use crate::models::{Account, Category, CategoryWithPath, Settings, Tag};
use crate::state::AppState;

struct Slot<T> {
    inner: RwLock<Option<(u64, T)>>,
}

impl<T: Clone> Slot<T> {
    fn new() -> Self {
        Self {
            inner: RwLock::new(None),
        }
    }

    fn get(&self, gen: u64) -> Option<T> {
        let guard = self.inner.read().ok()?;
        match guard.as_ref() {
            Some((stored_gen, val)) if *stored_gen == gen => Some(val.clone()),
            _ => None,
        }
    }

    fn set(&self, gen: u64, val: T) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = Some((gen, val));
        }
    }
}

pub struct AppCache {
    generation: AtomicU64,
    settings: Slot<Settings>,
    categories_with_path: Slot<Vec<CategoryWithPath>>,
    categories: Slot<Vec<Category>>,
    tags: Slot<Vec<Tag>>,
    accounts: Slot<Vec<Account>>,
    cash_accounts: Slot<Vec<Account>>,
}

impl Default for AppCache {
    fn default() -> Self {
        Self::new()
    }
}

impl AppCache {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            settings: Slot::new(),
            categories_with_path: Slot::new(),
            categories: Slot::new(),
            tags: Slot::new(),
            accounts: Slot::new(),
            cash_accounts: Slot::new(),
        }
    }

    pub fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    fn gen(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    pub fn load_settings(&self, pool: &DbPool, auth_mode: &AuthMode) -> AppResult<Settings> {
        let gen = self.gen();
        if let Some(cached) = self.settings.get(gen) {
            return Ok(cached);
        }
        let conn = pool.get()?;
        let mut settings = db_settings::get_settings(&conn)?;
        settings.is_authenticated = matches!(auth_mode, AuthMode::Password(_));
        self.settings.set(gen, settings.clone());
        Ok(settings)
    }

    pub fn load_categories_with_path(&self, pool: &DbPool) -> AppResult<Vec<CategoryWithPath>> {
        let gen = self.gen();
        if let Some(cached) = self.categories_with_path.get(gen) {
            return Ok(cached);
        }
        let conn = pool.get()?;
        let val = categories::list_categories_with_path(&conn)?;
        self.categories_with_path.set(gen, val.clone());
        Ok(val)
    }

    pub fn load_categories(&self, pool: &DbPool) -> AppResult<Vec<Category>> {
        let gen = self.gen();
        if let Some(cached) = self.categories.get(gen) {
            return Ok(cached);
        }
        let conn = pool.get()?;
        let val = categories::list_categories(&conn)?;
        self.categories.set(gen, val.clone());
        Ok(val)
    }

    pub fn load_tags(&self, pool: &DbPool) -> AppResult<Vec<Tag>> {
        let gen = self.gen();
        if let Some(cached) = self.tags.get(gen) {
            return Ok(cached);
        }
        let conn = pool.get()?;
        let val = tags::list_tags(&conn)?;
        self.tags.set(gen, val.clone());
        Ok(val)
    }

    pub fn load_accounts(&self, pool: &DbPool) -> AppResult<Vec<Account>> {
        let gen = self.gen();
        if let Some(cached) = self.accounts.get(gen) {
            return Ok(cached);
        }
        let conn = pool.get()?;
        let val = accounts::list_accounts(&conn)?;
        self.accounts.set(gen, val.clone());
        Ok(val)
    }

    pub fn load_cash_accounts(&self, pool: &DbPool) -> AppResult<Vec<Account>> {
        let gen = self.gen();
        if let Some(cached) = self.cash_accounts.get(gen) {
            return Ok(cached);
        }
        let conn = pool.get()?;
        let val = accounts::list_accounts_by_type(&conn, crate::models::AccountType::Cash)?;
        self.cash_accounts.set(gen, val.clone());
        Ok(val)
    }
}

pub async fn cache_invalidation_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let mutating = matches!(
        *req.method(),
        axum::http::Method::POST
            | axum::http::Method::PUT
            | axum::http::Method::DELETE
            | axum::http::Method::PATCH
    );
    let resp = next.run(req).await;
    if mutating && resp.status().is_success() {
        state.cache.invalidate();
    }
    resp
}
