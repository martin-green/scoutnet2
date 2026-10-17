//! Behörighet: rollkatalog och principal.
//!
//! Prototypen implementerar samma semantik som Cedar-policyerna i planen (bilaga C):
//! en roll på en enhet ger behörigheter på enheten och allt under den i trädet.

use std::{
    collections::{BTreeSet, HashMap},
    sync::LazyLock,
};

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    BodyRead,
    MembersRead,
    MembersContactRead,
    PersonalRead,
    NationalIdRead,
    MembersManage,
    AttendanceRead,
    AttendanceWrite,
    ReportsRead,
    AuditRead,
    /// Avgifter och betalstatus.
    EconomyRead,
}

#[derive(Debug)]
pub struct RoleDef {
    pub key: &'static str,
    pub label: &'static str,
    pub permissions: &'static [Permission],
}

use Permission::{
    AttendanceRead, AttendanceWrite, AuditRead, BodyRead, EconomyRead, MembersContactRead, MembersManage, MembersRead,
    NationalIdRead, PersonalRead, ReportsRead,
};

const ROLE_DEFS: &[RoleDef] = &[
    RoleDef {
        key: "forbundsadmin",
        label: "Förbundsadministratör",
        permissions: &[
            BodyRead,
            MembersRead,
            MembersContactRead,
            PersonalRead,
            NationalIdRead,
            MembersManage,
            AttendanceRead,
            ReportsRead,
            AuditRead,
            EconomyRead,
        ],
    },
    RoleDef {
        key: "distriktsordforande",
        label: "Distriktsordförande",
        permissions: &[BodyRead, MembersRead, ReportsRead],
    },
    RoleDef {
        key: "karordforande",
        label: "Kårordförande",
        permissions: &[
            BodyRead,
            MembersRead,
            MembersContactRead,
            PersonalRead,
            MembersManage,
            AttendanceRead,
            ReportsRead,
            AuditRead,
            EconomyRead,
        ],
    },
    RoleDef {
        key: "medlemsregistrerare",
        label: "Medlemsregistrerare",
        permissions: &[
            BodyRead,
            MembersRead,
            MembersContactRead,
            PersonalRead,
            NationalIdRead,
            MembersManage,
            AttendanceRead,
            ReportsRead,
            EconomyRead,
        ],
    },
    RoleDef {
        key: "kassor",
        label: "Kårkassör",
        permissions: &[BodyRead, MembersRead, MembersContactRead, ReportsRead, EconomyRead],
    },
    RoleDef {
        key: "avdelningsledare",
        label: "Avdelningsledare",
        permissions: &[
            BodyRead,
            MembersRead,
            MembersContactRead,
            PersonalRead,
            AttendanceRead,
            AttendanceWrite,
            ReportsRead,
        ],
    },
    RoleDef {
        key: "ledare",
        label: "Ledare",
        permissions: &[BodyRead, MembersRead, MembersContactRead, AttendanceRead, AttendanceWrite],
    },
    RoleDef {
        key: "assisterande_ledare",
        label: "Assisterande ledare",
        permissions: &[BodyRead, MembersRead, AttendanceWrite],
    },
    RoleDef { key: "patrulledare", label: "Patrulledare", permissions: &[BodyRead, MembersRead] },
];

static ROLES: LazyLock<HashMap<&'static str, &'static RoleDef>> =
    LazyLock::new(|| ROLE_DEFS.iter().map(|r| (r.key, r)).collect());

pub fn role(key: &str) -> Option<&'static RoleDef> {
    ROLES.get(key).copied()
}

pub fn role_label(key: &str) -> &str {
    role(key).map_or(key, |r| r.label)
}

/// En rolltilldelning på en enhet i trädet.
#[derive(Debug, Clone)]
pub struct Grant {
    pub body_id: Uuid,
    pub body_path: String,
    pub body_name: String,
    pub body_kind: String,
    pub body_number: i32,
    pub role_key: String,
}

