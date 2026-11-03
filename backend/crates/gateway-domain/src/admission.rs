//! Admission classes: which budget a request spends.
//!
//! One global limit cannot express "login keeps working while a reset flood is
//! in progress". Every request is classified by method and matched route
//! template, and each class has its own per-network budget. Optional anonymous
//! identity and verified-principal budgets add independent dimensions without
//! coupling one route class to another.

use std::fmt;

/// Budget window. Budgets are requests per client network per window.
pub const ADMISSION_WINDOW_SECONDS: u32 = 60;

/// The budget a request spends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RouteClass {
    /// Password and OAuth sign-in, including the OAuth authorize step.
    Login,
    /// Signup, email availability, and email verification.
    Signup,
    /// Password reset request and confirmation.
    Reset,
    /// Resending a verification email.
    VerificationResend,
    /// Refresh-token rotation.
    Refresh,
    /// Profile and post search.
    Search,
    /// Every other mutation.
    Write,
    /// Every other read.
    Read,
}

impl RouteClass {
    pub const ALL: [RouteClass; 8] = [
        Self::Login,
        Self::Signup,
        Self::Reset,
        Self::VerificationResend,
        Self::Refresh,
        Self::Search,
        Self::Write,
        Self::Read,
    ];

    /// Classify a request by method and matched route template.
    ///
    /// `route` is the template the router matched, so a path parameter cannot
    /// move a request into a cheaper class. The parity-only `/shadow` copies of
    /// auth routes share the class of the route they mirror.
    pub fn classify(method: &str, route: &str) -> Self {
        let route = route.strip_prefix("/shadow").unwrap_or(route);
        match route {
            "/auth/login"
            | "/auth/oauth/:provider/authorize"
            | "/auth/oauth/:provider/callback"
            | "/auth/oauth/:provider/callback/complete" => return Self::Login,
            "/auth/email/signup" | "/auth/email/check" | "/auth/email/verify" => {
                return Self::Signup
            }
            "/auth/password/reset/request" | "/auth/password/reset/confirm" => return Self::Reset,
            "/auth/email/resend" => return Self::VerificationResend,
            "/auth/refresh" => return Self::Refresh,
            _ => {}
        }
        if route.ends_with("/search") {
            return Self::Search;
        }
        match method {
            "GET" | "HEAD" | "OPTIONS" => Self::Read,
            _ => Self::Write,
        }
    }

    /// Stable name used in settings and metric labels.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Signup => "signup",
            Self::Reset => "reset",
            Self::VerificationResend => "verification_resend",
            Self::Refresh => "refresh",
            Self::Search => "search",
            Self::Write => "write",
            Self::Read => "read",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|class| class.as_str() == name.trim())
    }

    /// Default requests per client network per window.
    ///
    /// Auth classes are small because each admitted request can cost a password
    /// hash or an email; reads are the general budget
    /// (`GATEWAY_RATE_LIMIT_PER_MINUTE`).
    pub fn default_budget(self) -> u32 {
        match self {
            Self::Login => 30,
            Self::Signup => 10,
            Self::Reset => 10,
            Self::VerificationResend => 5,
            Self::Refresh => 120,
            Self::Search => 120,
            Self::Write => 300,
            Self::Read => 600,
        }
    }

    /// Per-process concurrency ceilings. Auth's login pool is aligned with the
    /// durable-journal authentication capacity; expensive email routes have a
    /// smaller independent ceiling, while cheap reads can fan out further.
    pub fn default_max_in_flight(self) -> usize {
        match self {
            Self::Login | Self::Refresh => 32,
            Self::Signup | Self::Reset | Self::VerificationResend => 8,
            Self::Search => 32,
            Self::Write => 64,
            Self::Read => 128,
        }
    }

    /// What admission does when the shared budget store cannot answer.
    ///
    /// Only ordinary reads may continue, on a small per-instance allowance.
    /// Auth and mutations fail closed: admitting them without a budget would
    /// let an outage of the store become an outage of the abuse limits.
    pub fn store_failure(self) -> StoreFailurePolicy {
        match self {
            Self::Read => StoreFailurePolicy::BoundedLocalFallback,
            _ => StoreFailurePolicy::Reject,
        }
    }

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|class| *class == self)
            .expect("every class is listed")
    }
}

