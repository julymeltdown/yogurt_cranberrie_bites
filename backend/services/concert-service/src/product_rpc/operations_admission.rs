//! Exact C04 administration route admission. Parsing grants no operator role.
use super::{canonical_uuid, proto::ProductRequest};
use tonic::Status;
use uuid::Uuid;

const MAX_BODY: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationsOperation {
    SettingsRead,
    SettingsWrite,
    PhaseWrite,
    PlacesRead,
    PlaceWrite(Uuid),
    NoticeRead,
    NoticeDetail(Uuid),
    NoticeAdminRead,
    NoticeCreate,
    NoticeUpdate(Uuid),
    ReportQueue,
    ReportDetail(Uuid),
}
impl OperationsOperation {
    pub fn id(self) -> &'static str {
        match self {
            Self::SettingsRead => "getConcertSettings",
            Self::SettingsWrite => "setConcertSettings",
            Self::PhaseWrite => "setConcertEventPhase",
            Self::PlacesRead => "listConcertAdminPlaces",
            Self::PlaceWrite(_) => "setConcertPlace",
            Self::NoticeRead => "listConcertNotices",
            Self::NoticeDetail(_) => "getConcertNotice",
            Self::NoticeAdminRead => "listConcertAdminNotices",
            Self::NoticeCreate => "createConcertNotice",
            Self::NoticeUpdate(_) => "updateConcertNotice",
            Self::ReportQueue => "listConcertAdminReports",
            Self::ReportDetail(_) => "getConcertAdminReport",
        }
    }
    pub fn mutation(self) -> bool {
        matches!(
            self,
            Self::SettingsWrite | Self::PhaseWrite | Self::PlaceWrite(_) | Self::NoticeCreate
                | Self::NoticeUpdate(_)
        )
    }
    pub fn route(event: Uuid, method: &str, target: &str) -> Result<Self, Status> {
        if event.is_nil()
            || target.len() > 4096
            || !target.is_ascii()
            || target.chars().any(char::is_control)
            || target.contains('#')
        {
            return Err(invalid());
        }
        let (path, _) = target
            .split_once('?')
            .map_or((target, None), |(path, query)| (path, Some(query)));
        let root = format!("/api/concert/events/{event}/");
        let route = path.strip_prefix(&root).ok_or_else(invalid)?;
        match (method, route) {
            ("GET", "admin/settings") => Ok(Self::SettingsRead),
            ("GET", "admin/places") => Ok(Self::PlacesRead),
            ("GET", "admin/notices") => Ok(Self::NoticeAdminRead),
            ("GET", "notices") => Ok(Self::NoticeRead),
            ("GET", "admin/reports") => Ok(Self::ReportQueue),
            ("GET", r) if r.starts_with("admin/reports/") => {
                Ok(Self::ReportDetail(canonical_uuid(&r[14..])?))
            }
            ("GET", r) if r.starts_with("notices/") => {
                Ok(Self::NoticeDetail(canonical_uuid(&r[8..])?))
            }
            ("PUT", "admin/settings") => Ok(Self::SettingsWrite),
            ("POST", "admin/phase") => Ok(Self::PhaseWrite),
            ("POST", "admin/notices") => Ok(Self::NoticeCreate),
            ("PUT", r) if r.starts_with("admin/notices/") => {
                Ok(Self::NoticeUpdate(canonical_uuid(&r[14..])?))
            }
            ("PUT", r) if r.starts_with("admin/places/") => {
                Ok(Self::PlaceWrite(canonical_uuid(&r[13..])?))
            }
            _ => Err(invalid()),
        }
    }
    pub fn validate(self, request: &ProductRequest) -> Result<(), Status> {
        let event = canonical_uuid(&request.event_id)?;
        canonical_uuid(&request.user_id)?;
        if Self::route(event, &request.http_method, &request.http_target)? != self {
            return Err(invalid());
        }
        if self.mutation() {
            if request.http_body.is_empty()
                || request.http_body.len() > MAX_BODY
                || request.content_type != "application/json"
            {
                return Err(invalid());
            }
            canonical_uuid(&request.idempotency_key)?;
        } else if !request.http_body.is_empty()
            || !request.content_type.is_empty()
            || !request.idempotency_key.is_empty()
        {
            return Err(invalid());
        }
        Ok(())
    }
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid Concert operations request")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product_rpc::Operation;

    fn target(event: Uuid, suffix: &str) -> String {
        format!("/api/concert/events/{event}/{suffix}")
    }
    #[test]
    fn every_registered_admin_route_reaches_the_operations_owner() {
        let event = Uuid::new_v4();
        let id = Uuid::new_v4();
        for (method, suffix, expected) in [
            ("GET", "admin/settings", "getConcertSettings"),
            ("PUT", "admin/settings", "setConcertSettings"),
            ("POST", "admin/phase", "setConcertEventPhase"),
            ("GET", "admin/places", "listConcertAdminPlaces"),
            ("POST", "admin/notices", "createConcertNotice"),
            ("GET", "admin/notices", "listConcertAdminNotices"),
            ("GET", "notices", "listConcertNotices"),
            ("GET", "admin/reports", "listConcertAdminReports"),
        ] {
            let route = target(event, suffix);
            let operation = crate::product_rpc::operation(event, method, &route)
                .unwrap_or_else(|_| panic!("{method} {suffix}"));
            assert_eq!(operation.contract_id(), expected, "{method} {suffix}");
            if let Operation::Operations(value) = operation {
                assert_eq!(value.id(), expected);
            }
        }
        let routes = [
            ("PUT", format!("admin/places/{id}"), "setConcertPlace"),
            ("PUT", format!("admin/notices/{id}"), "updateConcertNotice"),
            ("GET", format!("admin/reports/{id}"), "getConcertAdminReport"),
            ("GET", format!("notices/{id}"), "getConcertNotice"),
        ];
        for (method, suffix, expected) in routes {
            let route = target(event, &suffix);
            let operation = crate::product_rpc::operation(event, method, &route).unwrap();
            assert_eq!(operation.contract_id(), expected);
            assert!(matches!(operation, Operation::Operations(_)));
        }
    }

    #[test]
    fn unrelated_or_wrong_method_routes_are_not_operations() {
        let event = Uuid::new_v4();
        for (method, suffix) in [
            ("POST", "admin/settings"),
            ("GET", "admin/phase"),
            ("DELETE", "admin/notices"),
            ("PUT", "notices"),
            ("GET", "admin/places/not-a-uuid"),
            ("GET", "admin/notices/unknown"),
        ] {
            assert!(
                OperationsOperation::route(event, method, &target(event, suffix)).is_err(),
                "{method} {suffix}"
            );
        }
        assert!(OperationsOperation::route(Uuid::nil(), "GET", "/api/concert/events/x").is_err());
    }

    #[test]
    fn writes_require_a_canonical_key_and_reads_refuse_payloads() {
        let event = Uuid::new_v4();
        let mut request = crate::product_rpc::proto::ProductRequest {
            user_id: Uuid::new_v4().to_string(),
            event_id: event.to_string(),
            session_family_id: Uuid::new_v4().to_string(),
            request_id: Uuid::new_v4().to_string(),
            http_method: "PUT".into(),
            http_target: target(event, "admin/settings"),
            http_body: br"{}".to_vec(),
            content_type: "application/json".into(),
            idempotency_key: Uuid::new_v4().to_string(),
            ..Default::default()
        };
        assert!(OperationsOperation::SettingsWrite
            .validate(&request)
            .is_ok());
        request.idempotency_key.clear();
        assert!(OperationsOperation::SettingsWrite
            .validate(&request)
            .is_err());
        request.http_method = "GET".into();
        request.http_body.clear();
        request.content_type.clear();
        assert!(OperationsOperation::SettingsRead
            .validate(&request)
            .is_ok());
        request.http_body = b"x".to_vec();
        assert!(OperationsOperation::SettingsRead
            .validate(&request)
            .is_err());
    }
}
