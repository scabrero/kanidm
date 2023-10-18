use crate::be::dbvalue::DbValueAppPassword;
use crate::prelude::*;
use crate::repl::proto::{ReplAppPasswordV1, ReplAttrV1};
use crate::schema::SchemaAttribute;
use crate::value::AppPassword;
use crate::valueset::{uuid_to_proto_string, DbValueSetV2, ValueSet};
use std::collections::btree_map::Entry as BTreeEntry;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct ValueSetAppPassword {
    map: BTreeMap<Uuid, AppPassword>,
}

impl ValueSetAppPassword {
    pub fn new(u: Uuid, p: AppPassword) -> Box<Self> {
        let mut map = BTreeMap::new();
        map.insert(u, p);
        Box::new(ValueSetAppPassword { map })
    }

    pub fn from_dbvs2(data: Vec<DbValueAppPassword>) -> Result<ValueSet, OperationError> {
        #[allow(clippy::unnecessary_filter_map)]
        let map = data
            .into_iter()
            .filter_map(|dbv| match dbv {
                DbValueAppPassword::V1 {
                    refer,
                    application,
                    label,
                } => Some((refer, AppPassword { application, label })),
            })
            .collect();
        Ok(Box::new(ValueSetAppPassword { map }))
    }

    pub fn from_repl_v1(data: &[ReplAppPasswordV1]) -> Result<ValueSet, OperationError> {
        #[allow(clippy::unnecessary_filter_map)]
        let map = data
            .iter()
            .filter_map(
                |ReplAppPasswordV1 {
                     refer,
                     application,
                     label,
                 }| {
                    Some((
                        *refer,
                        AppPassword {
                            application: *application,
                            label: label.to_string(),
                        },
                    ))
                },
            )
            .collect();
        Ok(Box::new(ValueSetAppPassword { map }))
    }
}

impl ValueSetT for ValueSetAppPassword {
    // Returns whether the value was newly inserted. That is:
    // *  If the set did not previously contain an equal value, true is returned.
    // * If the set already contained an equal value, false is returned, and the entry is not updated.
    fn insert_checked(&mut self, value: Value) -> Result<bool, OperationError> {
        match value {
            Value::AppPassword(u, m) => {
                if let BTreeEntry::Vacant(e) = self.map.entry(u) {
                    e.insert(m);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Err(OperationError::InvalidValueState),
        }
    }

    fn clear(&mut self) {
        self.map.clear();
    }

    fn remove(&mut self, pv: &PartialValue, _cid: &Cid) -> bool {
        match pv {
            PartialValue::Refer(u) => self.map.remove(u).is_some(),
            _ => false,
        }
    }

    fn purge(&mut self, _cid: &Cid) -> bool {
        true
    }

    fn contains(&self, pv: &PartialValue) -> bool {
        match pv {
            PartialValue::Refer(u) => self.map.contains_key(u),
            _ => false,
        }
    }

    fn substring(&self, _pv: &PartialValue) -> bool {
        false
    }

    fn lessthan(&self, _pv: &PartialValue) -> bool {
        false
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    fn generate_idx_eq_keys(&self) -> Vec<String> {
        self.map
            .keys()
            .map(|u| u.as_hyphenated().to_string())
            .collect()
    }

    fn syntax(&self) -> SyntaxType {
        SyntaxType::AppPassword
    }

    fn validate(&self, _schema_attr: &SchemaAttribute) -> bool {
        self.map.iter().all(|(_, at)| {
            Value::validate_str_escapes(&at.label) && Value::validate_singleline(&at.label)
        })
    }

    fn to_proto_string_clone_iter(&self) -> Box<dyn Iterator<Item = String> + '_> {
        Box::new(
            self.map
                .iter()
                .map(|(u, m)| format!("{}: {:?}", uuid_to_proto_string(*u), m)),
        )
    }

    fn to_db_valueset_v2(&self) -> DbValueSetV2 {
        DbValueSetV2::AppPassword(
            self.map
                .iter()
                .map(|(u, m)| DbValueAppPassword::V1 {
                    refer: *u,
                    application: m.application,
                    label: m.label.clone(),
                })
                .collect(),
        )
    }

    fn to_repl_v1(&self) -> ReplAttrV1 {
        ReplAttrV1::AppPassword {
            set: self
                .map
                .iter()
                .map(|(u, m)| ReplAppPasswordV1 {
                    refer: *u,
                    application: m.application,
                    label: m.label.clone(),
                })
                .collect(),
        }
    }

    fn to_partialvalue_iter(&self) -> Box<dyn Iterator<Item = PartialValue> + '_> {
        Box::new(self.map.keys().cloned().map(PartialValue::Refer))
    }

    fn to_value_iter(&self) -> Box<dyn Iterator<Item = Value> + '_> {
        Box::new(
            self.map
                .iter()
                .map(|(u, m)| Value::AppPassword(*u, m.clone())),
        )
    }

    fn equal(&self, other: &ValueSet) -> bool {
        if let Some(other) = other.as_apppassword_map() {
            &self.map == other
        } else {
            debug_assert!(false);
            false
        }
    }

    fn merge(&mut self, other: &ValueSet) -> Result<(), OperationError> {
        if let Some(b) = other.as_apppassword_map() {
            mergemaps!(self.map, b)
        } else {
            debug_assert!(false);
            Err(OperationError::InvalidValueState)
        }
    }

    fn as_apppassword_map(&self) -> Option<&BTreeMap<Uuid, AppPassword>> {
        Some(&self.map)
    }
}