impl fmt::Display for RouteClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Behaviour when the shared budget store fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreFailurePolicy {
    /// No budget means no admission.
    Reject,
    /// Admit from a small per-instance allowance
    /// (`AdmissionPolicies::fallback_per_second`). Across a fleet the most this
    /// admits is replicas × that allowance; it never grants a client a fresh
    /// quota, because nothing is written to the store.
    BoundedLocalFallback,
}

/// Admission settings for every class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionMode {
    /// Enforce quota and cardinality decisions.
    Enforce,
    /// Observe quota and cardinality decisions without refusing requests.
    Shadow,
}

/// Which independently keyed dimension a request spends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AdmissionScope {
    Network,
    Identity,
    Principal,
}

impl AdmissionScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::Identity => "identity",
            Self::Principal => "principal",
        }
    }
}

impl AdmissionMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "enforce" => Some(Self::Enforce),
            "shadow" => Some(Self::Shadow),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enforce => "enforce",
            Self::Shadow => "shadow",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionPolicies {
    budgets: [u32; 8],
    /// Optional budgets keyed by an HMAC of a normalized anonymous account
    /// identifier. Zero disables that class's identity budget.
    identity_budgets: [u32; 8],
    /// Optional budgets keyed by an HMAC of a verified access-token subject.
    /// Zero disables that class's authenticated-principal budget.
    principal_budgets: [u32; 8],
    /// Maximum simultaneous requests per route class on each Gateway process.
    max_in_flight: [usize; 8],
    /// Whether quota and cardinality decisions are enforced or observed only.
    pub mode: AdmissionMode,
    /// While detailed capacity is full, identities without a budget of their
    /// own share one budget per class of `overflow_factor` × the class budget.
    pub overflow_factor: u32,
    /// Per-instance reads per second admitted while the store is unavailable.
    /// Zero makes reads fail closed as well.
    pub fallback_per_second: u32,
    /// IPv6 clients are keyed by this prefix, so one host cycling through its
    /// allocation does not receive a fresh budget per address.
    pub v6_prefix_length: u8,
}

pub const DEFAULT_OVERFLOW_FACTOR: u32 = 10;
pub const DEFAULT_FALLBACK_PER_SECOND: u32 = 20;
pub const DEFAULT_V6_PREFIX_LENGTH: u8 = 64;

impl Default for AdmissionPolicies {
    fn default() -> Self {
        let mut budgets = [0; 8];
        let mut max_in_flight = [0; 8];
        for class in RouteClass::ALL {
            budgets[class.index()] = class.default_budget();
            max_in_flight[class.index()] = class.default_max_in_flight();
        }
        Self {
            budgets,
            identity_budgets: [0; 8],
            principal_budgets: [0; 8],
            max_in_flight,
            mode: AdmissionMode::Enforce,
            overflow_factor: DEFAULT_OVERFLOW_FACTOR,
            fallback_per_second: DEFAULT_FALLBACK_PER_SECOND,
            v6_prefix_length: DEFAULT_V6_PREFIX_LENGTH,
        }
    }
}

impl AdmissionPolicies {
    /// Every class with the same budget.
    pub fn uniform(budget: u32) -> Self {
        Self {
            budgets: [budget; 8],
            ..Self::default()
        }
    }

    /// Requests per client network per window; zero leaves the class unlimited.
    pub fn budget(&self, class: RouteClass) -> u32 {
        self.budgets[class.index()]
    }

    pub fn with_budget(mut self, class: RouteClass, budget: u32) -> Self {
        self.budgets[class.index()] = budget;
        self
    }

    /// Requests per normalized anonymous identity per window; zero disables it.
    pub fn identity_budget(&self, class: RouteClass) -> u32 {
        self.identity_budgets[class.index()]
    }