/// Den inloggade användaren med sina aktuella rolltilldelningar.
#[derive(Debug, Clone)]
pub struct Principal {
    pub account_id: Uuid,
    pub person_id: Uuid,
    pub member_no: i32,
    pub display_name: String,
    pub auth_level: String,
    pub auth_time: DateTime<Utc>,
    pub grants: Vec<Grant>,
}

/// Sant om `path` är `root` eller ligger under `root` i ltree-trädet.
pub fn is_within(path: &str, root: &str) -> bool {
    path == root || (path.starts_with(root) && path.as_bytes().get(root.len()) == Some(&b'.'))
}

impl Principal {
    fn grants_with(&self, perm: Permission) -> impl Iterator<Item = &Grant> {
        self.grants.iter().filter(move |g| role(&g.role_key).is_some_and(|r| r.permissions.contains(&perm)))
    }

    /// Minimala rötter (ltree-sökvägar) där behörigheten gäller.
    pub fn roots_for(&self, perm: Permission) -> Vec<String> {
        let mut paths: Vec<&str> = self.grants_with(perm).map(|g| g.body_path.as_str()).collect();
        paths.sort_unstable_by_key(|p| p.len());
        let mut roots: Vec<String> = Vec::new();
        for p in paths {
            if !roots.iter().any(|r| is_within(p, r)) {
                roots.push(p.to_owned());
            }
        }
        roots
    }

    pub fn can(&self, perm: Permission, path: &str) -> bool {
        self.grants_with(perm).any(|g| is_within(path, &g.body_path))
    }

    pub fn permissions_on(&self, path: &str) -> BTreeSet<Permission> {
        self.grants
            .iter()
            .filter(|g| is_within(path, &g.body_path))
            .filter_map(|g| role(&g.role_key))
            .flat_map(|r| r.permissions.iter().copied())
            .collect()
    }

    /// Step-up: känsliga åtgärder kräver färsk autentisering (max 15 min).
    /// I produktion kontrolleras även `acr` (MFA-nivå) från `ScoutID`.
    pub fn has_recent_auth(&self) -> bool {
        Utc::now().signed_duration_since(self.auth_time).num_minutes() < 15
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(grants: &[(&str, &str)]) -> Principal {
        Principal {
            account_id: Uuid::nil(),
            person_id: Uuid::nil(),
            member_no: 1,
            display_name: "Test".into(),
            auth_level: "dev".into(),
            auth_time: Utc::now(),
            grants: grants
                .iter()
                .map(|(path, role)| Grant {
                    body_id: Uuid::nil(),
                    body_path: (*path).into(),
                    body_name: String::new(),
                    body_kind: String::new(),
                    body_number: 0,
                    role_key: (*role).into(),
                })
                .collect(),
        }
    }

    #[test]
    fn path_containment_respects_label_boundaries() {
        assert!(is_within("b1.b2.b3", "b1.b2"));
        assert!(is_within("b1.b2", "b1.b2"));
        assert!(!is_within("b1.b22", "b1.b2"));
        assert!(!is_within("b1", "b1.b2"));
    }

    #[test]
    fn leader_sees_only_own_troop() {
        let p = principal(&[("b1.b2.b3.b10", "avdelningsledare")]);
        assert!(p.can(Permission::MembersRead, "b1.b2.b3.b10.b11"));
        assert!(!p.can(Permission::MembersRead, "b1.b2.b3.b12"));
        assert!(!p.can(Permission::NationalIdRead, "b1.b2.b3.b10"));
    }

    #[test]
    fn roots_are_minimal() {
        let p = principal(&[("b1.b2", "karordforande"), ("b1.b2.b5", "ledare"), ("b1.b9", "ledare")]);
        assert_eq!(p.roots_for(Permission::MembersRead), vec!["b1.b2".to_owned(), "b1.b9".to_owned()]);
        assert_eq!(p.roots_for(Permission::NationalIdRead), [] as [std::string::String; 0]);
    }
}
