//! Figure properties the user set by hand.
//!
//! A hand-set value is the user's decision. Themes keep it, and automation
//! skips it unless the caller says the user asked for the change. Only edits
//! made in the desktop UI are recorded; automation writes are not, so a later
//! theme or agent edit may still replace what an agent chose.

use super::{
    PropertyAddress, PropertyId, PropertySkip, ScopeKind, SkipReason, canvas, definition, guide,
};
use crate::actions::Action;
use crate::automation::TargetRef;
use crate::state::PlotiumApp;
use serde::{Deserialize, Serialize};

/// One property the user set by hand on one target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandEdit {
    pub target: TargetRef,
    /// Catalog property id; an id this build no longer defines is ignored.
    pub property: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HandEdits(Vec<HandEdit>);

/// Properties that store one decision. Writing either one replaces what the
/// other holds (a placement choice clears a dragged legend position), so a
/// hand-set member protects the whole group.
const LINKED: &[&[PropertyId]] = &[&[guide::PLACEMENT, guide::POSITION]];

fn linked(property: PropertyId) -> &'static [PropertyId] {
    LINKED
        .iter()
        .copied()
        .find(|group| group.contains(&property))
        .unwrap_or(&[])
}

/// Figure and page properties belong to the document the user is drawing.
/// App preferences and processing parameters are not presentation choices a
/// theme could repaint, so they are never recorded.
pub(crate) fn is_recorded(property: PropertyId) -> bool {
    // Grid and auto height are live page preferences outside undo history.
    if matches!(property, canvas::SHOW_GRID | canvas::AUTO_HEIGHT) {
        return false;
    }
    definition(property).is_some_and(|definition| {
        matches!(
            definition.scope_kind,
            ScopeKind::Document | ScopeKind::Canvas | ScopeKind::Object
        )
    })
}

impl HandEdits {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &HandEdit> {
        self.0.iter()
    }

    /// Whether the user set `property`, or a property linked to it, on `target`.
    pub fn contains(&self, target: &TargetRef, property: PropertyId) -> bool {
        self.0.iter().any(|edit| {
            edit.target == *target
                && (edit.property == property.as_str()
                    || linked(property)
                        .iter()
                        .any(|id| edit.property == id.as_str()))
        })
    }

    /// Record a hand-made set, or forget the property (and any linked one) on a
    /// reset. Resetting a whole encoding forgets every property of that target.
    fn record(&mut self, address: &PropertyAddress, set: bool) {
        let property = address.definition.as_str();
        if set {
            if is_recorded(address.definition)
                && !self
                    .0
                    .iter()
                    .any(|edit| edit.target == address.target && edit.property == property)
            {
                self.0.push(HandEdit {
                    target: address.target.clone(),
                    property: property.to_owned(),
                });
            }
        } else if property == ENCODING_RESET {
            self.0.retain(|edit| edit.target != address.target);
        } else {
            let group = linked(address.definition);
            self.0.retain(|edit| {
                edit.target != address.target
                    || (edit.property != property
                        && !group.iter().any(|id| edit.property == id.as_str()))
            });
        }
    }
}

/// The address `plan_encoding_reset` reports for rebuilding a series encoding.
const ENCODING_RESET: &str = "encoding";

impl PlotiumApp {
    /// Whether the user set `property` on `target` by hand.
    pub fn is_hand_set(&self, target: &TargetRef, property: PropertyId) -> bool {
        self.doc.hand_edits.contains(target, property)
    }

    /// The undoable record update for hand edits at `addresses`, or `None`
    /// when the record would not change and `always` is false.
    pub(crate) fn hand_edit_action(
        &self,
        addresses: &[PropertyAddress],
        set: bool,
        always: bool,
    ) -> Option<Action> {
        let before = self.doc.hand_edits.clone();
        let mut after = before.clone();
        for address in addresses {
            after.record(address, set);
        }
        (always || after != before).then(|| Action::SetHandEdits {
            before: Box::new(before),
            after: Box::new(after),
        })
    }

    /// Execute a hand-made edit that does not go through the property catalog,
    /// recording the catalog properties it sets (or resets) in the same undo
    /// step.
    pub fn execute_hand_edit(&mut self, action: Action, addresses: &[PropertyAddress], set: bool) {
        let action = match self.hand_edit_action(addresses, set, false) {
            Some(record) => Action::Composite(vec![action, record]),
            None => action,
        };
        self.execute_action(action);
    }

    /// Split `targets` into those automation may write and skips for those the
    /// user set `property` on by hand.
    pub(crate) fn withhold_hand_edits(
        &self,
        property: PropertyId,
        targets: &[TargetRef],
    ) -> (Vec<TargetRef>, Vec<PropertySkip>) {
        let mut free = Vec::new();
        let mut held = Vec::new();
        for target in targets {
            if self.is_hand_set(target, property) {
                held.push(PropertySkip::new(
                    target.clone(),
                    SkipReason::HandSet,
                    hand_set_message(property),
                ));
            } else {
                free.push(target.clone());
            }
        }
        (free, held)
    }

    /// Hand edits whose target still has the property, in the order the user
    /// made them. Entries for deleted objects stay in the record (an undo can
    /// bring the object back) but are not reported.
    pub fn live_hand_edits(&self) -> Vec<PropertyAddress> {
        self.doc
            .hand_edits
            .iter()
            .filter_map(|edit| {
                let definition = super::definition_by_key(&edit.property)?;
                let address = PropertyAddress::new(edit.target.clone(), definition.id);
                self.resolve_property(&address).ok().map(|_| address)
            })
            .collect()
    }
}

/// Why automation left a hand-set property alone, worded for the agent that
/// reads it.
pub fn hand_set_message(property: PropertyId) -> String {
    format!(
        "the user set {property} by hand; change it only when the user asks, \
         by passing override_hand_edits: true"
    )
}