    pub fn has_identity_budgets(&self) -> bool {
        self.identity_budgets.iter().any(|budget| *budget > 0)
    }

    pub fn with_identity_budget(mut self, class: RouteClass, budget: u32) -> Self {
        self.identity_budgets[class.index()] = budget;
        self
    }

    /// Requests per verified access-token subject per window; zero disables it.
    pub fn principal_budget(&self, class: RouteClass) -> u32 {
        self.principal_budgets[class.index()]
    }

    pub fn has_principal_budgets(&self) -> bool {
        self.principal_budgets.iter().any(|budget| *budget > 0)
    }

    pub fn with_principal_budget(mut self, class: RouteClass, budget: u32) -> Self {
        self.principal_budgets[class.index()] = budget;
        self
    }

    /// Maximum simultaneous requests per Gateway process; zero is not allowed.
    pub fn max_in_flight(&self, class: RouteClass) -> usize {
        self.max_in_flight[class.index()]
    }

    pub fn with_max_in_flight(mut self, class: RouteClass, limit: usize) -> Self {
        self.max_in_flight[class.index()] = limit;
        self
    }

    pub fn with_mode(mut self, mode: AdmissionMode) -> Self {
        self.mode = mode;
        self
    }

    /// The shared budget for identities admitted while capacity is full.
    pub fn overflow_budget(&self, class: RouteClass) -> u32 {
        self.budget(class).saturating_mul(self.overflow_factor)
    }

    /// Apply `class=requests` overrides, as in
    /// `GATEWAY_ADMISSION_BUDGETS=login=20,reset=5`.
    ///
    /// The read budget is `GATEWAY_RATE_LIMIT_PER_MINUTE` and cannot also be
    /// set here, so one setting owns it.
    pub fn with_overrides(mut self, overrides: &str) -> Result<Self, String> {
        for entry in overrides
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            let (name, value) = entry
                .split_once('=')
                .ok_or_else(|| format!("expected class=requests, found \"{entry}\""))?;
            let class = RouteClass::parse(name).ok_or_else(|| {
                format!(
                    "unknown admission class \"{}\"; expected one of {}",
                    name.trim(),
                    RouteClass::ALL.map(RouteClass::as_str).join(", ")
                )
            })?;
            if class == RouteClass::Read {
                return Err("the read budget is GATEWAY_RATE_LIMIT_PER_MINUTE".into());
            }
            let budget = value
                .trim()
                .parse::<u32>()
                .map_err(|_| format!("budget for {class} must be a whole number"))?;
            if budget == 0 || budget > 100_000 {
                return Err(format!("budget for {class} must be between 1 and 100000"));
            }
            self.budgets[class.index()] = budget;
        }
        Ok(self)
    }

    /// Apply anonymous identity-budget overrides. Only classes whose request
    /// body carries an email address support this second key dimension.
    pub fn with_identity_overrides(mut self, overrides: &str) -> Result<Self, String> {
        for entry in overrides
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            let (name, value) = entry
                .split_once('=')
                .ok_or_else(|| format!("expected class=requests, found \"{entry}\""))?;
            let class = RouteClass::parse(name).ok_or_else(|| {
                format!(
                    "unknown admission class \"{}\"; expected one of {}",
                    name.trim(),
                    RouteClass::ALL.map(RouteClass::as_str).join(", ")
                )
            })?;
            if !matches!(
                class,
                RouteClass::Login
                    | RouteClass::Signup
                    | RouteClass::Reset
                    | RouteClass::VerificationResend
            ) {
                return Err(format!(
                    "identity admission is not supported for the {class} class"
                ));
            }
            let budget = value
                .trim()
                .parse::<u32>()
                .map_err(|_| format!("identity budget for {class} must be a whole number"))?;
            if budget == 0 || budget > 100_000 {
                return Err(format!(
                    "identity budget for {class} must be between 1 and 100000"
                ));
            }
            self.identity_budgets[class.index()] = budget;
        }
        Ok(self)
    }

    /// Apply authenticated-principal overrides. Only route classes that may
    /// serve actor-scoped search/read/write requests are accepted.
    pub fn with_principal_overrides(mut self, overrides: &str) -> Result<Self, String> {
        for entry in overrides
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            let (name, value) = entry
                .split_once('=')
                .ok_or_else(|| format!("expected class=requests, found \"{entry}\""))?;
            let class = RouteClass::parse(name).ok_or_else(|| {
                format!(
                    "unknown admission class \"{}\"; expected one of {}",
                    name.trim(),
                    RouteClass::ALL.map(RouteClass::as_str).join(", ")
                )
            })?;
            if !matches!(
                class,
                RouteClass::Search | RouteClass::Write | RouteClass::Read
            ) {
                return Err(format!(
                    "principal admission is not supported for the {class} class"
                ));
            }
            let budget = value
                .trim()
                .parse::<u32>()
                .map_err(|_| format!("principal budget for {class} must be a whole number"))?;
            if budget == 0 || budget > 100_000 {
                return Err(format!(
                    "principal budget for {class} must be between 1 and 100000"
                ));
            }
            self.principal_budgets[class.index()] = budget;
        }
        Ok(self)
    }

    /// Apply per-process concurrent-request limits. A full class refuses new
    /// requests immediately rather than queueing work behind a saturated class.
    pub fn with_max_in_flight_overrides(mut self, overrides: &str) -> Result<Self, String> {
        for entry in overrides
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            let (name, value) = entry
                .split_once('=')
                .ok_or_else(|| format!("expected class=limit, found \"{entry}\""))?;
            let class = RouteClass::parse(name).ok_or_else(|| {
                format!(
                    "unknown admission class \"{}\"; expected one of {}",
                    name.trim(),
                    RouteClass::ALL.map(RouteClass::as_str).join(", ")
                )
            })?;
            let limit = value
                .trim()
                .parse::<usize>()
                .map_err(|_| format!("in-flight limit for {class} must be a whole number"))?;
            if !(1..=10_000).contains(&limit) {
                return Err(format!(
                    "in-flight limit for {class} must be between 1 and 10000"
                ));
            }
            self.max_in_flight[class.index()] = limit;
        }
        Ok(self)
    }
}

/// What admission decided, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionOutcome {
    /// Within the client's own budget.
    Admitted,
    /// Detailed capacity was full; admitted from the class's shared overflow
    /// budget without creating a budget for this client.
    AdmittedOverflow,
    /// The store was unavailable; admitted from the per-instance allowance.
    AdmittedFallback,
    /// The client's budget for this class is spent.
    RejectedQuota,
    /// Detailed capacity and the class's overflow budget are both full.
    RejectedCardinality,
    /// The store was unavailable and this class fails closed.
    RejectedStoreUnavailable,
    /// The store was unavailable and the per-instance allowance is spent.
    RejectedFallbackExhausted,
}

impl AdmissionOutcome {
    pub fn is_admitted(self) -> bool {
        matches!(
            self,
            Self::Admitted | Self::AdmittedOverflow | Self::AdmittedFallback
        )
    }

    /// True when the refusal is the store's fault rather than the client's.
    pub fn is_store_failure(self) -> bool {
        matches!(
            self,
            Self::RejectedStoreUnavailable | Self::RejectedFallbackExhausted
        )
    }

    /// Shadow mode bypasses caller-budget refusals only. Store failures remain
    /// subject to the configured outage policy.
    pub fn is_shadowable_rejection(self) -> bool {
        matches!(self, Self::RejectedQuota | Self::RejectedCardinality)
    }

    /// Metric label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::AdmittedOverflow => "admitted_overflow",
            Self::AdmittedFallback => "admitted_fallback",
            Self::RejectedQuota => "rejected_quota",
            Self::RejectedCardinality => "rejected_cardinality",
            Self::RejectedStoreUnavailable => "rejected_store_unavailable",
            Self::RejectedFallbackExhausted => "rejected_fallback_exhausted",
        }
    }
}
